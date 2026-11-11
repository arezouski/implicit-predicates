//! The checks of the point types: code for their tests, not for their
//! users.
//!
//! For each type that is not explicit the module holds `exact_<name>`, its
//! coordinates evaluated with the exact dyadic numbers of
//! `implicit-predicates-oracle`, one statement per node of its graph.

use std::fmt::Write;

use super::points::TypeInfo;
use super::{comment, list};
use crate::point::PointNode;

/// Writes the checks module of the point types `types`, starting with
/// `notice`; nothing if every type is explicit.
pub(crate) fn write_checks(out: &mut String, types: &[TypeInfo<'_>], notice: &str) {
    let implicit: Vec<&TypeInfo<'_>> = types.iter().filter(|t| t.implicit.is_some()).collect();
    if implicit.is_empty() {
        return;
    }
    out.push_str(notice);
    let names: Vec<String> = (implicit.iter())
        .map(|t| format!("[`{}`]", t.check_name))
        .collect();
    comment(
        out,
        "//!",
        0,
        &format!(
            "Checks of the point types, for their tests: the exact coordinates of each \
             type that is not explicit, {}.",
            names.join(", ")
        ),
    );
    out.push_str("\nuse implicit_predicates_oracle::Dyadic;\n");
    for info in implicit {
        write_exact_check(out, info);
    }
}

fn write_exact_check(out: &mut String, info: &TypeInfo<'_>) {
    let point = info.point;
    let names: Vec<String> = point.params.iter().map(|p| p.name.clone()).collect();
    out.push('\n');
    comment(
        out,
        "///",
        0,
        &format!(
            "The exact coordinates of the point of type `{}` with the parameters {}, given \
             as the `F64x3` parameters `c` and the `f64` parameters `s` in the order of the \
             declaration: its expression evaluated with exact dyadic numbers.\n\n\
             # Panics\n\n\
             Panics if an input is not finite.",
            point.name,
            list(&names)
        ),
    );
    writeln!(
        out,
        "pub fn {}({}) -> [Dyadic; 3] {{\nlet c = c.map(|point| point.map(Dyadic::from_f64));",
        info.check_name,
        info.array_params()
    )
    .unwrap();
    if info.scalars > 0 {
        out.push_str("let s = s.map(Dyadic::from_f64);\n");
    }
    for (id, &node) in point.graph.nodes.iter().enumerate() {
        let value = match node {
            PointNode::Coord { .. } | PointNode::Scalar(_) => {
                format!("{}.clone()", info.leaf(node).expect("a leaf"))
            }
            PointNode::Int(_) => format!("Dyadic::from_f64({})", info.leaf(node).expect("a leaf")),
            PointNode::Neg(x) => format!("-&t{x}"),
            PointNode::Add(x, y) => format!("&t{x} + &t{y}"),
            PointNode::Sub(x, y) => format!("&t{x} - &t{y}"),
            PointNode::Mul(x, y) => format!("&t{x} * &t{y}"),
        };
        writeln!(out, "let t{id} = {value};").unwrap();
    }
    let roots = point.graph.roots;
    let mut results = Vec::new();
    for (k, &root) in roots.iter().enumerate() {
        // Move each root after its last use; clone it before.
        if roots[k + 1..].contains(&root) {
            results.push(format!("t{root}.clone()"));
        } else {
            results.push(format!("t{root}"));
        }
    }
    writeln!(out, "[{}]\n}}", results.join(", ")).unwrap();
}
