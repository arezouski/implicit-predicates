//! Checking that a point keeps the contract of [`Point`].

use core::fmt;

use super::{ExactError, Point, Weight};
use crate::bigint::Big;
use crate::contract;
use crate::scratch::{Scratch, ScratchExhausted, Span};
use crate::sign::Sign;

/// Checks that `p` keeps the contract of [`Point`], with the exact
/// coordinates that `p` itself gives.
///
/// It checks, in this order:
///
/// 1. that no [`err`](Point::err) is NaN or negative (`+∞` is allowed);
/// 2. that [`approx`](Point::approx) is finite on every axis whose `err` is
///    finite, unless the reason is an input of the point that is not
///    finite, which the next check reports;
/// 3. that [`exact_big`](Point::exact_big) succeeds, which it must for a
///    point with finite inputs and a weight of one, and gives each
///    coordinate in the unique representation of a [`Big`], with words
///    below the top of the arena;
/// 4. that on every axis whose `err` is finite, `approx` lies within `err`
///    of the coordinate that `exact_big` gives, compared exactly; an axis
///    whose `err` is `+∞` passes whatever its `approx`;
/// 5. that a point whose [`is_explicit`](Point::is_explicit) is `true` has
///    no error on any axis, so that, by the previous check, its `approx` is
///    exact;
/// 6. that when [`exact`](Point::exact) succeeds, it gives a weight of
///    one, every span lies below the top of the arena and holds a valid
///    expansion (see [`expansion`](crate::expansion)), and each coordinate
///    has the value that `exact_big` gives; `exact` may fail with
///    [`ExactError::OutOfRange`] when the coordinates leave the range of
///    the expansions, and a failure with [`ExactError::Weighted`], which
///    the contract names only for `exact_big`, is reported as
///    [`Violation::Weighted`];
/// 7. that a second call to `approx`, `err`, `is_explicit` and `exact_big`
///    gives the same values as the first, again in the unique
///    representation; and that a second call to `exact` fails with
///    `OutOfRange` when the first did, and otherwise gives a weight of one
///    and valid expansions below the top with the same values as the
///    first.
///
/// After every call to `exact` and `exact_big`, before reading what it
/// returned, `check` also checks that the call did not leave the top of the
/// arena below the top it was given ([`Violation::ArenaBelowTop`]).
///
/// It returns at the first obligation broken. When the approximation on an
/// axis is not finite, `check` reports [`Violation::ApproxNotFinite`] even
/// if `exact_big` then runs out of room, since the violation is known
/// without it. `check` can only tell whether the views of the point agree
/// with one another: it cannot know whether the exact coordinates are
/// those of the point its author meant to define. Nor does it see a point
/// that lowers the top and then writes back up past where it was, nor a
/// span of `exact` that points into what was in the arena below the top
/// it was given, rather than at what the point wrote, when the values
/// there happen to be the right ones.
///
/// `check` writes the exact coordinates in `scratch`, twice as big integers
/// and twice as expansions, and the sums and differences that compare them.
/// A growable arena (`Scratch::growable`, with the `alloc` feature) never
/// runs out. With a borrowed buffer, a few times the room that the exact
/// coordinates of the point need is enough; when it is not, `check` returns
/// [`CheckError::Exhausted`], which says nothing about the point. Either
/// way, the arena is left with the top it had on entry, except when the
/// point itself lowered it below that top, which `check` reports as
/// [`Violation::ArenaBelowTop`]: the top then stays where the point left
/// it, and what was below the top on entry may have been overwritten.
///
/// # Errors
///
/// - [`CheckError::Violation`] with the obligation that `p` breaks;
/// - [`CheckError::Exhausted`] if `scratch` runs out of room before the
///   check ends.
///
/// # Examples
///
/// A point at `1 + 2^-60` whose approximation is `1`: it keeps the
/// contract with an error bound of `2^-60`, and breaks it with a bound of
/// zero.
///
/// ```
/// use implicit_predicates_core::point::{CheckError, Violation, check};
/// use implicit_predicates_core::{
///     Big, BigCoords, ExactCoords, ExactError, Point, Scratch, Weight,
/// };
///
/// struct Near {
///     err: [f64; 3],
/// }
///
/// impl Point for Near {
///     fn approx(&self) -> &[f64; 3] {
///         &[1.0, 0.0, 0.0]
///     }
///
///     fn err(&self) -> &[f64; 3] {
///         &self.err
///     }
///
///     fn is_explicit(&self) -> bool {
///         false
///     }
///
///     fn exact(&self, scratch: &mut Scratch<'_>) -> Result<ExactCoords, ExactError> {
///         let x = scratch.push(&[2f64.powi(-60), 1.0])?;
///         let zero = scratch.push(&[])?;
///         Ok(ExactCoords { x, y: zero, z: zero, w: Weight::One })
///     }
///
///     fn exact_big(&self, scratch: &mut Scratch<'_>) -> Result<BigCoords, ExactError> {
///         let one = scratch.big_from_f64(1.0)?;
///         let tiny = scratch.big_from_f64(2f64.powi(-60))?;
///         let x = scratch.big_add(one, tiny)?;
///         Ok(BigCoords::new(x, Big::ZERO, Big::ZERO))
///     }
/// }
///
/// let mut buf = [0.0; 64];
/// let mut scratch = Scratch::borrowed(&mut buf);
///
/// let good = Near { err: [2f64.powi(-60), 0.0, 0.0] };
/// assert_eq!(check(&good, &mut scratch), Ok(()));
///
/// let bad = Near { err: [0.0; 3] };
/// assert_eq!(
///     check(&bad, &mut scratch),
///     Err(CheckError::Violation(Violation::ErrTooSmall { axis: 0 }))
/// );
/// assert_eq!(scratch.used(), 0);
///
/// // `[f64; 3]` keeps the contract when its components are finite.
/// assert_eq!(check(&[0.5, -3.0, 1e300], &mut scratch), Ok(()));
/// assert_eq!(
///     check(&[f64::NAN, 0.0, 0.0], &mut scratch),
///     Err(CheckError::Violation(Violation::NotFinite))
/// );
/// ```
pub fn check<P: Point + ?Sized>(p: &P, scratch: &mut Scratch<'_>) -> Result<(), CheckError> {
    let mark = scratch.mark();
    let used = scratch.used();
    let result = check_above(p, scratch);
    // A point that resets the arena below the top it was given leaves it
    // lower than the mark, where a reset to the mark is a logic error.
    if scratch.used() >= used {
        scratch.reset(mark);
    }
    result
}

