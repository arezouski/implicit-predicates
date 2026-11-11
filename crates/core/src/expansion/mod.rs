//! Floating-point expansions: exact values represented as sums of `f64`
//! components.
//!
//! An *expansion* is a slice of `f64` whose exact, unrounded sum is the value
//! it represents. Every function here requires its expansion inputs to
//! satisfy, and guarantees for its output, these invariants:
//!
//! - no component is zero, so the value zero is the empty slice;
//! - components are sorted by strictly increasing magnitude;
//! - components are *nonoverlapping*: the highest set bit of each lies
//!   strictly below the lowest set bit of the next.
//!
//! The components below the last one then add up to less than it in
//! magnitude, so the last component has the sign of the value. Expansions
//! built only with this module always satisfy the invariants.
//!
//! Results are written to the start of a caller-provided buffer `out`, and
//! the length written is returned. Each function documents its *capacity*,
//! the longest result it can produce. Debug builds panic if `out` is shorter
//! than the capacity; release builds panic only if the actual result does not
//! fit. Nothing is ever written outside `out`, and nothing allocates.
//!
//! The functions also require that:
//!
//! - every input component and scalar operand has magnitude below `2^1020`,
//!   which rules out overflow;
//! - operands of products ([`from_two_product`] and [`scale`]) lie in the
//!   product domain of [`eft::in_product_domain`](crate::eft::in_product_domain),
//!   which debug builds check;
//! - the floating-point rounding mode is the default round-to-nearest, which
//!   the correctness proofs depend on.
//!
//! These magnitude bounds are not closed under chaining: the output of an
//! operation whose inputs satisfy them can have components outside the
//! product domain or close to overflow, so a computation that chains
//! operations must bound the magnitudes it produces.
//!
//! Gradual underflow is harmless: sums of floating-point values are exact
//! whenever they are subnormal.
//!
//! The algorithms are from Shewchuk, "Adaptive Precision Floating-Point
//! Arithmetic and Fast Robust Geometric Predicates" (1997), all with zero
//! elimination: [`grow`] is GROW-EXPANSION (Theorem 10), [`sum`] and
//! [`diff`] are LINEAR-EXPANSION-SUM (Theorem 24) and [`scale`] is
//! SCALE-EXPANSION (Theorem 19). For each one the paper proves that
//! nonoverlapping inputs give a nonoverlapping output.

mod linear_sum;
mod output;
mod validity;

pub(crate) use validity::is_valid;

use crate::Sign;
use crate::eft::{fast_two_sum, two_diff, two_product, two_sum};
use linear_sum::linear_expansion_sum;
use output::Output;

/// Writes the expansion of `a`: empty if `a` is zero of either sign, `[a]`
/// otherwise.
///
/// Capacity: 1. Requirement: `a` is finite; debug builds panic if it is not.
///
/// # Examples
///
/// ```
/// use implicit_predicates_core::expansion;
///
/// let mut out = [0.0; 1];
/// assert_eq!(expansion::from_f64(2.5, &mut out), 1);
/// assert_eq!(out, [2.5]);
/// assert_eq!(expansion::from_f64(-0.0, &mut out), 0);
/// ```
#[must_use = "the returned length tells which part of `out` holds the result"]
pub fn from_f64(a: f64, out: &mut [f64]) -> usize {
    debug_assert!(
        a.is_finite(),
        "expansion::from_f64 of {a}, which is not finite"
    );
    let mut out = Output::new(out, 1);
    out.push(a);
    out.finish()
}

/// Writes the expansion of the exact sum `a + b`.
///
/// Capacity: 2. Requirement: `a` and `b` have magnitude below `2^1020`.
///
/// # Examples
///
/// ```
/// use implicit_predicates_core::expansion;
///
/// let mut out = [0.0; 2];
/// let len = expansion::from_two_sum(1.0, 1e-20, &mut out);
/// assert_eq!(&out[..len], &[1e-20, 1.0]);
/// ```
#[must_use = "the returned length tells which part of `out` holds the result"]
pub fn from_two_sum(a: f64, b: f64, out: &mut [f64]) -> usize {
    from_rounded_pair(two_sum(a, b), out)
}

