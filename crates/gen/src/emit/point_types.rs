//! The text of each point type: its struct, constructors, accessors and
//! implementation of `Point`, and `AnyPoint`.

use std::fmt::Write;

use super::point_big::write_exact_big;
use super::point_eval::write_evaluate;
use super::point_exact::write_exact;
use super::points::{Bound, Implicit, MAX_RENDERED_NODES, TypeInfo};
use super::{comment, list, literal};
use crate::point::{LinearSum, ParamType, PointNode, PointType};

/// `p: [f64; 3], q: [f64; 3], t: f64`.
fn signature(point: &PointType) -> String {
    let params: Vec<String> = (point.params.iter())
        .map(|p| match p.ty {
            ParamType::F64x3 => format!("{}: [f64; 3]", p.name),
            ParamType::F64 => format!("{}: f64", p.name),
        })
        .collect();
    params.join(", ")
}

/// The condition that every input of `point` is finite.
fn all_finite(point: &PointType) -> String {
    let parts: Vec<String> = (point.params.iter())
        .map(|p| match p.ty {
            ParamType::F64x3 => format!("{}.iter().all(|x| x.is_finite())", p.name),
            ParamType::F64 => format!("{}.is_finite()", p.name),
        })
        .collect();
    parts.join(" && ")
}

pub(crate) fn write_explicit(out: &mut String, info: &TypeInfo<'_>) {
    let point = info.point;
    let (name, param) = (&point.name, &point.params[0].name);
    comment(
        out,
        "///",
        0,
        &format!(
            "A point of type `{name}`: the point with the coordinates `{param}`, given \
             exactly. Its approximation is its coordinates and its error is zero."
        ),
    );
    writeln!(
        out,
        "#[derive(Clone, Copy, Debug, PartialEq)]\n#[repr(transparent)]\npub struct {name}([f64; 3]);\n\n\
         impl {name} {{"
    )
    .unwrap();
    comment(
        out,
        "///",
        0,
        &format!(
            "The point with the coordinates `{param}`.\n\n# Errors\n\n\
             [`PointError::NonFinite`] if a coordinate is NaN or infinite."
        ),
    );
    writeln!(
        out,
        "#[inline]\npub fn new({param}: [f64; 3]) -> Result<Self, PointError> {{\n\
         if !({}) {{\nreturn Err(PointError::NonFinite);\n}}\n\
         Ok(Self::new_unchecked({param}))\n}}\n",
        all_finite(point)
    )
    .unwrap();
    comment(
        out,
        "///",
        0,
        &format!(
            "The point with the coordinates `{param}`, without checking them. A \
             coordinate that is not finite makes [`Point::exact`] fail with \
             [`ExactError::OutOfRange`]."
        ),
    );
    writeln!(
        out,
        "#[inline]\npub fn new_unchecked({param}: [f64; 3]) -> Self {{\nSelf({param})\n}}\n}}\n\n\
         impl Point for {name} {{\n\
         #[inline]\nfn approx(&self) -> &[f64; 3] {{\n&self.0\n}}\n\n\
         #[inline]\nfn err(&self) -> &[f64; 3] {{\n&[0.0; 3]\n}}\n\n\
         #[inline]\nfn is_explicit(&self) -> bool {{\ntrue\n}}\n\n\
         #[inline]\nfn exact(&self, scratch: &mut Scratch<'_>) -> Result<ExactCoords, ExactError> {{\n\
         self.0.exact(scratch)\n}}\n\n\
         #[inline]\nfn exact_big(&self, scratch: &mut Scratch<'_>) -> Result<BigCoords, ExactError> {{\n\
         self.0.exact_big(scratch)\n}}\n}}\n"
    )
    .unwrap();
}

/// Writes the documentation of the coordinates of `point` and of its
/// conditions, as the first paragraphs of its doc comment.
fn write_description(out: &mut String, point: &PointType) {
    let names: Vec<String> = point.params.iter().map(|p| p.name.clone()).collect();
    let small = point.graph.nodes.len() <= MAX_RENDERED_NODES;
    let ending = if small { ":" } else { " by its declaration." };
    comment(
        out,
        "///",
        0,
        &format!(
            "A point of type `{}`, computed from {}{ending}",
            point.name,
            list(&names)
        ),
    );
    let mut lines = Vec::new();
    if small {
        for (k, axis) in ["x", "y", "z"].iter().enumerate() {
            lines.push(format!("{axis} = {}", render(point, k)));
        }
    }
    if !point.constraints.is_empty() {
        let shown: Vec<String> = (point.constraints.iter())
            .map(|c| format!("{} <= {}", sum_text(point, &c.lhs), sum_text(point, &c.rhs)))
            .collect();
        lines.push(format!("where {}", shown.join(", ")));
    }
    if !lines.is_empty() {
        out.push_str("///\n/// ```text\n");
        for line in lines {
            writeln!(out, "/// {line}").unwrap();
        }
        out.push_str("/// ```\n");
    }
}

