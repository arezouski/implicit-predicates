//! The second test of the filter of a predicate whose form is not
//! multihomogeneous: `filter_axes`, with the error bound by parts of
//! [`crate::PartsBound`], one size and one uncertainty per axis.
//!
//! It runs only when `filter`, the global test, declines, so it is a cold
//! stage of its own, `#[cold]` and `#[inline(never)]`, that computes
//! everything again from the points. The emitted `filter_axes(p, e: [f64; 3])`
//! computes, in this order:
//!
//! - each leaf, `p[i][axis] - p[j][axis]`, once, and each node of the
//!   difference form, one statement per node in the order of the graph, as
//!   the global filter does ([`super::filter`]);
//! - on each axis `k` of [`crate::PartsConstants::range_axes`], `delta_k`,
//!   the largest magnitude of the leaves of that axis, by comparisons, which
//!   are exact, and `d_k = delta_k + e[k]`; `d` is the array of the three,
//!   with `0.0` on an axis outside the range axes, which no term uses;
//! - `bound`, the sum, from `0.0` and in the order of the tables, of every
//!   term of `AXES_EPSILON` and of `AXES_KAPPA`, none left out whatever `e`
//!   is: a term of epsilon `(g, c)` is `product_chain(c, g, d)`, `c` times
//!   `d_k` repeated `g_k` times on each axis, and a term of kappa `(k, h, c)`
//!   is `product_chain(c * e[k], h, d)`, `c` times `e[k]` and then `d_j`
//!   repeated `h_j` times on each axis. Each term is a chain of products
//!   from left to right that starts with its constant, as the proof of the
//!   bound requires; the first addition, to `0.0`, is exact;
//! - `valid`, every `e[k]` of the three axes `>= 0.0` and `<= f64::MAX`;
//! - the decision: `valid`, `AXES_D_MIN <= d_k` and `d_k <= AXES_D_MAX` on
//!   each range axis, and `|det| > bound`, combined with a
//!   non-short-circuiting `&`; the sign is then chosen by a comparison;
//! - when every degree vector has positive degree in every axis
//!   ([`crate::PartsBound::certain_zero`]), the certain zero: if some `d_k`
//!   is zero, `valid` holds, every `d_k` is at most `f64::MAX` and `det` is
//!   exactly `0.0`, the stage returns `Some(Sign::Zero)`.
//!
//! Every comparison is written so that NaN makes it false: a NaN `e[k]`
//! fails `valid`, a NaN `d_k` fails the range and the finiteness of the
//! certain zero, and a NaN `det` or `bound` fails `|det| > bound` and
//! `det == 0.0`. The emitted `max` skips a NaN leaf, so a NaN coordinate
//! can leave every `delta_k` finite, but it makes `det` NaN, so it decides
//! nothing.
//!
//! # Why the decision is sound
//!
//! The documentation of the module of [`crate::PartsBound`], in its source,
//! proves that the bound computed in this way exceeds the error of `det`
//! when each of its conditions holds, and the emitted code meets each:
//!
//! 1. the range is checked on the `d_k` computed in `f64`,
//!    `d_k = delta_k + e[k]`, on every axis of `range_axes`, with
//!    comparisons that are false for NaN;
//! 2. `delta_k` is the largest magnitude of the same computed leaves that
//!    the value of the form uses;
//! 3. every `E_k` is finite and non-negative: `valid` checks it on the three
//!    axes, and the caller guarantees that `e[k]` is at least the true sum of
//!    the two largest errors on axis `k`;
//! 4. the constants are those of [`crate::PartsBound::constants`], read from
//!    the tables `AXES_EPSILON` and `AXES_KAPPA` by the function itself, so
//!    the tables are the values it uses;
//! 5. the bound is the sum of all the terms, each a chain of products that
//!    starts with its constant;
//! 6. the comparison is `|det| > bound`, false when the bound is infinite or
//!    NaN;
//! 7. the value is evaluated node by node with the structure of the graph.
//!
//! The certain zero is proved in the same documentation: with `d_k = 0`,
//! `e[k] >= 0` and `delta_k >= 0`, both are zero, so every computed leaf of
//! axis `k` is zero and the exact points have the given coordinates on axis
//! `k`; every exact leaf of axis `k` is then zero, and so is the true value
//! of the form. The computed value is zero too, or NaN when a factor of
//! another axis is not finite (as in `0 * inf`), so the stage decides zero
//! only when it is exactly `0.0`, every `d_k` is finite and every `e[k]` is
//! finite and non-negative.

use std::fmt::Write;