/// The failure of [`check`].
///
/// More reasons may be added: code that matches on it needs an arm for the
/// others.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CheckError {
    /// The arena ran out of room: whether the point keeps the contract is
    /// not known.
    Exhausted,
    /// The point breaks an obligation of the contract.
    Violation(Violation),
}

/// An obligation of the contract of [`Point`] that a point breaks, with the
/// axis when the obligation is about one coordinate.
///
/// More obligations may be added: code that matches on it needs an arm for
/// the others.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Violation {
    /// The error bound on the axis is NaN or negative.
    InvalidErr {
        /// The axis, 0 for `x`, 1 for `y`, 2 for `z`.
        axis: usize,
    },
    /// The approximation on the axis is not finite, with a finite error
    /// bound.
    ApproxNotFinite {
        /// The axis, 0 for `x`, 1 for `y`, 2 for `z`.
        axis: usize,
    },
    /// [`Point::exact_big`] failed with [`ExactError::OutOfRange`]: an input
    /// of the point is not finite.
    NotFinite,
    /// The exact coordinates have a weight other than one:
    /// [`Point::exact_big`] failed with [`ExactError::Weighted`], or
    /// [`Point::exact`] gave a weight other than [`Weight::One`].
    Weighted,
    /// The approximation on the axis is further from the exact coordinate
    /// than the error bound.
    ErrTooSmall {
        /// The axis, 0 for `x`, 1 for `y`, 2 for `z`.
        axis: usize,
    },
    /// [`Point::is_explicit`] is `true`, but the error bound on the axis is
    /// not zero.
    ExplicitWithErr {
        /// The axis, 0 for `x`, 1 for `y`, 2 for `z`.
        axis: usize,
    },
    /// The span that [`Point::exact`] gave for the axis does not hold a
    /// valid expansion.
    InvalidExpansion {
        /// The axis, 0 for `x`, 1 for `y`, 2 for `z`.
        axis: usize,
    },
    /// The coordinate that [`Point::exact_big`] gave for the axis is not in
    /// the unique representation of a [`Big`].
    NonCanonicalBig {
        /// The axis, 0 for `x`, 1 for `y`, 2 for `z`.
        axis: usize,
    },
    /// [`Point::exact`] and [`Point::exact_big`] give different values for
    /// the coordinate on the axis.
    ExactMismatch {
        /// The axis, 0 for `x`, 1 for `y`, 2 for `z`.
        axis: usize,
    },
    /// Two calls in a row to [`Point::approx`], [`Point::err`],
    /// [`Point::is_explicit`], [`Point::exact`] or [`Point::exact_big`]
    /// gave different values, or [`Point::exact`] failed with
    /// [`ExactError::OutOfRange`] on one call and not on the other.
    Unstable,
    /// A call to [`Point::exact`] or [`Point::exact_big`] left the top of
    /// the arena below the top it had when the call began: it reset the
    /// arena to a mark taken before the call, where a predicate keeps the
    /// coordinates of the points it read earlier.
    ArenaBelowTop,
}

