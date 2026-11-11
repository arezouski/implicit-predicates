//! The floating-point filter of a multihomogeneous predicate, with one size
//! and one uncertainty per axis.
//!
//! The emitted `filter(p, e: [f64; 3])` evaluates the difference form in
//! `f64` and returns its sign when the per-axis error bound of
//! [`crate::AxisBound`] proves it. Only the axes `k` of positive degree
//! `G_k` take part. It computes, in this order:
//!
//! - each leaf, `p[i][axis] - p[j][axis]`, once, and each node of the
//!   difference form, one statement per node in the order of the graph, as
//!   the global filter does ([`super::filter`]);
//! - on each axis `k`, `delta_k`, the largest magnitude of the leaves of
//!   that axis, by comparisons, which are exact, and `d_k = delta_k + e[k]`;
//! - `bound`, a sum of at most four terms: `EPSILON` times `G_k` factors
//!   `d_k` on each axis, then, for each axis `k` with `e[k] > 0`, the term
//!   `KAPPA[k] * e[k]` times the factors `d` of `G` less one `d_k`. Each
//!   term is one chain of products from left to right that starts with its
//!   constant; no product of `d` values is shared between terms, and nothing
//!   is divided;
//! - the decision `e[k] >= 0`, `D_MIN <= d_k`, `d_k <= D_MAX` on each axis
//!   and `|det| > bound`, combined with a non-short-circuiting `&`; the sign
//!   is then chosen by a comparison, not a branch on the decision;
//! - when every term has positive degree in every axis
//!   ([`crate::AxisBound::certain_zero`]), the certain zero: if some `d_k`
//!   is zero, with every `e[k]` finite and `>= 0`, and `det` is exactly
//!   `0.0`, the filter returns `Some(Sign::Zero)`.
//!
//! Every choice is a select on values already computed, so the filter has
//! no branch; the certain zero costs a few comparisons on every call. The
//! filter is always inlined, so that for explicit points, whose `e` is the
//! constant zero, every computation with `e` folds away: `delta_k + 0.0`
//! is `delta_k`, a magnitude, which cannot be `-0.0`, and the terms of
//! `KAPPA` are left out.
//!
//! A term of `KAPPA` is left out only when `e[k] > 0` is false. When
//! `e[k]` is zero, the term computed in full, `KAPPA[k] * 0.0` times `d`
//! values, is zero whenever every `d_k` is finite, and the filter does not
//! decide otherwise, so leaving it out changes no decision; when `e[k]` is
//! negative or NaN the filter does not decide either.
//!
//! # Why the decision is sound
//!
//! The documentation of [`crate::AxisBound`] (in the source of its module)
//! proves that the bound computed in this way is at least the error of
//! `det` under these conditions, each of which the emitted code meets:
//!
//! 1. the range is checked on the `d_k` computed in `f64`, with comparisons
//!    that are false for NaN, on every axis of positive degree;
//! 2. `delta_k` is the largest magnitude of the same computed leaves that
//!    the value of the form uses;
//! 3. each `e[k]` used is checked to be `>= 0.0`, which is false for NaN;
//!    an infinite `e[k]` makes `d_k` infinite, above `D_MAX`, so it decides
//!    no sign, and the certain zero checks `e[k] <= f64::MAX`, so it decides
//!    no zero either; the caller guarantees that `e[k]`
//!    is at least the true sum of the two largest errors on axis `k`;
//! 4. `EPSILON` and `KAPPA` are those of [`crate::AxisBound::constants`];
//! 5. the bound is a sum of at most four chains of products, each starting
//!    with its constant, less terms that are zero whenever the filter can
//!    decide;
//! 6. the comparison is `|det| > bound`, false when the bound is NaN;
//! 7. the value is evaluated node by node with the structure of the graph.
//!
//! # The certain zero
//!
//! `d_k = delta_k + e[k]` with `delta_k >= 0` and `e[k] >= 0` is zero only
//! when both are: every computed leaf of axis `k` is zero, which for finite
//! coordinates means that the two coordinates of each leaf are equal, and
//! the errors of the points on axis `k` are zero, so the exact coordinates
//! are the given ones. Every exact leaf of axis `k` is then zero, and since
//! every term of the form has positive degree in axis `k`, the form is
//! exactly zero. An infinite coordinate makes a leaf infinite or NaN, so
//! not zero, but a NaN leaf can be skipped by the maxima; it makes `det`
//! NaN, so the second condition, `det == 0.0`, is false then. A flat axis
//! has `d_k` below `D_MIN`, so the filter never decides a sign there.

