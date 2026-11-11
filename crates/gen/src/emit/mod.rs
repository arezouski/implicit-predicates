//! Rust code for the stages of a predicate.
//!
//! For each predicate the generated module holds its constants, the
//! floating-point filter ([`filter`]), three exact stages ([`exact`]) and the
//! chain that runs them ([`chain`]); a second module holds the checks that
//! its tests use ([`checks`]). The output is built as text, in a fixed
//! order, so it is the same on every run; it is not formatted.

mod chain;
mod checks;
mod edges;
mod exact;
mod filter;
mod program;
mod statements;

#[cfg(test)]
mod tests;

use std::fmt::Write;

use indirect_predicates_core::eft::{PRODUCT_OPERAND_MAX, PRODUCT_OPERAND_MIN};

use crate::bound::{FilterConstants, MAX_DEGREE, filter_bound};
use crate::differences::difference_graph;
use crate::error::Error;
use crate::parse::{Predicate, parse};
use crate::polynomial::check_same_polynomial;
use crate::range::{ExactRange, Leaves, exact_plan_differences, exact_plan_raw};
use exact::{Check, Stage, write_in_range, write_product, write_stage};
use program::{Program, Step};

/// The code generated for one predicate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Generated {
    /// The name of the predicate, and of the files to write.
    pub name: String,
    /// The module of its stages, unformatted.
    pub stages: String,
    /// The module of its checks, for its tests, unformatted: `exact`, the
    /// exact sign of the predicate, and `bound_edge_cases`, inputs on either
    /// side of the error bound of the filter. Empty when there are none, in
    /// which case no file is written.
    pub checks: String,
}

/// Generates the code of every predicate defined in `source`.
///
/// `file` labels errors and is named in the header of the generated code,
/// for instance `predicates/orient3d.pred`. `regenerate` tells how to
/// regenerate the code, for instance the command that produced it; it is
/// written in the header of every generated file after "To regenerate,",
/// one comment line per line of text. The command line tool passes the
/// command it was run with.
///
/// For each predicate the module of its stages holds the constants of its
/// filter and the ranges of its exact stages, and the functions `filter`,
/// `exact_differences`, `expansions_translated`, `expansions_untranslated`
/// (only when the predicate has an `#[exact]` form) and `staged`, which runs
/// the filter and then the exact stages in the order: differences that are
/// exact, the translated expansion form, then the `#[exact]` form if one is
/// given. The order is the same for every predicate for now. The module of
/// its checks holds `exact`, the main form evaluated with the exact numbers
/// of `indirect-predicates-oracle`, and `bound_edge_cases`, pairs of inputs
/// at the edge of the error bound of the filter.
///
/// # Errors
///
/// Returns the first problem found in the definitions: a syntax error, a
/// main form that is not homogeneous in differences of coordinates, a
/// degree above the 1022 that the filter accepts, a definition too large to
/// derive the filter bound for or to expand, an `#[exact]` form that is a
/// different polynomial, or a predicate that the exact stages cannot
/// support. The checks run from the cheapest to the most expensive, and
/// every expensive one has a budget that it checks before doing the work,
/// so a definition that is too large fails quickly; see [`crate::canonical`]
/// and [`crate::filter_bound`] for the limits.
pub fn generate(source: &str, file: &str, regenerate: &str) -> Result<Vec<Generated>, Error> {
    let notice = notice(file, regenerate);
    parse(source, file)?
        .iter()
        .map(|predicate| generate_one(predicate, file, &notice))
        .collect()
}

/// The forms of a predicate, with the plans of their exact stages.
struct Forms {
    constants: FilterConstants,
    differences: (Program, ExactRange),
    translated: (Program, ExactRange),
    untranslated: Option<(Program, ExactRange)>,
}