impl From<ScratchExhausted> for CheckError {
    fn from(_: ScratchExhausted) -> Self {
        CheckError::Exhausted
    }
}

impl From<Violation> for CheckError {
    fn from(v: Violation) -> Self {
        CheckError::Violation(v)
    }
}

impl fmt::Display for CheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CheckError::Exhausted => {
                f.write_str("the arena ran out of room before the check of the point ended")
            }
            CheckError::Violation(v) => write!(f, "the point breaks the contract of `Point`: {v}"),
        }
    }
}

impl core::error::Error for CheckError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            CheckError::Exhausted => None,
            CheckError::Violation(v) => Some(v),
        }
    }
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Violation::InvalidErr { axis } => {
                write!(f, "the error bound on axis {axis} is NaN or negative")
            }
            Violation::ApproxNotFinite { axis } => write!(
                f,
                "the approximation on axis {axis} is not finite, but its error bound is"
            ),
            Violation::NotFinite => f.write_str("an input of the point is not finite"),
            Violation::Weighted => {
                f.write_str("the exact coordinates have a weight other than one")
            }
            Violation::ErrTooSmall { axis } => write!(
                f,
                "the approximation on axis {axis} is further from the exact coordinate \
                 than its error bound"
            ),
            Violation::ExplicitWithErr { axis } => write!(
                f,
                "the point says it is explicit, but its error bound on axis {axis} is not zero"
            ),
            Violation::InvalidExpansion { axis } => write!(
                f,
                "the exact coordinate on axis {axis} is not a valid expansion"
            ),
            Violation::NonCanonicalBig { axis } => write!(
                f,
                "the exact coordinate on axis {axis} as a big integer is not in its \
                 unique representation"
            ),
            Violation::ExactMismatch { axis } => write!(
                f,
                "the exact coordinates as expansions and as big integers differ on axis {axis}"
            ),
            Violation::Unstable => {
                f.write_str("two calls in a row to the same method gave different values")
            }
            Violation::ArenaBelowTop => f.write_str(
                "a call for the exact coordinates left the top of the arena below the top \
                 it was given",
            ),
        }
    }
}

impl core::error::Error for Violation {}