/// `a + b + 3`.
fn sum_text(point: &PointType, sum: &LinearSum) -> String {
    let mut parts: Vec<String> = sum
        .params
        .iter()
        .map(|&p| point.params[p].name.clone())
        .collect();
    if sum.constant != 0 || parts.is_empty() {
        parts.push(sum.constant.to_string());
    }
    parts.join(" + ")
}

/// Coordinate `axis` of `point` as an expression, with the brackets that
/// keep the order of its operations. The graph is small, so the recursion
/// is shallow.
fn render(point: &PointType, axis: usize) -> String {
    fn go(point: &PointType, id: usize) -> (String, u8) {
        let name = |param: usize| point.params[param].name.as_str();
        let wrap = |(text, precedence): (String, u8), min: u8| {
            if precedence >= min {
                text
            } else {
                format!("({text})")
            }
        };
        match point.graph.nodes[id] {
            PointNode::Coord { param, axis } => (
                format!("{}.{}", name(param), ["x", "y", "z"][usize::from(axis)]),
                4,
            ),
            PointNode::Scalar(param) => (name(param).to_string(), 4),
            PointNode::Int(n) => (n.to_string(), if n < 0 { 3 } else { 4 }),
            PointNode::Neg(x) => (format!("-{}", wrap(go(point, x), 4)), 3),
            PointNode::Add(x, y) => (
                format!("{} + {}", wrap(go(point, x), 1), wrap(go(point, y), 2)),
                1,
            ),
            PointNode::Sub(x, y) => (
                format!("{} - {}", wrap(go(point, x), 1), wrap(go(point, y), 2)),
                1,
            ),
            PointNode::Mul(x, y) => (
                format!("{} * {}", wrap(go(point, x), 2), wrap(go(point, y), 3)),
                2,
            ),
        }
    }
    go(point, point.graph.roots[axis]).0
}

