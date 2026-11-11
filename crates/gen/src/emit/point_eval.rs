//! The approximate coordinates of a point type and their error bounds:
//! `evaluate`, which runs the error program of [`crate::error_bound`], and
//! the constants and functions of the module that it and `write_exact` use.

use std::fmt::Write;

use implicit_predicates_core::eft::{PRODUCT_OPERAND_MAX, PRODUCT_OPERAND_MIN};

use super::point_exact::Op;
use super::points::{Implicit, TypeInfo};
use super::{comment, literal, power_of_two};
use crate::point::{ErrorProgram, PointNode, Term};
use crate::range::OVERFLOW_EXPONENT;

/// The text of the error of node `id`: `e{id}`, or `0.0` if it has none.
pub(crate) fn error_name(program: &ErrorProgram, id: usize) -> String {
    if program.terms[id].is_empty() {
        "0.0".to_string()
    } else {
        format!("e{id}")
    }
}

/// Writes `evaluate`: one statement per node for its value, `v{id}`, and
/// one for its error, `e{id}`, unless it has none.
pub(crate) fn write_evaluate(out: &mut String, info: &TypeInfo<'_>, implicit: &Implicit) {
    let graph = &info.point.graph;
    let program = &implicit.program;
    comment(
        out,
        "///",
        0,
        "The coordinates in `f64` and the bounds on their errors, from the `F64x3` \
         parameters `c` and the `f64` parameters `s`, in the order of the declaration.",
    );
    writeln!(
        out,
        "fn evaluate({}) -> ([f64; 3], [f64; 3]) {{",
        info.array_params()
    )
    .unwrap();
    for (id, &node) in graph.nodes.iter().enumerate() {
        let value = match node {
            PointNode::Neg(x) => format!("-v{x}"),
            PointNode::Add(x, y) => format!("v{x} + v{y}"),
            PointNode::Sub(x, y) => format!("v{x} - v{y}"),
            PointNode::Mul(x, y) => format!("v{x} * v{y}"),
            leaf => info.leaf(leaf).expect("a leaf"),
        };
        writeln!(out, "let v{id} = {value};").unwrap();
        let terms = &program.terms[id];
        if terms.is_empty() {
            continue;
        }
        let parts: Vec<String> = terms
            .iter()
            .map(|&term| match term {
                Term::Error(x) => format!("e{x}"),
                Term::ValueTimesError { value, error } => {
                    format!("covered_product(v{value}.abs(), e{error})")
                }
                Term::ErrorTimesError(x, y) => format!("covered_product(e{x}, e{y})"),
                Term::SumRounding(x, y) => format!("sum_rounding(v{x}, v{y}, v{id})"),
                Term::ProductRounding(x, y) => format!("product_rounding(v{x}, v{y}, v{id})"),
            })
            .collect();
        writeln!(out, "let e{id} = {};", parts.join(" + ")).unwrap();
    }
    let roots = graph.roots;
    writeln!(
        out,
        "let approx = [v{}, v{}, v{}];\nlet err = [",
        roots[0], roots[1], roots[2]
    )
    .unwrap();
    for (k, &root) in roots.iter().enumerate() {
        writeln!(
            out,
            "finish(approx[{k}], {}, Self::ERR_SLACK),",
            error_name(program, root)
        )
        .unwrap();
    }
    out.push_str("];\n(approx, err)\n}\n\n");
}

/// Writes the constants and functions that `evaluate` uses.
pub(crate) fn write_error_helpers(out: &mut String, implicit: &[&Implicit]) {
    let uses = |f: fn(&Term) -> bool| {
        implicit
            .iter()
            .any(|i| i.program.terms.iter().flatten().any(f))
    };
    let sums = uses(|t| matches!(t, Term::SumRounding(..)));
    let products = uses(|t| matches!(t, Term::ProductRounding(..)));
    let covered = uses(|t| !matches!(t, Term::Error(_)));
    if sums || products {
        writeln!(
            out,
            "/// `2^-53`, the unit roundoff of `f64`.\nconst UNIT: f64 = {};\n",
            literal(2f64.powi(-53))
        )
        .unwrap();
    }
    if covered {
        writeln!(
            out,
            "/// `2^-1074`, the smallest positive `f64`.\nconst TINY: f64 = {};\n",
            literal(f64::from_bits(1))
        )
        .unwrap();
        comment(
            out,
            "///",
            0,
            "`fl(fl(a * b) + 2^-1074)` for non-negative `a` and `b`, or zero if either is \
             zero: at least `(1 - 2^-53)^2 * a * b`, also when `a * b` underflows, since \
             adding `2^-1074` to a value below `2^-1022` is exact.",
        );
        out.push_str(
            "#[inline]\nfn covered_product(a: f64, b: f64) -> f64 {\n\
             if a == 0.0 || b == 0.0 { 0.0 } else { a * b + TINY }\n}\n\n",
        );
    }
    if sums {
        comment(
            out,
            "///",
            0,
            "The bound on the rounding error of the sum or difference `s` of `x` and `y`: \
             the covered product of `2^-53` and `|s|`, or zero if an operand is zero, when \
             the operation is exact.",
        );
        out.push_str(
            "#[inline]\nfn sum_rounding(x: f64, y: f64, s: f64) -> f64 {\n\
             if x != 0.0 && y != 0.0 { covered_product(UNIT, s.abs()) } else { 0.0 }\n}\n\n",
        );
    }
    if products {
        comment(
            out,
            "///",
            0,
            "The bound on the rounding error of the product `p` of `x` and `y`: the covered \
             product of `2^-53` and `|p|`, plus `2^-1074` for underflow, or zero if an \
             operand is zero, when the operation is exact. The test is on the operands: a \
             product that underflows is zero without being exact.",
        );
        out.push_str(
            "#[inline]\nfn product_rounding(x: f64, y: f64, p: f64) -> f64 {\n\
             if x != 0.0 && y != 0.0 { covered_product(UNIT, p.abs()) + TINY } else { 0.0 }\n}\n\n",
        );
    }
    comment(
        out,
        "///",
        0,
        "The error bound stored for a coordinate whose value is `approx` and whose \
         computed error is `err`: `err * slack` rounded up, which is at least the error \
         since the slack factor covers the roundings of its computation; zero when `err` \
         is zero; and `+∞` when `approx` or the bound is not finite.",
    );
    out.push_str(
        "#[inline]\nfn finish(approx: f64, err: f64, slack: f64) -> f64 {\n\
         let bound = err * slack;\n\
         if !(approx.is_finite() && bound.is_finite()) {\nf64::INFINITY\n} \
         else if err == 0.0 {\n0.0\n} else {\nbound.next_up()\n}\n}\n\n",
    );
}

