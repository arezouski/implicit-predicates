//! The checks of a predicate: code for its tests, not for its users.
//!
//! The generated module holds `exact`, the main form evaluated with the
//! exact dyadic numbers of `implicit-predicates-oracle`, one statement per
//! node of the difference form as in the filter, and `bound_edge_cases`,
//! inputs on either side of the error bound of the filter for explicit
//! points; for a per-axis filter, also `uncertainty_edge_cases`, inputs on
//! either side of its bound with an uncertainty on one axis or on all of
//! them. The cases are built by [`super::edges`] and written as bit
//! patterns so that they read back as exactly the same `f64`s.

use std::fmt::Write;

use super::comment;
use super::edges::{Case, bound_edge_cases, uncertainty_edge_cases};
use super::filter::FilterKind;
use super::statements::node_statements;
use crate::differences::DiffGraph;
use crate::parse::Predicate;

/// Writes the checks module of `predicate`, whose difference form is
/// `graph` and whose filter has the form and the emitted constants of
/// `filter`, starting with `notice`.
pub(crate) fn write_checks(
    out: &mut String,
    predicate: &Predicate,
    graph: &DiffGraph,
    filter: &FilterKind,
    notice: &str,
) {
    let points = predicate.params.len();
    let ty = format!("[[f64; 3]; {points}]");
    out.push_str(notice);
    comment(
        out,
        "//!",
        0,
        &format!(
            "Checks of the predicate `{}`, for its tests: [`exact`], its exact sign, and \
             [`bound_edge_cases`], inputs at the edge of the error bound of its filter.",
            predicate.name
        ),
    );
    out.push_str("\nuse implicit_predicates::Sign;\nuse implicit_predicates_oracle::Dyadic;\n\n");
    comment(
        out,
        "///",
        0,
        "The exact sign of the predicate at the points `p`: its main form, evaluated \
         with exact dyadic numbers in the order of the definition.\n\n\
         # Panics\n\n\
         Panics if a coordinate is not finite.",
    );
    writeln!(
        out,
        "pub fn exact(p: &{ty}) -> Sign {{\n\
         let c = p.map(|point| point.map(Dyadic::from_f64));"
    )
    .unwrap();
    for statement in node_statements(graph, "c", true) {
        writeln!(out, "{statement}").unwrap();
    }
    out.push_str(
        "match det.sign() {\n\
         1 => Sign::Positive,\n\
         -1 => Sign::Negative,\n\
         _ => Sign::Zero,\n\
         }\n}\n\n",
    );
    let cases = bound_edge_cases(graph, points, filter);
    let bound = match filter {
        FilterKind::Global(c) => format!(
            "`EPSILON * delta^{}`, where `delta` is the largest difference of coordinates \
             as the filter computes it",
            c.degree
        ),
        FilterKind::PerAxis(..) => "`EPSILON*d^G`, the product of `d_k^G_k` over the axes, \
             where `d_k` is the largest difference of coordinates on axis `k` as the \
             filter computes it"
            .to_string(),
    };
    comment(
        out,
        "///",
        0,
        &format!(
            "Inputs on either side of the error bound of the filter for explicit points, \
             in pairs: the first of each pair has an exact magnitude at most {bound}, the \
             second above it; the two differ by one representable value in one \
             coordinate.\n\n\
             Each pair starts from points on a grid at which the predicate is zero, or \
             changes sign, and moves one coordinate away from there. The scales go from \
             just above `D_MIN` to just below `D_MAX`."
        ),
    );
    writeln!(
        out,
        "pub fn bound_edge_cases() -> Vec<{ty}> {{\n\
         BOUND_EDGE_CASES\n\
         .into_iter()\n\
         .map(|case| case.map(|point| point.map(f64::from_bits)))\n\
         .collect()\n}}\n"
    )
    .unwrap();
    comment(
        out,
        "///",
        0,
        "The bit patterns of the coordinates of [`bound_edge_cases`].",
    );
    writeln!(
        out,
        "const BOUND_EDGE_CASES: [[[u64; 3]; {points}]; {}] = [",
        2 * cases.len()
    )
    .unwrap();
    for (below, above) in &cases {
        writeln!(out, "{},", case(below)).unwrap();
        writeln!(out, "{},", case(above)).unwrap();
    }
    out.push_str("];\n");
    if let FilterKind::PerAxis(constants, _) = filter {
        let cases = uncertainty_edge_cases(graph, points, constants);
        write_uncertainty_cases(out, &cases, points);
    }
}

/// Writes `uncertainty_edge_cases` and its table, for `points` points.
fn write_uncertainty_cases(out: &mut String, cases: &[([f64; 3], Case, Case)], points: usize) {
    let ty = format!("[[f64; 3]; {points}]");
    out.push('\n');
    comment(
        out,
        "///",
        0,
        "Inputs on either side of the error bound of the filter with the uncertainty \
         `e` given with each, in pairs with the same `e`: the first of each pair has an \
         exact magnitude at most the bound, `EPSILON*d^G` plus \
         `KAPPA[k]*e[k]*d^G/d_k` on each axis, the second above it, where `d_k` is \
         the largest difference of coordinates on axis `k` plus `e[k]`, as the filter \
         computes it; the two differ by one representable value in one coordinate.\n\n\
         `e` is not zero on one axis only, for each axis, or on all of them: on those \
         axes it is a power of two between `2^-40` and `2^-8` times the scale of the \
         grid. The points are built as in [`bound_edge_cases`].",
    );
    writeln!(
        out,
        "pub fn uncertainty_edge_cases() -> Vec<({ty}, [f64; 3])> {{\n\
         UNCERTAINTY_EDGE_CASES\n\
         .into_iter()\n\
         .map(|(case, e)| (case.map(|point| point.map(f64::from_bits)), e.map(f64::from_bits)))\n\
         .collect()\n}}\n"
    )
    .unwrap();
    comment(
        out,
        "///",
        0,
        "The bit patterns of the coordinates and of the uncertainty of \
         [`uncertainty_edge_cases`].",
    );
    writeln!(
        out,
        "const UNCERTAINTY_EDGE_CASES: [([[u64; 3]; {points}], [u64; 3]); {}] = [",
        2 * cases.len()
    )
    .unwrap();
    for (e, below, above) in cases {
        let e = bits(e);
        writeln!(out, "({}, {e}),", case(below)).unwrap();
        writeln!(out, "({}, {e}),", case(above)).unwrap();
    }
    out.push_str("];\n");
}

/// The bit patterns of the coordinates of `case`, as an array of points.
fn case(case: &Case) -> String {
    let points: Vec<String> = case.iter().map(bits).collect();
    format!("[{}]", points.join(", "))
}

/// The bit patterns of three coordinates, as an array.
fn bits(point: &[f64; 3]) -> String {
    let bits: Vec<String> = point
        .iter()
        .map(|x| format!("{:#018x}", x.to_bits()))
        .collect();
    format!("[{}]", bits.join(", "))
}