pub(crate) fn write_implicit(out: &mut String, info: &TypeInfo<'_>, implicit: &Implicit) {
    let point = info.point;
    let name = &point.name;
    write_description(out, point);
    out.push_str("///\n");
    comment(
        out,
        "///",
        0,
        "The constructors compute the coordinates in `f64` with a bound on the error of \
         each one, [`Point::approx`] and [`Point::err`]; [`Point::exact`] computes them \
         exactly.",
    );
    out.push_str("#[derive(Clone, Copy, Debug, PartialEq)]\n");
    writeln!(out, "pub struct {name} {{").unwrap();
    for p in &point.params {
        let ty = if p.ty == ParamType::F64x3 {
            "[f64; 3]"
        } else {
            "f64"
        };
        writeln!(out, "{}: {ty},", p.name).unwrap();
    }
    out.push_str("approx: [f64; 3],\nerr: [f64; 3],\n}\n\n");
    writeln!(out, "impl {name} {{").unwrap();
    comment(
        out,
        "///",
        0,
        &format!(
            "The slack factor of the error bound: the smallest `f64` `S` with \
             `S * (1 - 2^-53)^{0} >= 1`, where {0} is the number of roundings of the \
             computation of the error of a coordinate on its longest chain of operations.",
            implicit.rounds
        ),
    );
    writeln!(out, "const ERR_SLACK: f64 = {};\n", literal(implicit.slack)).unwrap();
    let names: Vec<String> = point.params.iter().map(|p| p.name.clone()).collect();
    let mut errors = String::from("[`PointError::NonFinite`] if an input is NaN or infinite");
    if !point.constraints.is_empty() {
        errors.push_str(
            ", and [`PointError::ParamOutOfRange`] if the parameters do not satisfy the \
             conditions of the type",
        );
    }
    comment(
        out,
        "///",
        0,
        &format!(
            "The point of type `{name}` with the parameters {}.\n\n# Errors\n\n{errors}.",
            list(&names)
        ),
    );
    let args: Vec<&str> = point.params.iter().map(|p| p.name.as_str()).collect();
    writeln!(
        out,
        "pub fn new({}) -> Result<Self, PointError> {{\nif !({}) {{\nreturn Err(PointError::NonFinite);\n}}",
        signature(point),
        all_finite(point)
    )
    .unwrap();
    for &bound in &implicit.bounds {
        let name = |param: usize| &point.params[param].name;
        let reject = "return Err(PointError::ParamOutOfRange);";
        match bound {
            Bound::AtLeast(param, k) => {
                writeln!(out, "if {} < {} {{\n{reject}\n}}", name(param), literal(k)).unwrap();
            }
            Bound::AtMost(param, k) => {
                writeln!(out, "if {} > {} {{\n{reject}\n}}", name(param), literal(k)).unwrap();
            }
            // For finite `a` and `b` whose sum does not overflow, two-sum
            // gives `s` and `e` with `a + b = s + e` exactly, so `a + b <= k`
            // if and only if `s < k`, or `s == k` and `e <= 0`. If the sum
            // overflows, `s` is infinite, `s < k` alone gives the answer and
            // `e`, which may be NaN, is not read: `s == k` is false. The block
            // keeps `s` and `e` from hiding parameters of those names.
            Bound::SumAtMost(a, b, k) => {
                let k = literal(k);
                writeln!(
                    out,
                    "{{\nlet (s, e) = eft::two_sum({}, {});\n\
                     if !(s < {k} || (s == {k} && e <= 0.0)) {{\n{reject}\n}}\n}}",
                    name(a),
                    name(b)
                )
                .unwrap();
            }
        }
    }
    writeln!(out, "Ok(Self::new_unchecked({}))\n}}\n", args.join(", ")).unwrap();
    comment(
        out,
        "///",
        0,
        &format!(
            "The point of type `{name}` with the parameters {}, without checking them: any \
             `f64` is accepted. A coordinate whose approximation or error bound is not finite \
             gets an error of `+∞`, and [`Point::exact`] fails with \
             [`ExactError::OutOfRange`] when an input is not finite.",
            list(&names)
        ),
    );
    writeln!(
        out,
        "pub fn new_unchecked({}) -> Self {{\nlet (approx, err) = Self::evaluate({});\n\
         Self {{\n{},\napprox,\nerr,\n}}\n}}\n",
        signature(point),
        info.array_args(""),
        args.join(",\n")
    )
    .unwrap();
    for p in &point.params {
        comment(out, "///", 0, &format!("The parameter `{}`.", p.name));
        let (ty, value) = match p.ty {
            ParamType::F64x3 => ("&[f64; 3]", "&self."),
            ParamType::F64 => ("f64", "self."),
        };
        writeln!(
            out,
            "#[inline]\npub fn {0}(&self) -> {ty} {{\n{value}{0}\n}}\n",
            p.name
        )
        .unwrap();
    }
    write_evaluate(out, info, implicit);
    write_exact(out, info, implicit);
    write_exact_big(out, info);
    writeln!(
        out,
        "}}\n\nimpl Point for {name} {{\n\
         #[inline]\nfn approx(&self) -> &[f64; 3] {{\n&self.approx\n}}\n\n\
         #[inline]\nfn err(&self) -> &[f64; 3] {{\n&self.err\n}}\n\n\
         #[inline]\nfn is_explicit(&self) -> bool {{\nfalse\n}}\n\n\
         fn exact(&self, scratch: &mut Scratch<'_>) -> Result<ExactCoords, ExactError> {{\n\
         Self::write_exact(scratch, {0})\n}}\n\n\
         fn exact_big(&self, scratch: &mut Scratch<'_>) -> Result<BigCoords, ExactError> {{\n\
         Self::write_exact_big(scratch, {0})\n}}\n}}\n",
        info.array_args("self.")
    )
    .unwrap();
}

pub(crate) fn write_any_point(out: &mut String, types: &[TypeInfo<'_>]) {
    out.push_str(
        "/// A point of any of the types of this module.\n\
         #[derive(Clone, Copy, Debug, PartialEq)]\npub enum AnyPoint {\n",
    );
    for t in types {
        let name = &t.point.name;
        writeln!(out, "/// A point of type [`{name}`].\n{name}({name}),").unwrap();
    }
    out.push_str("}\n\nimpl Point for AnyPoint {\n");
    let methods = [
        ("approx", "&self", "&[f64; 3]", "p.approx()"),
        ("err", "&self", "&[f64; 3]", "p.err()"),
        ("is_explicit", "&self", "bool", "p.is_explicit()"),
        (
            "exact",
            "&self, scratch: &mut Scratch<'_>",
            "Result<ExactCoords, ExactError>",
            "p.exact(scratch)",
        ),
        (
            "exact_big",
            "&self, scratch: &mut Scratch<'_>",
            "Result<BigCoords, ExactError>",
            "p.exact_big(scratch)",
        ),
    ];
    for (i, (method, params, ty, call)) in methods.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        writeln!(
            out,
            "#[inline]\nfn {method}({params}) -> {ty} {{\nmatch self {{"
        )
        .unwrap();
        for t in types {
            writeln!(out, "AnyPoint::{0}(p) => {call},", t.point.name).unwrap();
        }
        out.push_str("}\n}\n");
    }
    out.push_str("}\n");
    for t in types {
        writeln!(
            out,
            "\nimpl From<{0}> for AnyPoint {{\n#[inline]\nfn from(p: {0}) -> Self {{\nAnyPoint::{0}(p)\n}}\n}}",
            t.point.name
        )
        .unwrap();
    }
    out.push('\n');
}
