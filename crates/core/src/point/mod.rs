//! The contract every point type implements, explicit or implicit.
//!
//! A predicate reads a point twice. Its fast path, the floating-point
//! filter, uses [`Point::approx`], an approximation of the coordinates, and
//! [`Point::err`], a bound on the error of each one. Only when the filter
//! cannot decide does it ask for [`Point::exact`], the coordinates as
//! expansions in a [`Scratch`] arena, in homogeneous form. A last exact
//! stage asks for [`Point::exact_big`] instead: the coordinates as big
//! integers ([`Big`]), with no weight.
//!
//! `[f64; 3]` implements the contract as an explicit point: its components
//! are its exact coordinates, with no error. [`check`] tests that a point
//! keeps the contract.

mod check;

pub use check::{CheckError, Violation, check};

use core::fmt;

use crate::bigint::Big;
use crate::expansion;
use crate::scratch::{Scratch, ScratchExhausted, Span};

/// A point in three dimensions, as the predicates read it.
///
/// Implementations must uphold, for each axis `k`:
///
/// - `err()[k]` is not negative (it may be `+∞`) and bounds the distance
///   between `approx()[k]` and coordinate `k` of the point, `x_k / w`,
///   where `x_k` is the numerator and `w` the weight that
///   [`exact`](Self::exact) writes (`x_k` itself when `w` is
///   [`Weight::One`]);
/// - `err()[k]` is zero only when `approx()[k]` is that coordinate;
/// - [`is_explicit`](Self::is_explicit) returns `true` only if `approx()`
///   holds the exact coordinates, `err()` is zero on every axis and
///   [`exact`](Self::exact) gives [`Weight::One`]: for such points the
///   predicates use `approx()` alone, with no uncertainty;
/// - every span of the [`ExactCoords`] that [`exact`](Self::exact) returns
///   holds a valid expansion (see [`expansion`]: nonzero components of
///   strictly increasing magnitude that do not overlap), written in the
///   arena it was given;
/// - [`exact`](Self::exact) leaves the arena below the top it had when
///   `exact` was called as it was: it writes only above that top and never
///   resets the arena to a mark taken before the call, since a predicate
///   keeps the exact coordinates of the points it read before there;
/// - while a predicate runs, which may call each method more than once,
///   [`approx`](Self::approx), [`err`](Self::err) and
///   [`is_explicit`](Self::is_explicit) give the same answer every time and
///   [`exact`](Self::exact) the same coordinates: a type with interior
///   mutability must not change them in the middle of a call;
/// - [`exact_big`](Self::exact_big) gives the same coordinates as
///   [`exact`](Self::exact), `x_k / w`, whenever `exact` succeeds, and the
///   same answer on every call; it fails with [`ExactError::OutOfRange`]
///   only when an input of the point is not finite, and with
///   [`ExactError::Weighted`] when the exact coordinates have a weight
///   other than one;
/// - [`exact_big`](Self::exact_big) leaves the arena below the top it had
///   when it was called as it was: it writes only above that top and never
///   resets the arena to a mark taken before the call. After an error the
///   arena may hold partial results above that top.
///
/// A predicate given a point that breaks this contract can return a wrong
/// sign: release builds of the predicates do not check it, and debug builds
/// check only part of it. [`check`] tests a point against every obligation
/// that the views of the point can show, and is meant for the tests of a
/// type of your own.
///
/// # Examples
///
/// ```
/// use implicit_predicates_core::{Point, Scratch, Weight};
///
/// let p = [1.5, 0.0, -2.0];
/// assert!(p.is_explicit());
/// assert_eq!(p.err(), &[0.0; 3]);
///
/// let mut buf = [0.0; 3];
/// let mut scratch = Scratch::borrowed(&mut buf);
/// let exact = p.exact(&mut scratch)?;
/// assert_eq!(scratch.get(exact.x), &[1.5]);
/// assert!(exact.y.is_empty()); // zero
/// assert_eq!(exact.w, Weight::One);
/// # Ok::<(), implicit_predicates_core::ExactError>(())
/// ```
pub trait Point {
    /// Returns an approximation of the coordinates.
    fn approx(&self) -> &[f64; 3];

    /// Returns, per coordinate, a bound on the error of
    /// [`approx`](Self::approx).
    fn err(&self) -> &[f64; 3];

    /// Whether the point is explicit. It may return `true` only if
    /// [`approx`](Self::approx) holds the exact coordinates, every
    /// [`err`](Self::err) is zero and [`exact`](Self::exact) gives
    /// [`Weight::One`]; returning `false` is always allowed. A wrong `true`
    /// makes the predicates return wrong signs; debug builds of the
    /// predicates and [`check`] verify that the errors of such a point are
    /// zero.
    fn is_explicit(&self) -> bool;