/// Generates the code of `predicate`, whose files start with `notice`.
fn generate_one(predicate: &Predicate, file: &str, notice: &str) -> Result<Generated, Error> {
    // The cheap checks come first: the difference form gives the degree of
    // the main form, one pass bounds the degree of the exact form, and the
    // filter bound checks its own size, all before any polynomial is
    // expanded.
    let position = predicate.position;
    let graph = difference_graph(&predicate.main, file, position)?;
    if let Some(exact) = &predicate.exact {
        let degree = exact.degree_bound();
        if degree > u64::from(MAX_DEGREE) {
            return Err(Error::at(
                file,
                predicate.exact_position.unwrap_or(position),
                format!(
                    "the exact form of `{}` has a degree of up to {degree}, above {MAX_DEGREE}, \
                     the highest degree the filter accepts",
                    predicate.name
                ),
            ));
        }
    }
    let constants = filter_bound(&graph, file, position)?.constants();
    check_same_polynomial(predicate, file)?;
    let (min, max) = (PRODUCT_OPERAND_MIN, PRODUCT_OPERAND_MAX);
    let plan = |leaves| {
        exact_plan_differences(&graph, leaves, file, position, min, max).map(|plan| {
            (
                Program::differences(&graph, leaves, &plan.lengths),
                plan.range,
            )
        })
    };
    let differences = plan(Leaves::One)?;
    let translated = plan(Leaves::Two)?;
    let untranslated = match &predicate.exact {
        Some(exact) => {
            let position = predicate.exact_position.unwrap_or(position);
            let plan = exact_plan_raw(exact, file, position, min, max)?;
            Some((Program::raw(exact, &plan.lengths), plan.range))
        }
        None => None,
    };
    let forms = Forms {
        constants,
        differences,
        translated,
        untranslated,
    };
    let points = predicate.params.len();
    let mut out = String::new();
    write_header(&mut out, predicate, notice);
    write_constants(&mut out, &forms);
    let uses_max = filter::write_filter(&mut out, &graph, points, constants.degree);
    write_exact_stages(&mut out, predicate, &forms);
    chain::write_chain(&mut out, points, forms.untranslated.is_some());
    if uses_max {
        filter::write_max(&mut out);
    }
    write_in_range(&mut out);
    let programs = [&forms.differences, &forms.translated]
        .into_iter()
        .chain(&forms.untranslated);
    if programs
        .flat_map(|(program, _)| &program.steps)
        .any(|step| matches!(step, Step::Product { .. }))
    {
        write_product(&mut out);
    }
    let mut checks = String::new();
    checks::write_checks(&mut checks, predicate, &graph, &constants, notice);
    Ok(Generated {
        name: predicate.name.clone(),
        stages: out,
        checks,
    })
}

/// The notice that starts every generated file: where it comes from, that
/// it must not be edited, and how to regenerate it, followed by a blank
/// line.
fn notice(file: &str, regenerate: &str) -> String {
    let mut out = format!("// Generated by indirect-predicates-gen from {file}. Do not edit.\n");
    for (i, line) in regenerate.lines().enumerate() {
        let line = if i == 0 {
            format!("To regenerate, {line}")
        } else {
            line.to_string()
        };
        writeln!(out, "// {line}").unwrap();
    }
    out.push('\n');
    out
}

fn write_header(out: &mut String, predicate: &Predicate, notice: &str) {
    out.push_str(notice);
    comment(
        out,
        "//!",
        0,
        &format!(
            "The stages of the predicate `{}`.\n\n\
             Every stage takes the points {} of the definition as `p`, in that order, \
             and returns the sign of the predicate, or `None` when it cannot decide. \
             [`staged`] runs them in order.",
            predicate.name,
            list(&predicate.params)
        ),
    );
    out.push_str("\nuse indirect_predicates_core::{Sign, eft, expansion};\n\n");
}

/// `a`, `b` and `c`.
fn list(names: &[String]) -> String {
    let quoted: Vec<String> = names.iter().map(|n| format!("`{n}`")).collect();
    match quoted.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        _ => quoted.concat(),
    }
}

