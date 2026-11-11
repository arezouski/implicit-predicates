//! The floating-point filter of a predicate.
//!
//! The emitted `filter` evaluates the difference form in `f64` and returns
//! its sign when the error bound derived by [`crate::filter_bound`] proves
//! it. It computes, in this order:
//!
//! - each leaf, `p[i][axis] - p[j][axis]`, once;
//! - each node of the difference form, one statement per node in the order
//!   of the graph, with the operation the node names: no reassociation, and
//!   a negation as a unary minus, which is exact;
//! - `delta`, the largest magnitude of the leaves, by comparisons, which are
//!   exact;
//! - `d = delta + e` and `bound = (EPSILON * d + KAPPA * e) * d * ... * d`,
//!   with `DEGREE - 1` multiplications by `d`, from left to right;
//! - the decision `D_MIN <= d <= D_MAX` and `|det| > bound`, combined with
//!   a non-short-circuiting `&`; the sign is then chosen by a comparison,
//!   not a branch on the decision.
//!
//! # Why the decision is sound
//!
//! Let `B = (epsilon D + kappa E) D^(k - 1)` be the exact bound, for the
//! exact `D = delta + E` and the exact coefficients. The value of the form at
//! the exact points differs from `det` by at most `(1 + 2^-32) B` whenever
//! `D` is in `[D_MIN, D_MAX]` (see [`crate::FilterBound::constants`]).
//!
//! The emitted bound is computed in `f64` with round-to-nearest, and each of
//! its operations can round down by a factor `1 - u`, `u = 2^-53`. The
//! operations are, for a degree `k`: the sum of the two largest errors of
//! the points that forms `E`, done by the caller; `delta + e`; `EPSILON *
//! d`; `KAPPA * e`; their sum; and the `k - 1` products by `d`: `k + 4` in
//! all, within the `2k + 8` that the constants allow for. `EPSILON` and
//! `KAPPA` are the exact coefficients times `1 + 2^-30`, rounded up, and
//! `(1 + 2^-30)(1 - u)^(2k + 8) >= 1 + 2^-31` for every degree the
//! generator accepts, so the computed bound is at least `(1 + 2^-31) B`,
//! above the error. The partial results are formed from the largest one
//! down, so for `D >= D_MIN` none of them leaves the normal range on the way
//! (the term `KAPPA * e` may underflow, which loses at most `2^-1075`, a
//! fraction `2^-54` of the sum it joins and within the same allowance).
//! `d` is compared with `D_MIN` and `D_MAX` after rounding, which can only
//! make it smaller than `D` by the factor already counted.
//!
//! Non-finite inputs never decide: a NaN leaf makes `det` NaN, an infinite
//! leaf makes `delta` and `d` infinite, a non-finite `e` makes `d` infinite
//! or NaN, and every comparison with NaN is false. A decision implies
//! `|det| > bound > 0`, so the filter never returns zero.

use std::fmt::Write;

use super::comment;
use crate::differences::{DiffGraph, DiffNode};
use crate::graph::NodeId;

/// The number of `f64` operations that form the emitted bound of a filter
/// of degree `degree`, including the sum of the two largest errors done by
/// the caller.
pub(crate) fn bound_operations(degree: u32) -> u32 {
    // E, d, EPSILON * d, KAPPA * e, the sum, and the products by d.
    1 + 1 + 3 + (degree - 1)
}

/// The statements that compute `d` and `bound`.
pub(crate) fn bound_statements(degree: u32) -> String {
    let mut bound = String::from("(EPSILON * d + KAPPA * e)");
    for _ in 1..degree {
        bound.push_str(" * d");
    }
    format!("let d = delta + e;\nlet bound = {bound};\n")
}

/// Writes the `filter` function of a difference form with `points` points,
/// and tells whether it uses the helper of [`write_max`].
pub(crate) fn write_filter(
    out: &mut String,
    graph: &DiffGraph,
    points: usize,
    degree: u32,
) -> bool {
    let name = |id: NodeId| {
        if id == graph.root {
            "det".to_string()
        } else {
            format!("t{id}")
        }
    };
    comment(
        out,
        "///",
        0,
        "The sign of the predicate from an evaluation of its main form in `f64`, \
         when the rounding error cannot change it.\n\n\
         `e` is the sum of the two largest errors of the coordinates of the points \
         given, as approximations of the exact points; it is `0.0` for explicit points. \
         `None` means that this stage cannot decide: the value is too close to zero for \
         its error bound, the scale of the differences is outside `D_MIN..=D_MAX`, or an \
         input is not finite. The result is never `Some(Sign::Zero)`.",
    );
    out.push_str("#[inline]\n");
    writeln!(
        out,
        "pub fn filter(p: [&[f64; 3]; {points}], e: f64) -> Option<Sign> {{"
    )
    .unwrap();
    let mut leaves = Vec::new();
    for (id, node) in graph.nodes.iter().enumerate() {
        let value = match *node {
            DiffNode::Leaf {
                axis,
                minuend,
                subtrahend,
            } => {
                leaves.push(id);
                format!("p[{minuend}][{axis}] - p[{subtrahend}][{axis}]")
            }
            DiffNode::Neg(x) => format!("-{}", name(x)),
            DiffNode::Add(x, y) => format!("{} + {}", name(x), name(y)),
            DiffNode::Sub(x, y) => format!("{} - {}", name(x), name(y)),
            DiffNode::Mul(x, y) => format!("{} * {}", name(x), name(y)),
        };
        writeln!(out, "let {} = {value};", name(id)).unwrap();
    }
    comment(
        out,
        "//",
        4,
        "A NaN leaf can be skipped by the comparisons, but it makes `det` NaN.",
    );
    for (i, &leaf) in leaves.iter().enumerate() {
        if i == 0 {
            writeln!(out, "let delta = {}.abs();", name(leaf)).unwrap();
        } else {
            writeln!(out, "let delta = max(delta, {}.abs());", name(leaf)).unwrap();
        }
    }
    comment(
        out,
        "//",
        4,
        &format!(
            "The bound takes {} f64 operations, counting the sum that forms `e`.",
            bound_operations(degree)
        ),
    );
    out.push_str(&bound_statements(degree));
    out.push_str(
        "let decided = (D_MIN..=D_MAX).contains(&d) & (det.abs() > bound);\n\
         let sign = if det > 0.0 { Sign::Positive } else { Sign::Negative };\n\
         decided.then_some(sign)\n}\n\n",
    );
    leaves.len() > 1
}

/// Writes the helper `max` that the filter uses.
pub(crate) fn write_max(out: &mut String) {
    comment(
        out,
        "///",
        0,
        "The larger of `a` and `b`, or `b` if they are unordered.",
    );
    out.push_str(
        "#[inline]\n\
         fn max(a: f64, b: f64) -> f64 {\n\
         if a > b { a } else { b }\n\
         }\n\n",
    );
}
