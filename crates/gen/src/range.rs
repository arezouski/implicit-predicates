//! The range of leaf magnitudes and the buffer sizes of the exact stages.
//!
//! An exact stage evaluates a polynomial bottom up with floating-point
//! expansions: a product of two `f64` is `two_product`, an expansion times an
//! `f64` is a scaling, an expansion times an expansion is a sum of scalings,
//! and sums and differences are exact merges. Every exact product of the
//! arithmetic core needs both operands to be zero or to lie in its product
//! domain, and every value it handles must stay below `2^1020`. This module
//! derives the inputs for which that holds, and how long each temporary
//! expansion can get.

mod shape;

use crate::differences::DiffGraph;
use crate::dyadic::Dyadic;
use crate::graph::Graph;
use shape::Shape;

/// The exponent of the overflow limit of the arithmetic core: every
/// component and scalar operand of its expansion functions has magnitude
/// below `2^1020`.
const OVERFLOW_EXPONENT: i64 = 1020;

/// The magnitudes of leaves for which an exact stage is valid: every
/// nonzero leaf `x` must satisfy `min <= |x| <= max`. Both are powers of two.
///
/// A leaf is an input coordinate for the raw form, the computed difference
/// for a difference form with [`Leaves::One`], and each of the two
/// coordinates that are subtracted for a difference form with
/// [`Leaves::Two`].
///
/// # Why the range is sufficient
///
/// Write `m = min`, `M = max` and `u = 2^-53`.
///
/// **Grid.** A nonzero `f64` of magnitude at least `m`, a normal power of
/// two, is an integer multiple of `q = m 2^-52`; so is zero. For a power of
/// two `g`, the rounding of a multiple of `g` to `f64` is a multiple of `g`:
/// where the spacing of `f64` near the value is at least `g`, every `f64`
/// there is a multiple of `g`; where it is smaller, the value is
/// representable and rounds to itself. Every error-free transformation
/// returns a rounded value and the exact difference between the true result
/// and it, so with multiples of `g` as inputs (of `g_a g_b` for a product of
/// multiples of `g_a` and `g_b`) both outputs are multiples of `g`. Hence
/// every value an evaluation handles, every component of the expansion of a
/// node included, is a multiple of the node's *grid*: `q` for a leaf (also
/// for both components of a difference `two_diff(a, b)`, which can be far
/// below `m` after cancellation but are still multiples of `q`), the smaller
/// grid of the operands for a sum, and the product of the grids for a
/// product. A nonzero component is at least its grid in magnitude; for a
/// node of degree `g` in the leaves, the grid is `q^g`.
///
/// **Magnitude.** Each error-free transformation turns inputs `a`, `b`
/// into `x + y = r`, where `r` is `a + b` or `a b`, with `|y| <= u |x|`;
/// then `|x| <= |r| / (1 - u)` and `|x| + |y| <= (1 + 3u)(|a| + |b|)`, or
/// `(1 + 3u) |a| |b|` for a product. Follow the multiset of values an
/// evaluation holds (pending inputs, with the factor of a scaling applied,
/// and outputs): its sum of magnitudes grows by at most a factor `1 + 3u` per
/// transformation, and the output expansion is part of it. So the sum of the
/// magnitudes of the components of a node, and every value handled on the
/// way, is at most `(1 + 3u)^T S <= (1 + 6uT) S`, where `T <= 2^51` is the
/// number of transformations the node depends on, counted with multiplicity
/// (see the operation counts below), and `S` follows the structure: `M` for
/// a coordinate or a computed difference, `2M` for the two components of a
/// difference of coordinates, `|n|` for a constant `n`, the sum of the
/// operands' `S` for a sum or difference, their product for a product.
///
/// **Rule.** The range is the widest pair of powers of two within
/// `[operand_min, operand_max]` such that, for every operand of every
/// product, the grid is at least `operand_min` and `(1 + 6uT) S` is at most
/// `operand_max`, and `(1 + 6uT) S < 2^1020` at every node. Every factor
/// of every exact product is then zero or in the product domain, and no value
/// overflows. The grid condition depends only on `m` and the magnitude
/// conditions only on `M`, and both are monotone, so the two ends are found
/// separately.
///
/// The operation counts, with `l_x` the worst-case length of `x`: a
/// difference of two coordinates is one transformation; a sum or difference
/// at most `2 (l_x + l_y)`; a product at most `3 l_x l_y` for the exact
/// products of the scalings plus `4 l_x l_y (l_x + l_y)` for merging them,
/// whichever operand is scaled and in whatever order the scalings are
/// merged.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExactRange {
    /// The smallest admissible magnitude of a nonzero leaf.
    pub min: f64,
    /// The largest admissible magnitude of a leaf.
    pub max: f64,
}