fn write_constants(out: &mut String, forms: &Forms) {
    let c = &forms.constants;
    let constant = |out: &mut String, doc: &str, name: &str, ty: &str, value: String| {
        comment(out, "///", 0, doc);
        writeln!(out, "pub const {name}: {ty} = {value};\n").unwrap();
    };
    constant(
        out,
        "The degree `k` of the predicate in the differences of coordinates.",
        "DEGREE",
        "u32",
        c.degree.to_string(),
    );
    constant(
        out,
        "The coefficient of `D^k` in the error bound of [`filter`], \
         `(EPSILON * D + KAPPA * E) * D^(k - 1)`, rounded up. `E` is the `e` given to \
         the filter and `D` is `E` plus the largest magnitude of a difference of \
         coordinates.",
        "EPSILON",
        "f64",
        literal(c.epsilon),
    );
    constant(
        out,
        "The coefficient of `E * D^(k - 1)` in the error bound of [`filter`], rounded up.",
        "KAPPA",
        "f64",
        literal(c.kappa),
    );
    constant(
        out,
        &format!(
            "The smallest `D` for which [`filter`] decides, `2^{}`: below it, underflow \
             could exceed the error bound.",
            power_of_two(c.d_min)
        ),
        "D_MIN",
        "f64",
        literal(c.d_min),
    );
    constant(
        out,
        &format!(
            "The largest `D` for which [`filter`] decides, `2^{}`: above it, a computed \
             value could overflow.",
            power_of_two(c.d_max)
        ),
        "D_MAX",
        "f64",
        literal(c.d_max),
    );
    let range = |out: &mut String, name: &str, what: &str, range: ExactRange| {
        constant(
            out,
            &format!(
                "`(min, max)`, `(2^{}, 2^{})`: {what} has a magnitude in `min..=max`.",
                power_of_two(range.min),
                power_of_two(range.max)
            ),
            name,
            "(f64, f64)",
            format!("({}, {})", literal(range.min), literal(range.max)),
        );
    };
    range(
        out,
        "DIFFERENCES_RANGE",
        "[`exact_differences`] decides when every difference of coordinates is exact \
         in `f64` and every nonzero one",
        forms.differences.1,
    );
    range(
        out,
        "TRANSLATED_RANGE",
        "[`expansions_translated`] decides when every nonzero coordinate",
        forms.translated.1,
    );
    if let Some((_, r)) = &forms.untranslated {
        range(
            out,
            "UNTRANSLATED_RANGE",
            "[`expansions_untranslated`] decides when every nonzero coordinate",
            *r,
        );
    }
}

fn write_exact_stages(out: &mut String, predicate: &Predicate, forms: &Forms) {
    let points = predicate.params.len();
    let doc = "The exact sign of the predicate: its main form evaluated with expansions on \
         the differences of coordinates, each computed in `f64`.\n\n\
         `None` means that this stage cannot decide: some difference is not exact in \
         `f64`, or some nonzero difference has a magnitude outside [`DIFFERENCES_RANGE`], \
         as when a coordinate is not finite.";
    write_stage(
        out,
        &Stage {
            name: "exact_differences",
            doc,
            program: &forms.differences.0,
            check: Check::Differences("DIFFERENCES_RANGE"),
        },
        points,
    );
    let doc = "The exact sign of the predicate: its main form evaluated with expansions, \
         each difference of coordinates as an exact expansion of two components.\n\n\
         `None` means that this stage cannot decide: some nonzero coordinate has a \
         magnitude outside [`TRANSLATED_RANGE`] or is not finite.";
    write_stage(
        out,
        &Stage {
            name: "expansions_translated",
            doc,
            program: &forms.translated.0,
            check: Check::Coordinates("TRANSLATED_RANGE"),
        },
        points,
    );
    if let Some((program, _)) = &forms.untranslated {
        let doc = "The exact sign of the predicate: its form for exact evaluation, marked \
             `#[exact]` in the definition, evaluated with expansions on the coordinates.\n\n\
             `None` means that this stage cannot decide: some nonzero coordinate has a \
             magnitude outside [`UNTRANSLATED_RANGE`] or is not finite.";
        write_stage(
            out,
            &Stage {
                name: "expansions_untranslated",
                doc,
                program,
                check: Check::Coordinates("UNTRANSLATED_RANGE"),
            },
            points,
        );
    }
}

/// The width the generated comments are wrapped at, indentation included.
const COMMENT_WIDTH: usize = 80;

/// Writes `text` as comment lines that start with `marker` (`///`, `//!` or
/// `//`), indented by `indent` spaces as rustfmt places them, wrapped at
/// [`COMMENT_WIDTH`]; a blank line in `text` separates paragraphs.
pub(crate) fn comment(out: &mut String, marker: &str, indent: usize, text: &str) {
    let room = COMMENT_WIDTH.saturating_sub(indent + marker.len() + 1);
    for (i, paragraph) in text.split("\n\n").enumerate() {
        if i > 0 {
            writeln!(out, "{marker}").unwrap();
        }
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            if !line.is_empty() && line.len() + 1 + word.len() > room {
                writeln!(out, "{marker} {line}").unwrap();
                line.clear();
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        writeln!(out, "{marker} {line}").unwrap();
    }
}

/// The exponent of a normal power of two.
fn power_of_two(x: f64) -> i64 {
    debug_assert!(x.is_normal() && x.to_bits() & ((1 << 52) - 1) == 0);
    i64::try_from(x.to_bits() >> 52).expect("11 bits") - 1023
}

/// An `f64` literal that reads back as exactly `x`, which must be finite:
/// the shortest decimal that rounds to it.
pub(crate) fn literal(x: f64) -> String {
    debug_assert!(x.is_finite());
    format!("{x:?}")
}