use super::statements::{node_name, node_statements};
use super::{comment, literal, power_of_two};
use crate::bound::PartsConstants;
use crate::differences::{DiffGraph, DiffNode};
use crate::graph::NodeId;

/// The names of the axes in the emitted locals.
const AXES: [&str; 3] = ["x", "y", "z"];

/// Writes the constants of `filter_axes`.
pub(crate) fn write_constants(out: &mut String, c: &PartsConstants) {
    let constant = |out: &mut String, doc: &str, name: &str, ty: &str, value: String| {
        comment(out, "///", 0, doc);
        writeln!(out, "pub const {name}: {ty} = {value};\n").unwrap();
    };
    let vector = |g: [u32; 3]| format!("[{}, {}, {}]", g[0], g[1], g[2]);
    let epsilon: Vec<String> = c
        .epsilon
        .iter()
        .map(|t| format!("({}, {})", vector(t.degrees), literal(t.value)))
        .collect();
    constant(
        out,
        "The terms of epsilon of the error bound of [`filter_axes`]: for each degree \
         vector `g` of the predicate, the coefficient of `d^g`, the product of `d_k^g_k` \
         over the axes, rounded up. On each axis `k`, `d_k` is the largest magnitude of \
         a difference of coordinates on that axis plus `e[k]`, the uncertainty given to \
         [`filter_axes`], and the bound is the sum of every term of this table and of \
         [`AXES_KAPPA`], computed without divisions.",
        "AXES_EPSILON",
        &format!("[([u32; 3], f64); {}]", epsilon.len()),
        format!("[{}]", epsilon.join(", ")),
    );
    let kappa: Vec<String> = c
        .kappa
        .iter()
        .map(|t| format!("({}, {}, {})", t.axis, vector(t.rest), literal(t.value)))
        .collect();
    constant(
        out,
        "The terms of kappa of the error bound of [`filter_axes`]: for each, the axis `k` \
         of its uncertainty `e[k]`, the degree vector `h` of the rest of the term, and the \
         coefficient of `e[k]*d^h`, rounded up.",
        "AXES_KAPPA",
        &format!("[(usize, [u32; 3], f64); {}]", kappa.len()),
        format!("[{}]", kappa.join(", ")),
    );
    let axes = range_axes_text(c.range_axes);
    constant(
        out,
        &format!(
            "The smallest `d_k`, on {axes}, for which [`filter_axes`] decides, `2^{}`: \
             below it, underflow could exceed the error bound.",
            power_of_two(c.d_min)
        ),
        "AXES_D_MIN",
        "f64",
        literal(c.d_min),
    );
    constant(
        out,
        &format!(
            "The largest `d_k`, on {axes}, for which [`filter_axes`] decides, `2^{}`: \
             above it, a computed value could overflow.",
            power_of_two(c.d_max)
        ),
        "AXES_D_MAX",
        "f64",
        literal(c.d_max),
    );
}

/// The axes of the range, in words: "every axis", or "the axes x and y".
fn range_axes_text(range_axes: [bool; 3]) -> String {
    if range_axes.iter().all(|&r| r) {
        return "every axis".to_string();
    }
    let names: Vec<&str> = (0..3).filter(|&k| range_axes[k]).map(|k| AXES[k]).collect();
    match names.as_slice() {
        [one] => format!("the axis {one}, the only one the predicate uses,"),
        _ => format!(
            "the axes {}, the only ones the predicate uses,",
            names.join(" and ")
        ),
    }
}

/// The axes on which the range is required.
fn axes(range_axes: [bool; 3]) -> impl Iterator<Item = usize> {
    (0..3).filter(move |&k| range_axes[k])
}

