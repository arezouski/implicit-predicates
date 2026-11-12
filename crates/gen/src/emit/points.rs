//! Rust code for the point types of a file.
//!
//! The generated module holds `PointError`, one type per declaration with
//! its constructors and its implementation of `Point`, and `AnyPoint`, an
//! enum of all of them. An explicit type (one `F64x3` returned as is) is a
//! transparent wrapper of `[f64; 3]`. Every other type stores its
//! parameters, the coordinates computed in `f64` and their error bounds,
//! computed by its constructor with the program of [`crate::error_bound`];
//! its exact coordinates are written by [`super::point_exact`]. The checks
//! module of [`super::point_checks`] gives the exact coordinates for tests.

use std::fmt::Write;

use super::point_checks::write_checks;
use super::point_eval::{write_error_helpers, write_exact_helpers};
use super::point_exact::ExactPlan;
use super::point_rules::{check_names, check_size};
use super::point_types::{write_any_point, write_explicit, write_implicit};
use super::{Generated, comment, literal, notice};
use crate::error::Error;
use crate::point::{
    ErrorProgram, LinearSum, ParamType, PointNode, PointType, error_bound, parse_points,
};

/// The largest graph whose coordinates the documentation spells out.
pub(crate) const MAX_RENDERED_NODES: usize = 64;

/// Generates the module of the point types declared in `source`, or `None`
/// if it declares none. `file` and `regenerate` are as for
/// [`generate`](super::generate).
///
/// The module, named `points`, holds `PointError`, the error of the
/// constructors; for each declared type, a struct with `new`, which rejects
/// inputs that are not finite and parameters outside the `where` clause,
/// `new_unchecked`, which accepts anything, one accessor per parameter of an
/// implicit type, and an implementation of `Point`; and `AnyPoint`, an enum
/// with one variant per type, which implements `Point` too and converts
/// from each type with `From`. Its checks module holds, for each type that
/// is not explicit, `exact_<name>`, its exact coordinates evaluated with
/// the exact numbers of `implicit-predicates-oracle`.
///
/// The constructor of a type that is not explicit evaluates its
/// coordinates in `f64` with an error bound for each, by the rule of
/// [`crate::error_bound`]; `Point::exact` writes them as expansions, and
/// fails with `ExactError::OutOfRange` when an input is not finite, an
/// operand of a sum of expansions reaches `2^1020`, or an operand of an
/// exact product leaves the product domain of the core.
///
/// # Errors
///
/// A syntax error in a declaration; a point type whose name does not start
/// with an uppercase letter; a parameter whose name does not start with a
/// lowercase letter; a parameter or type with a name that the generated code
/// uses; an integer that is not exactly an `f64`; an error bound with more
/// roundings than [`crate::MAX_ROUNDS`]; exact coordinates whose temporaries
/// take more than 4,096 `f64`; coordinates of more than
/// [`crate::MAX_POINT_NODES`] distinct operations and inputs; or a `where`
/// comparison that, after cancelling the parameters on both sides, is not
/// one parameter against an integer of magnitude at most `2^53`, the only
/// form emitted for now.
pub fn generate_points(
    source: &str,
    file: &str,
    regenerate: &str,
) -> Result<Option<Generated>, Error> {
    let points = parse_points(source, file)?;
    if points.is_empty() {
        return Ok(None);
    }
    let mut types = Vec::new();
    for point in &points {
        types.push(prepare(point, file, &types)?);
    }
    let notice = notice(file, regenerate);
    let mut stages = String::new();
    write_module(&mut stages, &types, &notice);
    let mut checks = String::new();
    write_checks(&mut checks, &types, &notice);
    Ok(Some(Generated {
        name: "points".to_string(),
        stages,
        checks,
    }))
}

/// A point type with what its emission needs.
pub(crate) struct TypeInfo<'a> {
    pub(crate) point: &'a PointType,
    /// For each parameter, its index among the parameters of its kind.
    pub(crate) slot: Vec<usize>,
    /// The number of `F64x3` and of `f64` parameters.
    pub(crate) vectors: usize,
    pub(crate) scalars: usize,
    /// The name of its function in the checks.
    pub(crate) check_name: String,
    /// For a type that is not explicit, its error program, slack factor,
    /// exact plan and the comparisons of its `where` clause.
    pub(crate) implicit: Option<Implicit>,
}

