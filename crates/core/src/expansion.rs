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
//! - operands of products ([`from_two_product`] and [`scale`]) are zero or
//!   have magnitude in `2^-480 ..= 2^480`, the domain of
//!   [`eft::two_product`](crate::eft::two_product);
//! - the floating-point rounding mode is the default round-to-nearest, which
//!   the correctness proofs depend on.
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

use crate::Sign;
use crate::eft::{fast_two_sum, two_diff, two_product, two_sum};

/// Writes the expansion of `a`: empty if `a` is zero of either sign, `[a]`
/// otherwise.
///
/// Capacity: 1. Requirement: `a` is finite.
///
/// # Examples
///
/// ```
/// use indirect_predicates_core::expansion;
///
/// let mut out = [0.0; 1];
/// assert_eq!(expansion::from_f64(2.5, &mut out), 1);
/// assert_eq!(out, [2.5]);
/// assert_eq!(expansion::from_f64(-0.0, &mut out), 0);
/// ```
pub fn from_f64(a: f64, out: &mut [f64]) -> usize {
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
/// use indirect_predicates_core::expansion;
///
/// let mut out = [0.0; 2];
/// let len = expansion::from_two_sum(1.0, 1e-20, &mut out);
/// assert_eq!(&out[..len], &[1e-20, 1.0]);
/// ```
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
/// use indirect_predicates_core::expansion;
///
/// let mut out = [0.0; 2];
/// let len = expansion::from_two_diff(1.0, 1e-20, &mut out);
/// assert_eq!(&out[..len], &[-1e-20, 1.0]);
/// ```
pub fn from_two_diff(a: f64, b: f64, out: &mut [f64]) -> usize {
    from_rounded_pair(two_diff(a, b), out)
}

/// Writes the expansion of the exact product `a * b`.
///
/// Capacity: 2. Requirement: each of `a` and `b` is zero or has magnitude in
/// `2^-480 ..= 2^480`.
///
/// # Examples
///
/// ```
/// use indirect_predicates_core::expansion;
///
/// let a = 1.0 + f64::EPSILON;
/// let mut out = [0.0; 2];
/// let len = expansion::from_two_product(a, a, &mut out);
/// assert_eq!(&out[..len], &[f64::EPSILON * f64::EPSILON, 1.0 + 2.0 * f64::EPSILON]);
/// ```
pub fn from_two_product(a: f64, b: f64, out: &mut [f64]) -> usize {
    from_rounded_pair(two_product(a, b), out)
}

/// Writes the expansion of `e + b`.
///
/// Capacity: `e.len() + 1`. Requirements: `e` is a valid expansion (see the
/// [module documentation](self)) and `b` has magnitude below `2^1020`.
/// Guarantee: the result is a valid expansion.
///
/// # Examples
///
/// ```
/// use indirect_predicates_core::expansion;
///
/// let mut out = [0.0; 3];
/// let len = expansion::grow(&[1e-20, 1.0], 1e20, &mut out);
/// assert_eq!(&out[..len], &[1e-20, 1.0, 1e20]);
/// ```
pub fn grow(e: &[f64], b: f64, out: &mut [f64]) -> usize {
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
/// use indirect_predicates_core::expansion;
///
/// let mut out = [0.0; 4];
/// let len = expansion::sum(&[1e-20, 1.0], &[-1e-20, 2.0], &mut out);
/// assert_eq!(&out[..len], &[3.0]);
/// ```
pub fn sum(e: &[f64], f: &[f64], out: &mut [f64]) -> usize {
    let out = Output::new(out, e.len() + f.len());
    sum_in_magnitude_order(MergedComponents::new(e, f, false), out)
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
/// use indirect_predicates_core::expansion;
///
/// let mut out = [0.0; 4];
/// let len = expansion::diff(&[1e-20, 1.0], &[1e-20, 2.0], &mut out);
/// assert_eq!(&out[..len], &[-1.0]);
/// ```
pub fn diff(e: &[f64], f: &[f64], out: &mut [f64]) -> usize {
    let out = Output::new(out, e.len() + f.len());
    sum_in_magnitude_order(MergedComponents::new(e, f, true), out)
}

/// Writes the expansion of `e * b`.
///
/// Capacity: `2 * e.len()`. Requirements: `e` is a valid expansion (see the
/// [module documentation](self)), and each of its components and `b` is zero
/// or has magnitude in `2^-480 ..= 2^480`. Guarantee: the result is a valid
/// expansion.
///
/// # Examples
///
/// ```
/// use indirect_predicates_core::expansion;
///
/// let mut out = [0.0; 4];
/// let len = expansion::scale(&[0.25, 4.0], 3.0, &mut out);
/// assert_eq!(&out[..len], &[12.75]);
/// ```
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
/// use indirect_predicates_core::expansion;
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
/// use indirect_predicates_core::{Sign, expansion};
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
/// use indirect_predicates_core::expansion;
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

/// LINEAR-EXPANSION-SUM with zero elimination: adds the components of two
/// expansions, already merged by increasing magnitude.
///
/// `high + low` is a running approximation of the sum of the components seen
/// so far; the rest of that sum has already been written out.
fn sum_in_magnitude_order(mut components: MergedComponents<'_>, mut out: Output<'_>) -> usize {
    let Some(first) = components.next() else {
        return out.finish();
    };
    let Some(second) = components.next() else {
        out.push(first);
        return out.finish();
    };
    // The merge order guarantees `|second| >= |first|`.
    let (mut high, mut low) = fast_two_sum(second, first);
    for component in components {
        // The proof of Theorem 24 shows `|low| <= ulp(component)`, so
        // `|low| <= |component|`.
        let (rounded, error) = fast_two_sum(component, low);
        out.push(error);
        (high, low) = two_sum(high, rounded);
    }
    out.push(low);
    out.push(high);
    out.finish()
}

/// The components of two expansions `e` and `f`, merged by increasing
/// magnitude, with those of `f` negated if requested.
struct MergedComponents<'a> {
    e: &'a [f64],
    f: &'a [f64],
    negate_f: bool,
}

impl<'a> MergedComponents<'a> {
    fn new(e: &'a [f64], f: &'a [f64], negate_f: bool) -> Self {
        MergedComponents { e, f, negate_f }
    }
}

impl Iterator for MergedComponents<'_> {
    type Item = f64;

    fn next(&mut self) -> Option<f64> {
        let take_from_e = match (self.e.first(), self.f.first()) {
            (Some(a), Some(b)) => a.abs() <= b.abs(),
            (Some(_), None) => true,
            (None, _) => false,
        };
        if take_from_e {
            let (&component, rest) = self.e.split_first()?;
            self.e = rest;
            Some(component)
        } else {
            let (&component, rest) = self.f.split_first()?;
            self.f = rest;
            Some(if self.negate_f { -component } else { component })
        }
    }
}

/// The output buffer of an operation, filled from the start with the nonzero
/// components pushed to it.
struct Output<'a> {
    buffer: &'a mut [f64],
    len: usize,
}

impl<'a> Output<'a> {
    /// Wraps `buffer`, checking in debug builds that it holds `capacity`
    /// components.
    fn new(buffer: &'a mut [f64], capacity: usize) -> Self {
        debug_assert!(
            buffer.len() >= capacity,
            "output buffer holds {} components but the capacity required is {capacity}",
            buffer.len()
        );
        Output { buffer, len: 0 }
    }

    /// Appends `component` unless it is zero (zero elimination). Panics if
    /// the buffer is full.
    fn push(&mut self, component: f64) {
        if component != 0.0 {
            self.buffer[self.len] = component;
            self.len += 1;
        }
    }

    /// The number of components written.
    fn finish(self) -> usize {
        self.len
    }
}
