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
//! - the decision `e >= 0`, `D_MIN <= d <= D_MAX` and `|det| > bound`,
//!   combined with a non-short-circuiting `&`; the sign is then chosen by a
//!   comparison, not a branch on the decision.
//!
//! # Why the decision is sound
//!
//! Let `B = (epsilon D + kappa E) D^(k - 1)` be the exact bound, for the
//! exact `D = delta + E` and the exact coefficients. The value of the form at
//! the exact points differs from `det` by at most `(1 + 2^-32) B` whenever
//! `D` is in `[D_MIN, D_MAX]` (see [`crate::FilterBound::constants`]).
//!
//! The emitted bound is computed in `f64` with round-to-nearest, and each of
//! its operations can round down by a factor `1 - u`, `u = 2^-53`. For a
//! degree `k` the operations are `delta + e`, `EPSILON * d`, `KAPPA * e`,
//! their sum and the `k - 1` products by `d`: `k + 3` in all. The rounding
//! of `d` enters the bound once for each of the `k` times `d` is used, so
//! the computed bound is at least `(1 - u)^(2k + 2)` times its exact value:
//! one factor per operation and `k - 1` more for the uses of `d` beyond the
//! first. The caller's sum that forms `e` adds none, because `e` must be
//! rounded up, so that it is at least `E`. That is within the `2k + 8`
//! factors that the constants allow for: `EPSILON` and `KAPPA` are the
//! exact coefficients times `1 + 2^-30`, rounded up, and
//! `(1 + 2^-30)(1 - u)^(2k + 8) >= 1 + 2^-31` for every degree the
//! generator accepts, so the computed bound is at least `(1 + 2^-31) B`,
//! above the error. The partial results are formed from the largest one
//! down, so for `D >= D_MIN` none of them leaves the normal range on the way
//! (the term `KAPPA * e` may underflow, which loses at most `2^-1075`, a
//! fraction `2^-54` of the sum it joins and within the same allowance).
//!
//! `d` is compared with `D_MIN` and `D_MAX` after rounding to nearest, which
//! can make it larger or smaller than the exact `D`, by a factor `1 + u` at
//! most. That is harmless, because both conditions have slack. A `D` just
//! below `D_MIN` that rounds up to it raises the underflow allowance, at
//! most `2^-32` of the bound at `D_MIN`, by a factor `(1 + u)^k`, far within
//! the margin `2^-31` of the constants, and keeps the bound normal, since
//! at `D_MIN` the term `epsilon D^k` is at least `2^-1021`, twice the
//! smallest normal. A `D` just above
//! `D_MAX` that rounds down to it keeps every computed value below
//! `2^1022 (1 + u)^k`, still below the overflow threshold `2^1024`, since
//! `D_MAX` asks for `2^1022`. With `e = 0`, `d = delta` is exact.
//!
//! # The contract of `e`
//!
//! `e` is the sum of the two largest per-point errors, where the error of a
//! point is an upper bound on the absolute error of each coordinate of its
//! approximation, as an approximation of the exact point. It must be
//! finite, non-negative and rounded upward, so that it is at least the
//! exact sum `E`; it is `0.0` for explicit points. The bound needs
//! `E >= 0`: a negative `e` shrinks it, so the filter only decides when
//! `e >= 0`, a comparison that is also false for a NaN `e`. A NaN or
//! negative `e` therefore never decides, and an infinite one makes `d`
//! infinite, which never decides either.
//! Non-finite inputs never decide: a NaN leaf makes `det` NaN, an infinite
//! leaf makes `delta` and `d` infinite, an infinite `e` makes `d` infinite,
//! and every comparison with NaN is false. A decision implies
//! `|det| > bound > 0`, so the filter never returns zero.

use std::fmt::Write;

use super::comment;
use super::statements::{node_name, node_statements};
use crate::differences::{DiffGraph, DiffNode};
use crate::graph::NodeId;

/// The number of `f64` operations in [`bound_statements`] for a filter of
/// degree `degree`: `d`, `EPSILON * d`, `KAPPA * e`, their sum, and the
/// `degree - 1` products by `d`.
pub(crate) fn bound_operations(degree: u32) -> u32 {
    4 + (degree - 1)
}

/// The number of factors `1 - u` by which the rounding can shrink the
/// emitted bound of a filter of degree `degree`: one per operation, and one
/// more for each use of `d` beyond the first, since the rounding of `d`
/// enters the bound each time `d` is used.
pub(crate) fn bound_rounding_factors(degree: u32) -> u32 {
    bound_operations(degree) + (degree - 1)
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
/// and tells whether it uses the helper of [`write_max`]. Its documentation
/// states the contract of `e` given in the module documentation.
pub(crate) fn write_filter(
    out: &mut String,
    graph: &DiffGraph,
    points: usize,
    degree: u32,
) -> bool {
    comment(
        out,
        "///",
        0,
        "The sign of the predicate from an evaluation of its main form in `f64`, \
         when the rounding error cannot change it.\n\n\
         The points given are approximations of exact points. `e` is the sum of the \
         two largest per-point errors, where the error of a point is an upper bound on \
         the absolute error of each coordinate of its approximation; `e` must be \
         finite, non-negative and rounded upward, and it is `0.0` for explicit points. \
         A NaN or negative `e` never decides.\n\n\
         `None` means that this stage cannot decide: the value is too close to zero \
         for its error bound, the scale of the differences is outside \
         `D_MIN..=D_MAX`, `e` is negative or NaN, or an input is not finite. The \
         result is never `Some(Sign::Zero)`.",
    );
    out.push_str("#[inline]\n");
    writeln!(
        out,
        "pub fn filter(p: [&[f64; 3]; {points}], e: f64) -> Option<Sign> {{"
    )
    .unwrap();
    for statement in node_statements(graph, "p", false) {
        writeln!(out, "{statement}").unwrap();
    }
    let leaves: Vec<NodeId> = (0..graph.nodes.len())
        .filter(|&id| matches!(graph.nodes[id], DiffNode::Leaf { .. }))
        .collect();
    comment(
        out,
        "//",
        4,
        "A NaN leaf can be skipped by the comparisons, but it makes `det` NaN.",
    );
    for (i, &leaf) in leaves.iter().enumerate() {
        if i == 0 {
            writeln!(out, "let delta = {}.abs();", node_name(graph, leaf)).unwrap();
        } else {
            writeln!(
                out,
                "let delta = max(delta, {}.abs());",
                node_name(graph, leaf)
            )
            .unwrap();
        }
    }
    comment(
        out,
        "//",
        4,
        &format!(
            "The bound takes {} f64 operations; with the rounding of `d` counted at each \
             of its {degree} uses, at most {} roundings reach it.",
            bound_operations(degree),
            bound_rounding_factors(degree)
        ),
    );
    out.push_str(&bound_statements(degree));
    out.push_str(
        "let decided = (e >= 0.0) & (D_MIN..=D_MAX).contains(&d) & (det.abs() > bound);\n\
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