use std::fmt::Write;

use super::statements::{node_name, node_statements};
use super::{comment, literal, power_of_two};
use crate::bound::AxisConstants;
use crate::differences::{DiffGraph, DiffNode};
use crate::graph::NodeId;

/// The names of the axes in the emitted locals.
const AXES: [&str; 3] = ["x", "y", "z"];

/// Writes the constants of the per-axis filter.
pub(crate) fn write_constants(out: &mut String, c: &AxisConstants) {
    let constant = |out: &mut String, doc: &str, name: &str, ty: &str, value: String| {
        comment(out, "///", 0, doc);
        writeln!(out, "pub const {name}: {ty} = {value};\n").unwrap();
    };
    let g = c.degrees;
    constant(
        out,
        "The degree `G_k` of the predicate in the differences of coordinates of each \
         axis `k`, for x, y and z.",
        "DEGREES",
        "[u32; 3]",
        format!("[{}, {}, {}]", g[0], g[1], g[2]),
    );
    constant(
        out,
        "The coefficient of `d^G`, the product of `d_k^G_k` over the axes, in the error \
         bound of [`filter`], rounded up. On each axis `k`, `d_k` is the largest \
         magnitude of a difference of coordinates on that axis plus `e[k]`, the \
         uncertainty given to the filter, and the bound is `EPSILON*d^G` plus \
         `KAPPA[k]*e[k]*d^G/d_k` for each axis of positive degree, computed \
         without divisions.",
        "EPSILON",
        "f64",
        literal(c.epsilon),
    );
    constant(
        out,
        "The coefficient of `e[k]*d^G/d_k` in the error bound of [`filter`], for each \
         axis `k`, rounded up; zero on an axis of degree zero.",
        "KAPPA",
        "[f64; 3]",
        format!(
            "[{}, {}, {}]",
            literal(c.kappa[0]),
            literal(c.kappa[1]),
            literal(c.kappa[2])
        ),
    );
    constant(
        out,
        &format!(
            "The smallest `d_k`, on each axis of positive degree, for which [`filter`] \
             decides, `2^{}`: below it, underflow could exceed the error bound.",
            power_of_two(c.d_min)
        ),
        "D_MIN",
        "f64",
        literal(c.d_min),
    );
    constant(
        out,
        &format!(
            "The largest `d_k`, on each axis of positive degree, for which [`filter`] \
             decides, `2^{}`: above it, a computed value could overflow.",
            power_of_two(c.d_max)
        ),
        "D_MAX",
        "f64",
        literal(c.d_max),
    );
}

/// The axes of positive degree.
fn axes(degrees: [u32; 3]) -> impl Iterator<Item = usize> {
    (0..3).filter(move |&k| degrees[k] >= 1)
}

/// The statements that compute `bound`: `EPSILON * d^G`, then, for each
/// axis `k` of positive degree whose `e[k]` is positive, the sum with
/// `KAPPA[k] * e[k] * d^(G - e_k)`. Each term is a chain of products that
/// starts with its constant.
pub(crate) fn bound_statements(degrees: [u32; 3]) -> String {
    let factors = |skip: Option<usize>| {
        let mut text = String::new();
        for k in axes(degrees) {
            for _ in 0..degrees[k] - u32::from(skip == Some(k)) {
                write!(text, " * d{}", AXES[k]).unwrap();
            }
        }
        text
    };
    let mut out = format!("let bound = EPSILON{};\n", factors(None));
    for k in axes(degrees) {
        writeln!(
            out,
            "let bound = if e[{k}] > 0.0 {{ bound + KAPPA[{k}] * e[{k}]{} }} else {{ bound }};",
            factors(Some(k))
        )
        .unwrap();
    }
    out
}

