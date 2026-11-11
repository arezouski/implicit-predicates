//! The checks of a predicate: code for its tests, not for its users.
//!
//! The generated module holds `exact`, the main form evaluated with the
//! exact dyadic numbers of `indirect-predicates-oracle`, one statement per
//! node of the difference form as in the filter, and `bound_edge_cases`,
//! inputs on either side of the error bound of the filter, built by
//! [`super::edges`] and written as bit patterns so that they read back as
//! exactly the same `f64`s.

use std::fmt::Write;

use super::edges::{Case, bound_edge_cases};
use super::statements::node_statements;
use super::{comment, write_notice};
use crate::bound::FilterConstants;
use crate::differences::DiffGraph;
use crate::parse::Predicate;

/// Writes the checks module of `predicate`, whose difference form is
/// `graph` and whose filter has the emitted `constants`.
pub(crate) fn write_checks(
    out: &mut String,
    predicate: &Predicate,
    graph: &DiffGraph,
    constants: &FilterConstants,
    file: &str,
) {
    let points = predicate.params.len();
    let ty = format!("[[f64; 3]; {points}]");
    write_notice(out, file);
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
    out.push_str("\nuse indirect_predicates::Sign;\nuse indirect_predicates_oracle::Dyadic;\n\n");
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
    let cases = bound_edge_cases(graph, points, constants);
    comment(
        out,
        "///",
        0,
        &format!(
            "Inputs on either side of the error bound of the filter for explicit points, \
             in pairs: the first of each pair has an exact magnitude at most \
             `EPSILON * delta^{0}`, the second above it, where `delta` is the largest \
             difference of coordinates as the filter computes it; the two differ by one \
             representable value in one coordinate.\n\n\
             Each pair starts from points on a grid at which the predicate is zero, or \
             changes sign, and moves one coordinate away from there. The scales go from \
             just above `D_MIN` to just below `D_MAX`.",
            constants.degree
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
        write_case(out, below);
        write_case(out, above);
    }
    out.push_str("];\n");
}

fn write_case(out: &mut String, case: &Case) {
    let points: Vec<String> = case
        .iter()
        .map(|point| {
            let bits: Vec<String> = point
                .iter()
                .map(|x| format!("{:#018x}", x.to_bits()))
                .collect();
            format!("[{}]", bits.join(", "))
        })
        .collect();
    writeln!(out, "[{}],", points.join(", ")).unwrap();
}