pub(crate) struct Implicit {
    pub(crate) program: ErrorProgram,
    pub(crate) rounds: u32,
    pub(crate) slack: f64,
    pub(crate) exact: ExactPlan,
    /// Each comparison of the `where` clause, as the parameter it bounds
    /// and the bound.
    pub(crate) bounds: Vec<(usize, Bound)>,
}

/// One comparison of a `where` clause, reduced to one parameter.
#[derive(Clone, Copy)]
pub(crate) enum Bound {
    AtLeast(f64),
    AtMost(f64),
}

impl TypeInfo<'_> {
    /// The text of a leaf: `c[i][k]` for a coordinate, `s[i]` for an `f64`
    /// parameter, a literal for an integer.
    pub(crate) fn leaf(&self, node: PointNode) -> Option<String> {
        match node {
            PointNode::Coord { param, axis } => Some(format!("c[{}][{axis}]", self.slot[param])),
            PointNode::Scalar(param) => Some(format!("s[{}]", self.slot[param])),
            #[allow(clippy::cast_precision_loss)]
            PointNode::Int(n) => Some(literal(n as f64)),
            _ => None,
        }
    }

    /// The parameters of `evaluate` and `write_exact`, after any others.
    pub(crate) fn array_params(&self) -> String {
        let mut text = format!("c: [[f64; 3]; {}]", self.vectors);
        if self.scalars > 0 {
            write!(text, ", s: [f64; {}]", self.scalars).unwrap();
        }
        text
    }

    /// The arguments that pass `values` (each parameter, by name or as
    /// `self.name`) to `evaluate` or `write_exact`.
    pub(crate) fn array_args(&self, prefix: &str) -> String {
        let of = |ty: ParamType| {
            let names: Vec<String> = (self.point.params.iter())
                .filter(|p| p.ty == ty)
                .map(|p| format!("{prefix}{}", p.name))
                .collect();
            format!("[{}]", names.join(", "))
        };
        let mut text = of(ParamType::F64x3);
        if self.scalars > 0 {
            write!(text, ", {}", of(ParamType::F64)).unwrap();
        }
        text
    }
}

/// Checks `point` and derives what its emission needs; `earlier` are the
/// types before it.
fn prepare<'a>(
    point: &'a PointType,
    file: &str,
    earlier: &[TypeInfo<'_>],
) -> Result<TypeInfo<'a>, Error> {
    check_names(point, file)?;
    let check_name = format!("exact_{}", snake_case(&point.name));
    if let Some(other) = earlier.iter().find(|t| t.check_name == check_name) {
        return Err(Error::at(
            file,
            point.position,
            format!(
                "`{}` and `{}` would both be checked by `{check_name}`: rename one of them",
                other.point.name, point.name
            ),
        ));
    }
    let mut counts = [0, 0];
    let slot = (point.params.iter())
        .map(|p| {
            let kind = usize::from(p.ty == ParamType::F64);
            counts[kind] += 1;
            counts[kind] - 1
        })
        .collect();
    let implicit = if point.is_explicit() {
        None
    } else {
        let bound = error_bound(point, file)?;
        let program = ErrorProgram::of(point);
        Some(Implicit {
            program,
            rounds: bound.rounds.into_iter().max().unwrap_or(0),
            slack: bound.slack,
            exact: ExactPlan::of(point, file)?,
            bounds: constraint_bounds(point, file)?,
        })
    };
    // After the other checks, which report a long chain of roundings or
    // large exact coordinates more precisely.
    check_size(point, file)?;
    Ok(TypeInfo {
        point,
        slot,
        vectors: counts[0],
        scalars: counts[1],
        check_name,
        implicit,
    })
}