/// Writes `filter_axes` for a difference form with `points` points and the
/// emitted `constants`, with the certain zero when `certain_zero` holds, and
/// tells whether it uses the helper `max`.
pub(crate) fn write_filter_axes(
    out: &mut String,
    graph: &DiffGraph,
    points: usize,
    constants: &PartsConstants,
    certain_zero: bool,
) -> bool {
    let range = constants.range_axes;
    let result = if certain_zero {
        "`Some(Sign::Zero)` means that some axis is flat: every difference of coordinates \
         on it is zero and so is its `e[k]`, so the predicate is exactly zero. "
    } else {
        "The result is never `Some(Sign::Zero)`. "
    };
    comment(
        out,
        "///",
        0,
        &format!(
            "The sign of the predicate from an evaluation of its main form in `f64`, \
             when the rounding error cannot change it, with an error bound that keeps one \
             size per axis: the second test of the filter, for the calls that [`filter`] \
             does not decide.\n\n\
             The points given are approximations of exact points. `e[k]` is the sum of \
             the two largest errors of the points on axis `k` (0, 1 and 2 for x, y and \
             z), where the error of a point on an axis is an upper bound on the absolute \
             error of its coordinate on that axis; each `e[k]` must be finite, \
             non-negative and rounded upward, and they are `0.0` for explicit points. An \
             `e[k]` that is negative, infinite or NaN never decides.\n\n\
             {result}`None` means that this stage cannot decide: the value is too close \
             to zero for its error bound, the scale `d_k` of the differences of some axis \
             is outside `AXES_D_MIN..=AXES_D_MAX`, some `e[k]` is not finite and \
             non-negative, or an input is not finite."
        ),
    );
    // The ranges are two comparisons joined by `&`, not `contains`, which
    // short-circuits.
    out.push_str("#[allow(clippy::manual_range_contains)]\n#[cold]\n#[inline(never)]\n");
    writeln!(
        out,
        "pub fn filter_axes(p: [&[f64; 3]; {points}], e: [f64; 3]) -> Option<Sign> {{"
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
    for k in axes(range) {
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
    for k in axes(range) {
        writeln!(out, "let d{0} = delta_{0} + e[{k}];", AXES[k]).unwrap();
    }
    let d: Vec<String> = (0..3)
        .map(|k| {
            if range[k] {
                format!("d{}", AXES[k])
            } else {
                "0.0".to_string()
            }
        })
        .collect();
    if !range.iter().all(|&r| r) {
        comment(
            out,
            "//",
            4,
            "No term has a factor of an axis outside the range.",
        );
    }
    writeln!(out, "let d = [{}];", d.join(", ")).unwrap();
    comment(
        out,
        "//",
        4,
        "Every term, none left out whatever `e` is, each a chain of products that \
         starts with its constant; the first sum, with `0.0`, is exact.",
    );
    out.push_str(
        "let mut bound = 0.0;\n\
         for (g, c) in AXES_EPSILON {\n\
         bound += product_chain(c, g, d);\n\
         }\n\
         for (k, h, c) in AXES_KAPPA {\n\
         bound += product_chain(c * e[k], h, d);\n\
         }\n",
    );
    comment(
        out,
        "//",
        4,
        "Each `e[k]` finite and non-negative, a comparison that is false for NaN.",
    );
    let valid: Vec<String> = (0..3)
        .map(|k| format!("(e[{k}] >= 0.0) & (e[{k}] <= f64::MAX)"))
        .collect();
    writeln!(out, "let valid = {};", valid.join(" & ")).unwrap();
    let mut decided = vec!["valid".to_string()];
    for k in axes(range) {
        let axis = AXES[k];
        decided.push(format!("(AXES_D_MIN <= d{axis}) & (d{axis} <= AXES_D_MAX)"));
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
             exactly zero. The computed value is then zero, or NaN when a factor of \
             another axis is not finite: it decides only when it is exactly zero, with \
             every `d_k` finite. A flat axis is below `AXES_D_MIN`, so `decided` is \
             false then.",
        );
        let flat: Vec<String> = axes(range)
            .map(|k| format!("(d{} == 0.0)", AXES[k]))
            .collect();
        writeln!(out, "let flat = {};", flat.join(" | ")).unwrap();
        let finite: Vec<String> = axes(range)
            .map(|k| format!("(d{} <= f64::MAX)", AXES[k]))
            .collect();
        writeln!(out, "let finite = {};", finite.join(" & ")).unwrap();
        out.push_str(
            "let zero = valid & finite & flat & (det == 0.0);\n\
             let sign = if zero { Sign::Zero } else { sign };\n\
             (decided | zero).then_some(sign)\n}\n\n",
        );
    } else {
        out.push_str("decided.then_some(sign)\n}\n\n");
    }
    uses_max
}

/// Writes the helper `product_chain` that `filter_axes` uses.
pub(crate) fn write_product_chain(out: &mut String) {
    comment(
        out,
        "///",
        0,
        "`start` times `d[k]` repeated `g[k]` times, on each axis `k` in turn: a chain \
         of products from left to right, each rounded, that starts with `start`.",
    );
    out.push_str(
        "#[inline(always)]\n\
         fn product_chain(start: f64, g: [u32; 3], d: [f64; 3]) -> f64 {\n\
         let mut x = start;\n\
         for (&n, &dk) in g.iter().zip(&d) {\n\
         for _ in 0..n {\n\
         x *= dk;\n\
         }\n\
         }\n\
         x\n\
         }\n\n",
    );
}