/// What an exact stage needs: the range of its leaves and the size of its
/// temporaries.
#[derive(Clone, Debug, PartialEq)]
pub struct ExactPlan {
    /// The magnitudes of the leaves for which the stage is valid.
    pub range: ExactRange,
    /// The worst-case length of the expansion of each node, in the order of
    /// the graph: 1 or 2 for a leaf (see [`Leaves`]), 1 for a nonzero
    /// constant and 0 for zero, `l_x + l_y` for a sum or difference,
    /// `2 l_x l_y` for a product and `l_x` for a negation.
    pub lengths: Vec<usize>,
    /// The sum of `lengths`: the size of all the temporaries, each shared
    /// node counted once.
    pub total: usize,
}

/// The components of each leaf of a difference form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leaves {
    /// One: every difference was computed exactly in `f64`.
    One,
    /// Two: each difference is the exact `two_diff` of two coordinates.
    Two,
}

/// Derives the exact stage of the raw form, whose leaves are the input
/// coordinates, for the product domain `[operand_min, operand_max]`.
///
/// See [`ExactRange`] for the rule and why it is sound.
///
/// # Panics
///
/// Panics if `operand_min` and `operand_max` are not normal powers of two
/// with `operand_min <= operand_max`, if no range exists, if a constant is
/// not exact in `f64`, or if a length overflows `usize`.
pub fn exact_plan_raw(graph: &Graph, operand_min: f64, operand_max: f64) -> ExactPlan {
    plan(&Shape::raw(graph), operand_min, operand_max)
}

/// Derives the exact stage of a difference form whose leaves have the given
/// number of components, for the product domain `[operand_min, operand_max]`.
///
/// See [`ExactRange`] for the rule and why it is sound.
///
/// # Panics
///
/// As [`exact_plan_raw`].
pub fn exact_plan_differences(
    graph: &DiffGraph,
    leaves: Leaves,
    operand_min: f64,
    operand_max: f64,
) -> ExactPlan {
    plan(&Shape::differences(graph, leaves), operand_min, operand_max)
}

fn plan(shape: &Shape, operand_min: f64, operand_max: f64) -> ExactPlan {
    let (low, high) = (exponent(operand_min), exponent(operand_max));
    assert!(low <= high, "the product domain is empty");
    let min = first_failure(low, high, |m| !grids_fit(shape, m, low));
    let max = first_failure(low, high, |big_m| magnitudes_fit(shape, big_m, high)) - 1;
    assert!(
        min <= high && max >= low && min <= max,
        "the exact stage has no range: nonzero leaves need a magnitude of at \
         least 2^{min} for the products to stay in the domain, and at most \
         2^{max} to avoid leaving it or overflowing"
    );
    let total = shape
        .lengths
        .iter()
        .try_fold(0usize, |sum, &n| sum.checked_add(n))
        .expect("the total length overflows usize");
    ExactPlan {
        range: ExactRange {
            min: Dyadic::pow2(min).round_up(),
            max: Dyadic::pow2(max).round_up(),
        },
        lengths: shape.lengths.clone(),
        total,
    }
}

/// Whether, with nonzero leaves of magnitude at least `2^m`, every nonzero
/// component of every product operand is at least `2^low`.
fn grids_fit(shape: &Shape, m: i64, low: i64) -> bool {
    let grids = shape.grids(m);
    shape
        .operands()
        .all(|x| grids[x].is_none_or(|grid| grid >= low))
}

/// Whether, with leaves of magnitude at most `2^big_m`, every component of
/// every product operand is at most `2^high` and every value is below the
/// overflow limit.
fn magnitudes_fit(shape: &Shape, big_m: i64, high: i64) -> bool {
    let bounds = shape.component_bounds(big_m);
    let overflow = Dyadic::pow2(OVERFLOW_EXPONENT);
    let operand_max = Dyadic::pow2(high);
    bounds.iter().all(|bound| *bound < overflow)
        && shape.operands().all(|x| bounds[x] <= operand_max)
}

/// The first `e` in `low..=high` for which `holds(e)` is false, or
/// `high + 1`, for a `holds` that is true up to some point and false after.
fn first_failure(low: i64, high: i64, holds: impl Fn(i64) -> bool) -> i64 {
    let (mut low, mut high) = (low, high + 1);
    while low < high {
        let middle = low + (high - low) / 2;
        if holds(middle) {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    low
}

/// The exponent of a normal power of two.
fn exponent(x: f64) -> i64 {
    assert!(
        x.is_normal() && x > 0.0 && x.to_bits() & ((1 << 52) - 1) == 0,
        "a bound of the product domain must be a normal power of two, not {x}"
    );
    i64::try_from(x.to_bits() >> 52).expect("11 bits") - 1023
}

#[cfg(test)]
mod tests;