/// The obligations of [`check`], in its order, writing above the top of
/// `scratch`; the caller restores the top.
fn check_above<P: Point + ?Sized>(p: &P, scratch: &mut Scratch<'_>) -> Result<(), CheckError> {
    let approx = *p.approx();
    let err = *p.err();
    if let Some(axis) = (0..3).find(|&k| !contract::axis_err_is_valid(err[k])) {
        return Err(Violation::InvalidErr { axis }.into());
    }
    // An approximation that is not finite usually comes from an input that
    // is not finite, as in a `[f64; 3]` with a NaN: `exact_big` tells.
    let not_finite = (0..3).find(|&k| err[k].is_finite() && !approx[k].is_finite());
    let big = exact_big(p, scratch);
    if let Some(axis) = not_finite {
        return Err(match big {
            Err(CheckError::Violation(v @ (Violation::NotFinite | Violation::ArenaBelowTop))) => v,
            _ => Violation::ApproxNotFinite { axis },
        }
        .into());
    }
    let big = big?;
    if let Some(axis) = (0..3).find(|&k| !contract::big_is_canonical(scratch, big[k])) {
        return Err(Violation::NonCanonicalBig { axis }.into());
    }
    for axis in 0..3 {
        // An infinite bound holds whatever the approximation, which may
        // then not be finite.
        if err[axis].is_finite() && !within(scratch, approx[axis], err[axis], big[axis])? {
            return Err(Violation::ErrTooSmall { axis }.into());
        }
    }
    let explicit = p.is_explicit();
    if explicit {
        if let Some(axis) = (0..3).find(|&k| err[k] != 0.0) {
            return Err(Violation::ExplicitWithErr { axis }.into());
        }
    }
    let top = scratch.used();
    let exact = p.exact(scratch);
    if scratch.used() < top {
        return Err(Violation::ArenaBelowTop.into());
    }
    let in_range = exact.is_ok();
    match exact {
        Ok(coords) => {
            if coords.w != Weight::One {
                return Err(Violation::Weighted.into());
            }
            let spans = [coords.x, coords.y, coords.z];
            if let Some(axis) = (0..3).find(|&k| !contract::expansion_is_valid(scratch, spans[k])) {
                return Err(Violation::InvalidExpansion { axis }.into());
            }
            for axis in 0..3 {
                if !same_value(scratch, spans[axis], big[axis])? {
                    return Err(Violation::ExactMismatch { axis }.into());
                }
            }
        }
        // Allowed: the coordinates may leave the range of the expansions.
        Err(ExactError::OutOfRange) => {}
        Err(ExactError::Exhausted(_)) => return Err(CheckError::Exhausted),
        Err(ExactError::Weighted) => return Err(Violation::Weighted.into()),
    }
    let again = |a: &[f64; 3], b: &[f64; 3]| {
        (0..3).all(|k| a[k] == b[k] || (a[k].is_nan() && b[k].is_nan()))
    };
    if !again(&approx, p.approx()) || !again(&err, p.err()) || explicit != p.is_explicit() {
        return Err(Violation::Unstable.into());
    }
    let top = scratch.used();
    let second = p.exact_big(scratch);
    if scratch.used() < top {
        return Err(Violation::ArenaBelowTop.into());
    }
    let second = match second {
        Ok(c) => [c.x, c.y, c.z],
        Err(ExactError::Exhausted(_)) => return Err(CheckError::Exhausted),
        Err(_) => return Err(Violation::Unstable.into()),
    };
    if let Some(axis) = (0..3).find(|&k| !contract::big_is_canonical(scratch, second[k])) {
        return Err(Violation::NonCanonicalBig { axis }.into());
    }
    for axis in 0..3 {
        if scratch.big_sub(second[axis], big[axis])?.sign() != Sign::Zero {
            return Err(Violation::Unstable.into());
        }
    }
    let top = scratch.used();
    let second = p.exact(scratch);
    if scratch.used() < top {
        return Err(Violation::ArenaBelowTop.into());
    }
    match second {
        Ok(coords) if in_range => {
            if coords.w != Weight::One {
                return Err(Violation::Unstable.into());
            }
            let spans = [coords.x, coords.y, coords.z];
            if let Some(axis) = (0..3).find(|&k| !contract::expansion_is_valid(scratch, spans[k])) {
                return Err(Violation::InvalidExpansion { axis }.into());
            }
            // The first call gave the values of `big`.
            for axis in 0..3 {
                if !same_value(scratch, spans[axis], big[axis])? {
                    return Err(Violation::Unstable.into());
                }
            }
        }
        Err(ExactError::OutOfRange) if !in_range => {}
        Err(ExactError::Exhausted(_)) => return Err(CheckError::Exhausted),
        // In range on one call and not on the other, or weighted now.
        Ok(_) | Err(_) => return Err(Violation::Unstable.into()),
    }
    Ok(())
}

/// The coordinates that [`Point::exact_big`] gives, or the failure of
/// [`check`] that its error is, or [`Violation::ArenaBelowTop`] if it
/// lowered the top.
fn exact_big<P: Point + ?Sized>(p: &P, scratch: &mut Scratch<'_>) -> Result<[Big; 3], CheckError> {
    let top = scratch.used();
    let result = p.exact_big(scratch);
    if scratch.used() < top {
        return Err(Violation::ArenaBelowTop.into());
    }
    match result {
        Ok(c) => Ok([c.x, c.y, c.z]),
        Err(ExactError::Exhausted(_)) => Err(CheckError::Exhausted),
        Err(ExactError::OutOfRange) => Err(Violation::NotFinite.into()),
        Err(ExactError::Weighted) => Err(Violation::Weighted.into()),
    }
}