/// Writes the expansion of the exact difference `a - b`.
///
/// Capacity: 2. Requirement: `a` and `b` have magnitude below `2^1020`.
///
/// # Examples
///
/// ```
/// use implicit_predicates_core::expansion;
///
/// let mut out = [0.0; 2];
/// let len = expansion::from_two_diff(1.0, 1e-20, &mut out);
/// assert_eq!(&out[..len], &[-1e-20, 1.0]);
/// ```
#[must_use = "the returned length tells which part of `out` holds the result"]
pub fn from_two_diff(a: f64, b: f64, out: &mut [f64]) -> usize {
    from_rounded_pair(two_diff(a, b), out)
}

/// Writes the expansion of the exact product `a * b`.
///
/// Capacity: 2. Requirement: `a` and `b` lie in the product domain (see
/// [`eft::in_product_domain`](crate::eft::in_product_domain)); debug builds
/// panic if they do not.
///
/// # Examples
///
/// ```
/// use implicit_predicates_core::expansion;
///
/// let a = 1.0 + f64::EPSILON;
/// let mut out = [0.0; 2];
/// let len = expansion::from_two_product(a, a, &mut out);
/// assert_eq!(&out[..len], &[f64::EPSILON * f64::EPSILON, 1.0 + 2.0 * f64::EPSILON]);
/// ```
#[must_use = "the returned length tells which part of `out` holds the result"]
pub fn from_two_product(a: f64, b: f64, out: &mut [f64]) -> usize {
    from_rounded_pair(two_product(a, b), out)
}

/// Writes the expansion of `e + b`.
///
/// Capacity: `e.len() + 1`. Requirements: `e` is a valid expansion (see the
/// [module documentation](self)) and `b` has magnitude below `2^1020`
/// (debug builds panic if `b` is not finite). Guarantee: the result is a
/// valid expansion.
///
/// # Examples
///
/// ```
/// use implicit_predicates_core::expansion;
///
/// let mut out = [0.0; 3];
/// let len = expansion::grow(&[1e-20, 1.0], 1e20, &mut out);
/// assert_eq!(&out[..len], &[1e-20, 1.0, 1e20]);
/// ```
#[must_use = "the returned length tells which part of `out` holds the result"]
pub fn grow(e: &[f64], b: f64, out: &mut [f64]) -> usize {
    debug_assert!(b.is_finite(), "expansion::grow by {b}, which is not finite");
    let mut out = Output::new(out, e.len() + 1);
    let mut accumulator = b;
    for &component in e {
        let (next, low) = two_sum(accumulator, component);
        out.push(low);
        accumulator = next;
    }
    out.push(accumulator);
    out.finish()
}

/// Writes the expansion of `e + f`.
///
/// Capacity: `e.len() + f.len()`. Requirement: `e` and `f` are valid
/// expansions (see the [module documentation](self)). Guarantee: the result
/// is a valid expansion.
///
/// This is the linear-time LINEAR-EXPANSION-SUM rather than the faster
/// FAST-EXPANSION-SUM because its nonoverlapping guarantee holds for any
/// nonoverlapping inputs.
///
/// # Examples
///
/// ```
/// use implicit_predicates_core::expansion;
///
/// let mut out = [0.0; 4];
/// let len = expansion::sum(&[1e-20, 1.0], &[-1e-20, 2.0], &mut out);
/// assert_eq!(&out[..len], &[3.0]);
/// ```
#[must_use = "the returned length tells which part of `out` holds the result"]
pub fn sum(e: &[f64], f: &[f64], out: &mut [f64]) -> usize {
    linear_expansion_sum(e, f, false, Output::new(out, e.len() + f.len()))
}

