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
//! Both are on the fast path of every call with implicit points: they do
//! not allocate, and they use selects rather than data-dependent branches.
//! For explicit points, whose errors are the constant zero, they fold away.
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
        top.add(max(max(err[0], err[1]), err[2]));
    }
    top.sum()
}

/// The two largest of the values added so far, starting from two zeros.
#[derive(Clone, Copy)]
struct TopTwo {
    first: f64,
    second: f64,
}

impl TopTwo {
    const ZERO: TopTwo = TopTwo {
        first: 0.0,
        second: 0.0,
    };

    /// Keeps the two largest of `first`, `second` and `x`. Once a NaN is
    /// added, both stay NaN.
    #[inline]
    fn add(&mut self, x: f64) {
        self.second = max(self.second, min(self.first, x));
        self.first = max(self.first, x);
    }

    /// `first + second`, rounded up.
    #[inline]
    fn sum(self) -> f64 {
        let sum = self.first + self.second;
        // The rounding error of the sum, exact as in Dekker's fast-two-sum
        // since `first >= second >= 0`; positive when the sum rounded down.
        // It is `-∞` when the sum overflows to `+∞`, and NaN with a NaN.
        let error = self.second - (sum - self.first);
        if error > 0.0 { sum.next_up() } else { sum }
    }
}

/// The larger of `a` and `b`, or NaN if either is NaN.
#[inline]
fn max(a: f64, b: f64) -> f64 {
    if a > b {
        a
    } else if b >= a {
        b
    } else {
        a + b
    }
}

/// The smaller of `a` and `b`, or NaN if either is NaN.
#[inline]
fn min(a: f64, b: f64) -> f64 {
    if a < b {
        a
    } else if b <= a {
        b
    } else {
        a + b
    }
}