/// Writes the constants and functions that `write_exact` uses.
pub(crate) fn write_exact_helpers(out: &mut String, implicit: &[&Implicit]) {
    let uses = |f: fn(&Op) -> bool| implicit.iter().any(|i| i.exact.uses(f));
    let sums = uses(|op| !matches!(op, Op::TwoProduct(..) | Op::Scale { .. } | Op::Product(..)));
    if sums {
        comment(
            out,
            "///",
            0,
            &format!(
                "`2^{OVERFLOW_EXPONENT}`: the expansions of the core require every operand of \
                 a sum to be below it in magnitude."
            ),
        );
        writeln!(
            out,
            "const SUM_OPERAND_LIMIT: f64 = {};\n",
            literal(2f64.powi(i32::try_from(OVERFLOW_EXPONENT).expect("small")))
        )
        .unwrap();
        comment(
            out,
            "///",
            0,
            "Whether every component of `e` is below `SUM_OPERAND_LIMIT` in magnitude, as \
             the components of an operand of a sum of expansions must be.",
        );
        out.push_str(
            "#[inline]\nfn summable(e: &[f64]) -> bool {\n\
             e.iter().all(|x| x.abs() < SUM_OPERAND_LIMIT)\n}\n\n",
        );
    }
    if implicit.iter().any(|i| i.exact.uses_products()) {
        comment(
            out,
            "///",
            0,
            &format!(
                "Whether every component of `e` lies in the product domain of the core, zero \
                 or a magnitude from `2^{}` to `2^{}`, as the components of an operand of an \
                 exact product must.",
                power_of_two(PRODUCT_OPERAND_MIN),
                power_of_two(PRODUCT_OPERAND_MAX)
            ),
        );
        out.push_str(
            "#[inline]\nfn multipliable(e: &[f64]) -> bool {\n\
             e.iter().all(|&x| eft::in_product_domain(x))\n}\n\n",
        );
    }
    if uses(|op| matches!(op, Op::Product(..))) {
        comment(
            out,
            "///",
            0,
            "Writes the exact product of the expansions `a` and `b` to `out`, of at least \
             `2 * a.len() * b.len()` components, and returns its length; `work`, of at least \
             `2 * b.len()` components more than that, is working space. Every component of \
             `a` and `b` must lie in the product domain.",
        );
        out.push_str(
            "fn product(a: &[f64], b: &[f64], out: &mut [f64], work: &mut [f64]) -> usize {\n\
             let (scaled, sum) = work.split_at_mut(2 * b.len());\n\
             let mut len = 0;\n\
             for &x in a {\n\
             let n = expansion::scale(b, x, scaled);\n\
             sum[..len].copy_from_slice(&out[..len]);\n\
             len = expansion::sum(&sum[..len], &scaled[..n], out);\n\
             }\n\
             len\n}\n\n",
        );
    }
    comment(
        out,
        "///",
        0,
        "The span of a coordinate just written, or the error of `write_exact` once the \
         arena is reset to `mark`: [`ExactError::OutOfRange`] when `in_range` is false.",
    );
    out.push_str(
        "#[inline]\nfn written(\nscratch: &mut Scratch<'_>,\nmark: Mark,\n\
         span: Result<Span, ScratchExhausted>,\nin_range: bool,\n) -> Result<Span, ExactError> {\n\
         match span {\nOk(span) if in_range => Ok(span),\n\
         Ok(_) => {\nscratch.reset(mark);\nErr(ExactError::OutOfRange)\n}\n\
         Err(e) => {\nscratch.reset(mark);\nErr(e.into())\n}\n}\n}\n",
    );
}
