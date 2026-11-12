//! The uncertainty that the errors of the points of a call add to the
//! floating-point filter of a predicate.
//!
//! A predicate evaluates its filter on the approximations
//! ([`Point::approx`](crate::Point::approx)) of its points. Their errors
//! ([`Point::err`](crate::Point::err)) enter its error bound through the sum
//! of the two largest errors among the points: a difference of two
//! coordinates is off by at most the sum of their errors. These functions
//! compute that sum per axis ([`uncertainty`]), or once for every axis
//! ([`uncertainty_max`]).
//!
//! Both are on the fast path of every call with implicit points. They do
//! not allocate. Each of their comparisons selects one of its two operands,
//! and a NaN is tracked apart, so that the compiler can use minimum and
//! maximum instructions instead of branches that depend on the errors;
//! whether it does is up to the compiler, and is not guaranteed.
//! Calls on explicit points do not need them, since every error is zero:
//! the generated predicates use an uncertainty of zero instead when every
//! point is explicit ([`Point::is_explicit`](crate::Point::is_explicit)),
//! a test that the compiler resolves for the types where it is a constant.
//! Whether a call to these functions with errors that are the constant zero
//! folds away is up to the compiler, and is not guaranteed.
//!
//! The result is rounded up, so that it never falls below the exact sum. An
//! infinite error gives an infinite result and a NaN error a NaN result,
//! for the axes they lie on, and the filter then does not decide.

/// Per axis, the sum of the two largest errors among the points, rounded
/// up.
///
/// The errors are expected to be non-negative, as
/// [`Point::err`](crate::Point::err) guarantees. On each axis an infinite
/// error gives `+∞` and a NaN error gives NaN; the other axes are not
/// affected.
///
/// # Examples
///
/// ```
/// use implicit_predicates_core::filter::uncertainty;
///
/// let errs = [[1.0, 0.0, 0.5], [4.0, 0.0, 0.0], [2.0, 0.25, 0.0], [0.0; 3]];
/// assert_eq!(uncertainty(errs.each_ref()), [6.0, 0.25, 0.5]);
/// ```
#[inline]
#[must_use]
pub fn uncertainty<const N: usize>(errs: [&[f64; 3]; N]) -> [f64; 3] {
    let mut top = [TopTwo::ZERO; 3];
    for err in errs {
        for (top, &e) in top.iter_mut().zip(err) {
            top.add(e);
        }
    }
    top.map(TopTwo::sum)
}

/// The sum of the two largest values of `max_k err_k` among the points,
/// rounded up: one uncertainty for every axis, an upper bound on each
/// component of [`uncertainty`].
///
/// An infinite error gives `+∞` and a NaN error gives NaN.
///
/// # Examples
///
/// ```
/// use implicit_predicates_core::filter::uncertainty_max;
///
/// let errs = [[1.0, 0.0, 0.5], [0.0, 0.0, 4.0], [2.0, 0.25, 0.0], [0.0; 3]];
/// assert_eq!(uncertainty_max(errs.each_ref()), 6.0);
/// ```
#[inline]
#[must_use]
pub fn uncertainty_max<const N: usize>(errs: [&[f64; 3]; N]) -> f64 {
    let mut top = TopTwo::ZERO;
    for err in errs {
        let largest = max(max(err[0], err[1]), err[2]);
        top.add(largest);
        top.nan |= err[0].is_nan() | err[1].is_nan() | err[2].is_nan();
    }
    top.sum()
}

/// The two largest of the values added so far, starting from two zeros,
/// and whether a NaN was added.
#[derive(Clone, Copy)]
struct TopTwo {
    first: f64,
    second: f64,
    nan: bool,
}

impl TopTwo {
    const ZERO: TopTwo = TopTwo {
        first: 0.0,
        second: 0.0,
        nan: false,
    };

    /// Keeps the two largest of `first`, `second` and `x`, and records a
    /// NaN. Each comparison selects one of its operands, so that the
    /// compiler can use a minimum or maximum instruction instead of a
    /// branch; a NaN `x` may be dropped by them, but not from `nan`.
    #[inline]
    fn add(&mut self, x: f64) {
        self.second = max(self.second, min(self.first, x));
        self.first = max(self.first, x);
        self.nan |= x.is_nan();
    }

    /// `first + second`, rounded up, or NaN if a NaN was added.
    #[inline]
    fn sum(self) -> f64 {
        let sum = self.first + self.second;
        // The rounding error of the sum, exact as in Dekker's fast-two-sum
        // since `first >= second >= 0`; positive when the sum rounded down.
        // It is `-∞` when the sum overflows to `+∞`. When it is positive the
        // sum is finite and positive, and the next `f64` up is the next bit
        // pattern.
        let error = self.second - (sum - self.first);
        let up = f64::from_bits(sum.to_bits() + u64::from(error > 0.0));
        if self.nan { f64::NAN } else { up }
    }
}

/// The larger of `a` and `b`; `b` if they are unordered.
#[inline]
fn max(a: f64, b: f64) -> f64 {
    if a > b { a } else { b }
}

/// The smaller of `a` and `b`; `b` if they are unordered.
#[inline]
fn min(a: f64, b: f64) -> f64 {
    if a < b { a } else { b }
}