    /// Writes the exact coordinates as expansions in `scratch`: the point is
    /// `(x / w, y / w, z / w)`. Every span returned must hold a valid
    /// expansion of `scratch` (see [`expansion`]); predicates rely on it,
    /// and check it only in debug builds.
    ///
    /// # Errors
    ///
    /// - [`ExactError::Exhausted`] if `scratch` runs out of room;
    /// - [`ExactError::OutOfRange`] if the inputs of the point lie outside
    ///   the range in which its exact coordinates can be computed: they are
    ///   not finite, or a product they need leaves the product domain of
    ///   [`eft`](crate::eft).
    ///
    /// After an error, `scratch` may hold partial results above the top it
    /// had before the call; resetting to a mark taken before the call
    /// releases them.
    fn exact(&self, scratch: &mut Scratch<'_>) -> Result<ExactCoords, ExactError>;

    /// Writes the exact coordinates as big integers in `scratch`: the
    /// point is `(x, y, z)`, with no common denominator. Each coordinate has
    /// the same value as the one that [`exact`](Self::exact) gives, when it
    /// succeeds.
    ///
    /// # Errors
    ///
    /// - [`ExactError::Exhausted`] if `scratch` runs out of room;
    /// - [`ExactError::OutOfRange`] only if an input of the point is not
    ///   finite;
    /// - [`ExactError::Weighted`] if the exact coordinates of the point have
    ///   a weight other than one.
    ///
    /// The method writes only above the top the arena had before the call
    /// and never resets below it; after an error, `scratch` may hold partial
    /// results above that top, which resetting to a mark taken before the
    /// call releases.
    fn exact_big(&self, scratch: &mut Scratch<'_>) -> Result<BigCoords, ExactError>;
}

/// The exact coordinates of a point as big integers: the point is
/// `(x, y, z)`. The values belong to the arena given to
/// [`Point::exact_big`].
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct BigCoords {
    /// The first coordinate.
    pub x: Big,
    /// The second coordinate.
    pub y: Big,
    /// The third coordinate.
    pub z: Big,
}

impl BigCoords {
    /// The coordinates `x`, `y` and `z`.
    #[must_use]
    pub fn new(x: Big, y: Big, z: Big) -> BigCoords {
        BigCoords { x, y, z }
    }
}

/// The exact coordinates of a point, in homogeneous form: the point is
/// `(x / w, y / w, z / w)`. The spans belong to the arena given to
/// [`Point::exact`], and each holds a valid expansion (see [`expansion`]);
/// the predicates that read them check it only in debug builds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExactCoords {
    /// The numerator of the first coordinate.
    pub x: Span,
    /// The numerator of the second coordinate.
    pub y: Span,
    /// The numerator of the third coordinate.
    pub z: Span,
    /// The common denominator.
    pub w: Weight,
}

/// The common denominator of the exact coordinates of a point.
///
/// More kinds of denominator may be added: code that matches on it needs an
/// arm for the others, in which a predicate that does not know the
/// denominator must not give a sign.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Weight {
    /// The denominator is one: the numerators are the coordinates.
    One,
    /// The denominator is the expansion at this span. The predicates of the
    /// `implicit-predicates` crate do not support it yet: they panic on a
    /// point whose exact coordinates have it, rather than give a sign
    /// computed without it.
    Expansion(Span),
}

/// The error of [`Point::exact`].
///
/// More reasons may be added: code that matches on it needs an arm for the
/// others, in which the exact coordinates are not available.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExactError {
    /// The arena ran out of room.
    Exhausted(ScratchExhausted),
    /// The inputs of the point lie outside the range of the exact
    /// arithmetic.
    OutOfRange,
    /// The exact coordinates of the point have a weight other than one,
    /// which big-integer coordinates cannot carry.
    Weighted,
}

impl From<ScratchExhausted> for ExactError {
    fn from(e: ScratchExhausted) -> Self {
        ExactError::Exhausted(e)
    }
}

impl fmt::Display for ExactError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExactError::Exhausted(e) => e.fmt(f),
            ExactError::OutOfRange => {
                f.write_str("point outside the range of the exact arithmetic")
            }
            ExactError::Weighted => f.write_str(
                "point with a weight other than one, which big-integer coordinates cannot carry",
            ),
        }
    }
}

impl core::error::Error for ExactError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            ExactError::Exhausted(e) => Some(e),
            ExactError::OutOfRange | ExactError::Weighted => None,
        }
    }
}

/// An explicit point, not validated: its components are its exact
/// coordinates. A component that is not finite makes
/// [`exact`](Point::exact) fail with [`ExactError::OutOfRange`].
impl Point for [f64; 3] {
    #[inline]
    fn approx(&self) -> &[f64; 3] {
        self
    }

    #[inline]
    fn err(&self) -> &[f64; 3] {
        &[0.0; 3]
    }

    #[inline]
    fn is_explicit(&self) -> bool {
        true
    }

    /// Writes each coordinate as an expansion of one component, or none for
    /// zero, with [`Weight::One`]. Needs up to 3 free components; on an
    /// error the arena is unchanged.
    fn exact(&self, scratch: &mut Scratch<'_>) -> Result<ExactCoords, ExactError> {
        if !self.iter().all(|c| c.is_finite()) {
            return Err(ExactError::OutOfRange);
        }
        let mark = scratch.mark();
        let mut write = |c: f64| {
            scratch
                .write(1, |out| expansion::from_f64(c, out))
                .inspect_err(|_| scratch.reset(mark))
        };
        Ok(ExactCoords {
            x: write(self[0])?,
            y: write(self[1])?,
            z: write(self[2])?,
            w: Weight::One,
        })
    }

    /// Writes each coordinate as a big integer. Needs up to 6 free
    /// components; on an error the arena may hold the coordinates already
    /// written.
    fn exact_big(&self, scratch: &mut Scratch<'_>) -> Result<BigCoords, ExactError> {
        if !self.iter().all(|c| c.is_finite()) {
            return Err(ExactError::OutOfRange);
        }
        Ok(BigCoords::new(
            scratch.big_from_f64(self[0])?,
            scratch.big_from_f64(self[1])?,
            scratch.big_from_f64(self[2])?,
        ))
    }
}