/// Writes the expansion of `e - f`, reading `f` without copying or modifying
/// it.
///
/// Capacity: `e.len() + f.len()`. Requirement: `e` and `f` are valid
/// expansions (see the [module documentation](self)). Guarantee: the result
/// is a valid expansion. The algorithm is that of [`sum`].
///
/// # Examples
///
/// ```
/// use implicit_predicates_core::expansion;
///
/// let mut out = [0.0; 4];
/// let len = expansion::diff(&[1e-20, 1.0], &[1e-20, 2.0], &mut out);
/// assert_eq!(&out[..len], &[-1.0]);
/// ```
#[must_use = "the returned length tells which part of `out` holds the result"]
pub fn diff(e: &[f64], f: &[f64], out: &mut [f64]) -> usize {
    linear_expansion_sum(e, f, true, Output::new(out, e.len() + f.len()))
}

/// Writes the expansion of `e * b`.
///
/// Capacity: `2 * e.len()`. Requirements: `e` is a valid expansion (see the
/// [module documentation](self)), and each of its components and `b` lie in
/// the product domain (see
/// [`eft::in_product_domain`](crate::eft::in_product_domain)); debug builds
/// panic if they do not. Guarantee: the result is a valid expansion.
///
/// # Examples
///
/// ```
/// use implicit_predicates_core::expansion;
///
/// let mut out = [0.0; 4];
/// let len = expansion::scale(&[0.25, 4.0], 3.0, &mut out);
/// assert_eq!(&out[..len], &[12.75]);
/// ```
#[must_use = "the returned length tells which part of `out` holds the result"]
pub fn scale(e: &[f64], b: f64, out: &mut [f64]) -> usize {
    let mut out = Output::new(out, 2 * e.len());
    let Some((&first, rest)) = e.split_first() else {
        return out.finish();
    };
    let (mut accumulator, low) = two_product(first, b);
    out.push(low);
    for &component in rest {
        let (product, product_low) = two_product(component, b);
        let (partial, partial_low) = two_sum(accumulator, product_low);
        out.push(partial_low);
        // Theorem 19 of the paper shows `|product| >= |partial|`.
        let (next, next_low) = fast_two_sum(product, partial);
        out.push(next_low);
        accumulator = next;
    }
    out.push(accumulator);
    out.finish()
}

/// Negates the valid expansion `e` in place. Negation is exact and keeps
/// the invariants.
///
/// # Examples
///
/// ```
/// use implicit_predicates_core::expansion;
///
/// let mut e = [1e-20, -1.0];
/// expansion::negate(&mut e);
/// assert_eq!(e, [-1e-20, 1.0]);
/// ```
pub fn negate(e: &mut [f64]) {
    for component in e {
        *component = -*component;
    }
}

/// Returns the exact sign of the value of the valid expansion `e` (see the
/// [module documentation](self)): the sign of its last component, which
/// outweighs all the others together.
///
/// # Examples
///
/// ```
/// use implicit_predicates_core::{Sign, expansion};
///
/// assert_eq!(expansion::sign(&[1e-20, -1.0]), Sign::Negative);
/// assert_eq!(expansion::sign(&[]), Sign::Zero);
/// ```
#[must_use]
pub fn sign(e: &[f64]) -> Sign {
    e.last().map_or(Sign::Zero, |&largest| Sign::of(largest))
}

/// Returns a floating-point approximation of the value of the valid
/// expansion `e`: its components added from smallest to largest. Use
/// [`sign`] for the exact sign.
///
/// # Examples
///
/// ```
/// use implicit_predicates_core::expansion;
///
/// assert_eq!(expansion::estimate(&[1e-20, 1.0]), 1.0);
/// assert_eq!(expansion::estimate(&[]), 0.0);
/// ```
#[must_use]
pub fn estimate(e: &[f64]) -> f64 {
    e.iter().fold(0.0, |total, &component| total + component)
}

/// Writes `(x, y)`, a rounded result `x` and its exact error `y`, as an
/// expansion. The paper shows that the error of TWO-SUM and of TWO-PRODUCT
/// is too small to overlap the rounded result.
fn from_rounded_pair((x, y): (f64, f64), out: &mut [f64]) -> usize {
    let mut out = Output::new(out, 2);
    out.push(y);
    out.push(x);
    out.finish()
}