/// `CamelCase` as `camel_case`.
fn snake_case(name: &str) -> String {
    let mut out = String::new();
    let mut previous_lower = false;
    for c in name.chars() {
        if c.is_uppercase() && previous_lower {
            out.push('_');
        }
        previous_lower = c.is_lowercase() || c.is_ascii_digit();
        out.extend(c.to_lowercase());
    }
    out
}

/// The comparisons of the `where` clause of `point`, each reduced to one
/// parameter against an `f64` that is exactly the integer of the source.
fn constraint_bounds(point: &PointType, file: &str) -> Result<Vec<(usize, Bound)>, Error> {
    let mut bounds = Vec::new();
    for constraint in &point.constraints {
        // lhs <= rhs  <=>  sum of coefficient * parameter <= k.
        let mut coefficients = vec![0i64; point.params.len()];
        let count = |sum: &LinearSum, sign: i64, coefficients: &mut Vec<i64>| {
            for &p in &sum.params {
                coefficients[p] = coefficients[p].saturating_add(sign);
            }
        };
        count(&constraint.lhs, 1, &mut coefficients);
        count(&constraint.rhs, -1, &mut coefficients);
        let k = i128::from(constraint.rhs.constant) - i128::from(constraint.lhs.constant);
        let used: Vec<(usize, i64)> = (coefficients.iter().copied().enumerate())
            .filter(|&(_, c)| c != 0)
            .collect();
        let limit = 1i128 << 53;
        let bound = match used.as_slice() {
            [(param, 1)] if k.abs() <= limit => (*param, Bound::AtMost(k as f64)),
            [(param, -1)] if k.abs() <= limit => (*param, Bound::AtLeast(-k as f64)),
            _ => {
                return Err(Error::at(
                    file,
                    constraint.position,
                    "only a comparison of one parameter with an integer of magnitude at most \
                     2^53, once the parameters on both sides cancel, can be checked exactly \
                     for now",
                ));
            }
        };
        bounds.push(bound);
    }
    Ok(bounds)
}

/// Writes the module of the point types `types`, starting with `notice`.
fn write_module(out: &mut String, types: &[TypeInfo<'_>], notice: &str) {
    out.push_str(notice);
    let names: Vec<String> = types
        .iter()
        .map(|t| format!("[`{}`]", t.point.name))
        .collect();
    comment(
        out,
        "//!",
        0,
        &format!(
            "The point types {}, and [`AnyPoint`], which holds a point of any of them.",
            names.join(", ")
        ),
    );
    let implicit: Vec<&Implicit> = types.iter().filter_map(|t| t.implicit.as_ref()).collect();
    let mut core = vec!["ExactCoords", "ExactError", "Point", "Scratch"];
    if !implicit.is_empty() {
        core.extend(["Mark", "ScratchExhausted", "Span", "Weight"]);
        if implicit.iter().any(|i| i.exact.uses_products()) {
            core.push("eft");
        }
        core.push("expansion");
    }
    writeln!(
        out,
        "\nuse core::fmt;\n\nuse implicit_predicates_core::{{{}}};\n",
        core.join(", ")
    )
    .unwrap();
    write_error(out);
    for info in types {
        match &info.implicit {
            None => write_explicit(out, info),
            Some(implicit) => write_implicit(out, info, implicit),
        }
    }
    write_any_point(out, types);
    if !implicit.is_empty() {
        write_error_helpers(out, &implicit);
        write_exact_helpers(out, &implicit);
    }
}

fn write_error(out: &mut String) {
    out.push_str(
        "/// Why the constructor of a point type rejected its inputs.\n\
         #[derive(Clone, Copy, Debug, PartialEq, Eq)]\n\
         pub enum PointError {\n\
         /// An input is NaN or infinite.\n\
         NonFinite,\n\
         /// The parameters do not satisfy the conditions of the type.\n\
         ParamOutOfRange,\n\
         }\n\n\
         impl fmt::Display for PointError {\n\
         fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {\n\
         f.write_str(match self {\n\
         PointError::NonFinite => \"an input of the point is not finite\",\n\
         PointError::ParamOutOfRange => \"a parameter of the point is out of range\",\n\
         })\n}\n}\n\n\
         impl core::error::Error for PointError {}\n\n",
    );
}