/// Writes the `filter` function of a multihomogeneous difference form with
/// `points` points and the emitted `constants`, with the branch of the
/// certain zero when `certain_zero` holds, and tells whether it uses the
/// helper `max`.
pub(crate) fn write_filter(
    out: &mut String,
    graph: &DiffGraph,
    points: usize,
    constants: &AxisConstants,
    certain_zero: bool,
) -> bool {
    let g = constants.degrees;
    let result = if certain_zero {
        "`Some(Sign::Zero)` means that some axis is flat: every difference of coordinates \
         on it is zero and so is its `e[k]`, so the predicate is exactly zero. "
    } else {
        "The result is never `Some(Sign::Zero)`. "
    };
    let unused = if g.contains(&0) {
        " The `e[k]` of an axis of degree zero is not used."
    } else {
        ""
    };
    comment(
        out,
        "///",
        0,
        &format!(
            "The sign of the predicate from an evaluation of its main form in `f64`, \
             when the rounding error cannot change it.\n\n\
             The points given are approximations of exact points. `e[k]` is the sum of \
             the two largest errors of the points on axis `k` (0, 1 and 2 for x, y and \
             z), where the error of a point on an axis is an upper bound on the absolute \
             error of its coordinate on that axis; each `e[k]` must be finite, \
             non-negative and rounded upward, and they are `0.0` for explicit points. A \
             NaN or negative `e[k]` never decides.{unused}\n\n\
             {result}`None` means that this stage cannot decide: the value is too close \
             to zero for its error bound, the scale `d_k` of the differences of some axis \
             is outside `D_MIN..=D_MAX`, some `e[k]` is negative or NaN, or an input is \
             not finite."
        ),
    );
    // Always inlined: a call would pass `e` through memory, and inlined with
    // a constant `e` every computation with `e` folds away. The range of each
    // `d_k` is two comparisons joined by `&`, not `contains`, which
    // short-circuits.
    out.push_str("#[allow(clippy::manual_range_contains)]\n#[inline(always)]\n");
    writeln!(
        out,
        "pub fn filter(p: [&[f64; 3]; {points}], e: [f64; 3]) -> Option<Sign> {{"
    )
    .unwrap();
    for statement in node_statements(graph, "p", false) {
        writeln!(out, "{statement}").unwrap();
    }
    comment(
        out,
        "//",
        4,
        "The largest magnitude of the leaves of each axis. A NaN leaf can be skipped by \
         the comparisons, but it makes `det` NaN.",
    );
    let mut uses_max = false;
    for k in axes(g) {
        let leaves: Vec<NodeId> = (0..graph.nodes.len())
            .filter(|&id| {
                matches!(graph.nodes[id], DiffNode::Leaf { axis, .. } if usize::from(axis) == k)
            })
            .collect();
        let name = format!("delta_{}", AXES[k]);
        for (i, &leaf) in leaves.iter().enumerate() {
            let leaf = node_name(graph, leaf);
            if i == 0 {
                writeln!(out, "let {name} = {leaf}.abs();").unwrap();
            } else {
                writeln!(out, "let {name} = max({name}, {leaf}.abs());").unwrap();
            }
        }
        uses_max |= leaves.len() > 1;
    }
    for k in axes(g) {
        writeln!(out, "let d{0} = delta_{0} + e[{k}];", AXES[k]).unwrap();
    }
    comment(
        out,
        "//",
        4,
        "Each term is a chain of products that starts with its constant. A term with \
         `e[k] == 0` is zero whenever the filter can decide, and is left out, so that \
         it costs nothing when `e` is a constant zero.",
    );
    out.push_str(&bound_statements(g));
    let valid: Vec<String> = axes(g).map(|k| format!("(e[{k}] >= 0.0)")).collect();
    let valid = valid.join(" & ");
    let mut decided = vec![valid.clone()];
    for k in axes(g) {
        let axis = AXES[k];
        decided.push(format!("(D_MIN <= d{axis}) & (d{axis} <= D_MAX)"));
    }
    decided.push("(det.abs() > bound)".to_string());
    writeln!(out, "let decided = {};", decided.join(" & ")).unwrap();
    out.push_str("let sign = if det > 0.0 { Sign::Positive } else { Sign::Negative };\n");
    if certain_zero {
        comment(
            out,
            "//",
            4,
            "A flat axis, with every leaf and its `e[k]` zero, makes the predicate \
             exactly zero; a NaN leaf skipped by the maxima makes `det` NaN, and each \
             `e[k]` must be finite. A flat axis is below `D_MIN`, so `decided` is \
             false then.",
        );
        let flat: Vec<String> = axes(g).map(|k| format!("(d{} == 0.0)", AXES[k])).collect();
        writeln!(out, "let flat = {};", flat.join(" | ")).unwrap();
        let finite: Vec<String> = axes(g).map(|k| format!("(e[{k}] <= f64::MAX)")).collect();
        writeln!(
            out,
            "let zero = {valid} & {} & flat & (det == 0.0);",
            finite.join(" & ")
        )
        .unwrap();
        out.push_str(
            "let sign = if zero { Sign::Zero } else { sign };\n\
             (decided | zero).then_some(sign)\n}\n\n",
        );
    } else {
        out.push_str("decided.then_some(sign)\n}\n\n");
    }
    uses_max
}