/// Whether `|approx - exact| <= err`, exactly: both `err - (approx - exact)`
/// and `err + (approx - exact)` are not negative. `approx` and `err` must be
/// finite. Releases what it writes.
fn within(
    scratch: &mut Scratch<'_>,
    approx: f64,
    err: f64,
    exact: Big,
) -> Result<bool, ScratchExhausted> {
    let mark = scratch.mark();
    let result = (|| {
        let a = scratch.big_from_f64(approx)?;
        let e = scratch.big_from_f64(err)?;
        let d = scratch.big_sub(a, exact)?;
        let below = scratch.big_sub(e, d)?.sign() != Sign::Negative;
        Ok(below && scratch.big_add(e, d)?.sign() != Sign::Negative)
    })();
    scratch.reset(mark);
    result
}

/// Whether the expansion at `span`, whose components are finite, has the
/// value `b`. Releases what it writes.
fn same_value(scratch: &mut Scratch<'_>, span: Span, b: Big) -> Result<bool, ScratchExhausted> {
    let mark = scratch.mark();
    let result = (|| {
        let mut sum = Big::ZERO;
        for i in 0..span.len() {
            let component = scratch.big_from_f64(scratch.get(span)[i])?;
            sum = scratch.big_add(sum, component)?;
        }
        Ok(scratch.big_sub(sum, b)?.sign() == Sign::Zero)
    })();
    scratch.reset(mark);
    result
}

#[cfg(test)]
mod tests {
    use super::{CheckError, Violation, check};
    use crate::bigint::Big;
    use crate::point::{BigCoords, ExactCoords, ExactError, Point};
    use crate::scratch::{Scratch, Span};

    /// How [`NonCanonical`] spoils the coordinate as a big integer.
    #[derive(Clone, Copy, Debug)]
    enum Spoil {
        /// Zero with the sign flag set.
        NegativeZero,
        /// Zero with an exponent other than zero.
        ZeroWithExponent,
        /// The words `[0, 3]`: a zero lowest word.
        LowZero,
        /// The words `[3, 0]`: a zero highest word.
        HighZero,
    }

    /// The point `[0, 3, 0]` with its coordinate on `axis` as a big integer
    /// spoiled as `spoil` says, keeping its value, which no code outside
    /// the crate can write.
    struct NonCanonical {
        axis: usize,
        spoil: Spoil,
    }

    impl Point for NonCanonical {
        fn approx(&self) -> &[f64; 3] {
            &[0.0, 3.0, 0.0]
        }

        fn err(&self) -> &[f64; 3] {
            &[0.0; 3]
        }

        fn is_explicit(&self) -> bool {
            false
        }

        fn exact(&self, scratch: &mut Scratch<'_>) -> Result<ExactCoords, ExactError> {
            self.approx().exact(scratch)
        }

        fn exact_big(&self, scratch: &mut Scratch<'_>) -> Result<BigCoords, ExactError> {
            let mut big = [Big::ZERO, scratch.big_from_f64(3.0)?, Big::ZERO];
            let words = |w: [u64; 2]| {
                move |[]: [&[f64]; 0], out: &mut [f64]| {
                    out[0] = f64::from_bits(w[0]);
                    out[1] = f64::from_bits(w[1]);
                    2
                }
            };
            big[self.axis] = match self.spoil {
                Spoil::NegativeZero => Big::from_parts(Span::EMPTY, 0, true),
                Spoil::ZeroWithExponent => Big::from_parts(Span::EMPTY, 1, false),
                // 3 · 2^-64 · 2^64.
                Spoil::LowZero => {
                    Big::from_parts(scratch.write_words([], 2, words([0, 3]))?, -1, false)
                }
                Spoil::HighZero => {
                    Big::from_parts(scratch.write_words([], 2, words([3, 0]))?, 0, false)
                }
            };
            Ok(BigCoords::new(big[0], big[1], big[2]))
        }
    }

    #[test]
    fn a_big_integer_out_of_its_unique_representation_is_reported() {
        let spoils = [
            (0, Spoil::NegativeZero),
            (2, Spoil::ZeroWithExponent),
            (1, Spoil::LowZero),
            (1, Spoil::HighZero),
        ];
        for (axis, spoil) in spoils {
            let mut buf = [0.0; 64];
            let mut scratch = Scratch::borrowed(&mut buf);
            assert_eq!(
                check(&NonCanonical { axis, spoil }, &mut scratch),
                Err(CheckError::Violation(Violation::NonCanonicalBig { axis })),
                "{spoil:?}"
            );
            assert_eq!(scratch.used(), 0);
        }
    }
}
