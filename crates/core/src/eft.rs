//! Error-free transformations: floating-point sums and products together with
//! their exact rounding errors.
//!
//! Every function returns a pair `(x, y)` where `x` is the correctly rounded
//! floating-point result of the operation and `y` its rounding error, so that
//! `x + y` equals the real result exactly. This holds under the requirement
//! documented on each function, assuming the default round-to-nearest-even
//! mode. NaN and infinite inputs are not supported.
//!
//! The algorithms are those of Knuth, Dekker and Veltkamp as presented by
//! Shewchuk, "Adaptive Precision Floating-Point Arithmetic and Fast Robust
//! Geometric Predicates" (1997).

/// `2^27 + 1`: multiplying by it splits an `f64` into two halves of at most
/// 26 significant bits each (Veltkamp's splitting).
const SPLITTER: f64 = 134_217_729.0;

/// Computes `a + b` and its rounding error (Knuth's two-sum).
///
/// Requirement: `a + b` does not overflow.
///
/// # Examples
///
/// ```
/// use indirect_predicates_core::eft::two_sum;
///
/// let (x, y) = two_sum(1.0, 1e-20);
/// assert_eq!(x, 1.0);
/// assert_eq!(y, 1e-20);
/// ```
#[inline]
#[must_use]
pub fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let x = a + b;
    let b_virtual = x - a;
    let a_virtual = x - b_virtual;
    let b_round = b - b_virtual;
    let a_round = a - a_virtual;
    (x, a_round + b_round)
}

/// Computes `a + b` and its rounding error, faster than [`two_sum`] when the
/// magnitudes are known to be ordered (Dekker's fast-two-sum).
///
/// Requirement: `|a| >= |b|` or `a == 0`, and `a + b` does not overflow.
///
/// # Examples
///
/// ```
/// use indirect_predicates_core::eft::fast_two_sum;
///
/// let (x, y) = fast_two_sum(1.0, 1e-20);
/// assert_eq!(x, 1.0);
/// assert_eq!(y, 1e-20);
/// ```
#[inline]
#[must_use]
pub fn fast_two_sum(a: f64, b: f64) -> (f64, f64) {
    let x = a + b;
    let b_virtual = x - a;
    (x, b - b_virtual)
}

/// Computes `a - b` and its rounding error.
///
/// Requirement: `a - b` does not overflow.
///
/// # Examples
///
/// ```
/// use indirect_predicates_core::eft::two_diff;
///
/// let (x, y) = two_diff(1.0, 1e-20);
/// assert_eq!(x, 1.0);
/// assert_eq!(y, -1e-20);
/// ```
#[inline]
#[must_use]
pub fn two_diff(a: f64, b: f64) -> (f64, f64) {
    // Negation is exact, and `a + (-b)` rounds exactly like `a - b`.
    two_sum(a, -b)
}

/// Computes `a * b` and its rounding error with Dekker's algorithm, using
/// only additions and multiplications.
///
/// Requirement: each of `a` and `b` is zero or has magnitude in
/// `2^-480 ..= 2^480`.
///
/// # Examples
///
/// ```
/// use indirect_predicates_core::eft::two_product_dekker;
///
/// let a = 1.0 + f64::EPSILON;
/// let (x, y) = two_product_dekker(a, a);
/// assert_eq!(x, 1.0 + 2.0 * f64::EPSILON);
/// assert_eq!(y, f64::EPSILON * f64::EPSILON);
/// ```
#[inline]
#[must_use]
pub fn two_product_dekker(a: f64, b: f64) -> (f64, f64) {
    let x = a * b;
    let (a_hi, a_lo) = split(a);
    let (b_hi, b_lo) = split(b);
    let err1 = x - a_hi * b_hi;
    let err2 = err1 - a_lo * b_hi;
    let err3 = err2 - a_hi * b_lo;
    (x, a_lo * b_lo - err3)
}

/// Computes `a * b` and its rounding error with the crate's selected
/// implementation, currently [`two_product_dekker`].
///
/// Requirement: each of `a` and `b` is zero or has magnitude in
/// `2^-480 ..= 2^480`.
///
/// # Examples
///
/// ```
/// use indirect_predicates_core::eft::two_product;
///
/// let a = 1.0 + f64::EPSILON;
/// let (x, y) = two_product(a, 1.0 - f64::EPSILON);
/// assert_eq!(x, 1.0);
/// assert_eq!(y, -f64::EPSILON * f64::EPSILON);
/// ```
#[inline]
#[must_use]
pub fn two_product(a: f64, b: f64) -> (f64, f64) {
    two_product_dekker(a, b)
}

/// Computes `a * a` and its rounding error, with fewer operations than
/// [`two_product_dekker`].
///
/// Requirement: `a` is zero or has magnitude in `2^-480 ..= 2^480`.
///
/// # Examples
///
/// ```
/// use indirect_predicates_core::eft::two_square;
///
/// let a = 1.0 + f64::EPSILON;
/// let (x, y) = two_square(a);
/// assert_eq!(x, 1.0 + 2.0 * f64::EPSILON);
/// assert_eq!(y, f64::EPSILON * f64::EPSILON);
/// ```
#[inline]
#[must_use]
pub fn two_square(a: f64) -> (f64, f64) {
    let x = a * a;
    let (hi, lo) = split(a);
    let err1 = x - hi * hi;
    // `hi + hi` is exact, and `hi * lo` counts twice in `a * a`.
    let err3 = err1 - (hi + hi) * lo;
    (x, lo * lo - err3)
}

/// Veltkamp's splitting: returns `(hi, lo)` with `hi + lo == a` exactly and
/// each half fitting in 26 significant bits, so that products of halves are
/// exact. Requires `|a| * SPLITTER` not to overflow.
#[inline]
fn split(a: f64) -> (f64, f64) {
    let c = SPLITTER * a;
    let a_big = c - a;
    let hi = c - a_big;
    (hi, a - hi)
}
