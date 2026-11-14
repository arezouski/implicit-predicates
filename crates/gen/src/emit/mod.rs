//! Rust code for the stages of a predicate.
//!
//! For each predicate the generated module holds its constants, the
//! floating-point filter ([`filter`]), the exact stages on the stack
//! ([`exact`]) and over spans of an arena ([`spans`]), the final stage on
//! big integers ([`big`]) and the chains that run them ([`chain`] and
//! [`generic`]); a second module holds the checks that its tests use
//! ([`checks`]). The output is built as text, in a fixed
//! order, so it is the same on every run; it is not formatted.

mod big;
mod chain;
mod check_fns;
mod checks;
mod edges;
mod exact;
mod filter;
mod filter_axis;
mod generic;
mod grid;
mod point_big;
mod point_checks;
mod point_eval;
mod point_exact;
mod point_rules;
mod point_types;
mod points;
mod program;
mod spans;
mod stages;
mod statements;
mod text;

#[cfg(test)]
mod tests;

use std::fmt::Write;

use implicit_predicates_core::eft::{PRODUCT_OPERAND_MAX, PRODUCT_OPERAND_MIN};

use crate::bound::{FilterForm, MAX_DEGREE, filter_form};
use crate::differences::{DiffGraph, difference_graph};
use crate::error::Error;
use crate::parse::{Position, Predicate, parse};
use crate::polynomial::checked_polynomial;
use crate::range::{
    Leaves, MAX_COORD_COMPONENTS, big_explicit_arena, exact_plan_differences,
    exact_plan_spans_translated,
};
use chain::ChainStage;
use exact::{write_in_range, write_product};
use filter::FilterKind;
pub use points::generate_points;
use program::{Program, Step};
use spans::write_span_in_range;
pub use stages::BIG_INTEGERS_ONLY_DEGREE;
use stages::{Expansions, Forms, fits_on_stack, write_constants, write_exact_stages};
pub(crate) use text::{comment, literal, power_of_two};

/// The code generated for one predicate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Generated {
    /// The name of the predicate, and of the files to write.
    pub name: String,
    /// The module of its stages, unformatted.
    pub stages: String,
    /// The module of its checks, for its tests, unformatted: `exact`, the
    /// exact sign of the predicate, `bound_edge_cases`, inputs on either
    /// side of the error bound of the filter, and for a per-axis filter
    /// `uncertainty_edge_cases`, the same with an uncertainty. Empty when
    /// there are none, in which case no file is written.
    pub checks: String,
    /// What the user of the generated code should know and the code itself
    /// cannot enforce, one message each: for example, that no input at the
    /// edge of the error bound was found, so that the checks have no such
    /// case. Empty for most definitions.
    pub warnings: Vec<String>,
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
/// filter and the ranges of its exact stages, and the functions `filter`
/// (with one size and one uncertainty per axis when every term of the main
/// form has the same degree in each axis, see [`crate::filter_form`]),
/// `exact_differences` and `expansions_translated` on stack buffers, the
/// expansion form over arena spans, `expansions_translated_in`, which takes
/// the exact coordinates of any point type, and `staged`, which runs the
/// filter and then the exact stages in the order: differences that are
/// exact, then the translated expansion form. The order is the same for
/// every predicate that has them. No stage evaluates the `#[exact]` form: it
/// is only checked to be the same polynomial as the main form. A stage on
/// the stack is emitted only when its temporaries take at most 4,096 `f64`;
/// when one does not fit, `staged` takes an arena and runs the stage over
/// spans of the same form in its place, on the exact coordinates of the
/// explicit points. For points of any type that implements `Point`,
/// `staged_in` runs `filter_points`, the filter with the uncertainty of the
/// errors of the points, then `exact_stages`, the exact stages of `staged`,
/// when every point is explicit, and otherwise `exact_stages_in`, the stage
/// over spans on the exact coordinates of the points.
///
/// A predicate whose main form has a total degree of
/// [`BIG_INTEGERS_ONLY_DEGREE`] or more has no stage on expansions: none is
/// planned or emitted, nor their ranges, `MAX_COORD_COMPONENTS` or
/// `exact_stages`. Its `staged` is the filter alone, without an arena, and
/// its `exact_stages_explicit_in` and `exact_stages_in` run only the stage
/// on big integers below, so its chain asks the points for their exact
/// coordinates as big integers and never as expansions.
/// `big_integers_in` evaluates the main form exactly with the big-integer
/// operations of an arena, on the coordinates of the points as big integers,
/// and `BIG_INTEGERS_EXPLICIT_ARENA` is a number of `f64` slots of the arena
/// that suffices for it on explicit points. It always decides, and ends the
/// chain for points of any type: `exact_stages_explicit_in` runs it after
/// `exact_stages`, and `exact_stages_in` after the stage over spans, so
/// `staged_in` decides every input, and panics on a point that cannot give
/// its coordinates as big integers or whose exact coordinates have a weight
/// other than one. The module of its checks holds `exact`, the main form
/// evaluated with the exact numbers of `implicit-predicates-oracle`,
/// `bound_edge_cases`, pairs of inputs at the edge of the error bound of the
/// filter for explicit points, and, for a per-axis filter,
/// `uncertainty_edge_cases`, the same with an uncertainty.
///
/// # Errors
///
/// Returns the first problem found in the definitions: a syntax error, a
/// main form that is not homogeneous in differences of coordinates, a
/// degree above the 1022 that the filter accepts, a definition too large to
/// derive the filter bound for or to expand, an `#[exact]` form that is a
/// different polynomial, a main form with more than 256 distinct
/// subexpressions, or a predicate that the exact stages cannot support: for
/// a main form of degree below [`BIG_INTEGERS_ONLY_DEGREE`], one without a
/// range for its stage over spans or too large for it, and for every
/// predicate, one whose stage on big integers would need an arena of more
/// than `u32::MAX` words for explicit points.
///
/// # Work
///
/// The checks run from the cheapest to the most expensive, and each step
/// whose work can grow faster than the size of the definition has a budget,
/// checked before the work it limits:
///
/// - [`parse`] nests brackets at most 32 deep, and the difference form and
///   the degree of the `#[exact]` form take one pass each;
/// - [`crate::filter_bound`] keeps its exact coefficients within `2^26`
///   bits in all;
/// - [`crate::canonical`] keeps every expansion within `2^16` terms, `2^16`
///   bits per coefficient and `2^20` units of work;
/// - the exact plans on expansions, derived only for main forms of degree
///   below [`BIG_INTEGERS_ONLY_DEGREE`], reject temporaries of `2^32`
///   components or more, or of more than `2^51` operations on the stack and
///   `2^50` over spans, and take a few passes over the graph for each power
///   of two they try; the plans over spans stop each pass at the first node
///   whose bound overflows;
/// - the search for the edge cases of the checks, the most expensive step
///   per node, runs only on main forms of at most 256 distinct
///   subexpressions, and makes at most a fixed number of exact evaluations
///   of the form.
///
/// So the work of `generate` is bounded for every definition, and the
/// slowest definitions accepted take a few seconds in a debug build, most
/// of it in the search for the edge cases; a definition beyond a budget is
/// rejected with an error that names the limit.
pub fn generate(source: &str, file: &str, regenerate: &str) -> Result<Vec<Generated>, Error> {
    let notice = notice(file, regenerate);
    parse(source, file)?
        .iter()
        .map(|predicate| generate_one(predicate, file, &notice))
        .collect()
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
    let filter = match filter_form(&graph, file, position)? {
        FilterForm::PerAxis(bound) => FilterKind::PerAxis(bound.constants(), bound.certain_zero()),
        FilterForm::Global(bound) => FilterKind::Global(bound.constants()),
    };
    let affine = checked_polynomial(predicate, file)?.is_affine_in_each_coordinate();
    if graph.nodes.len() > edges::MAX_NODES {
        return Err(Error::at(
            file,
            position,
            format!(
                "the main form of `{}` has {} distinct subexpressions, more than the {} \
                 for which the checks of its tests are generated: their edge cases take \
                 thousands of exact evaluations of the whole form",
                predicate.name,
                graph.nodes.len(),
                edges::MAX_NODES
            ),
        ));
    }
    // A main form of high degree has no stage on expansions, so none is
    // planned.
    let expansions = if graph.degrees[graph.root] >= BIG_INTEGERS_ONLY_DEGREE {
        None
    } else {
        Some(expansions(&graph, file, position)?)
    };
    let points = predicate.params.len();
    let big_arena = big_explicit_arena(&graph, points, file, position)?;
    let forms = Forms { filter, expansions };
    let mut out = String::new();
    write_header(
        &mut out,
        predicate,
        notice,
        &forms,
        graph.degrees[graph.root],
    );
    write_constants(&mut out, &forms);
    big::write_big_arena(&mut out, big_arena);
    let (uses_max, explicit) = match &forms.filter {
        FilterKind::Global(c) => (
            filter::write_filter(&mut out, &graph, points, c.degree),
            "0.0",
        ),
        FilterKind::PerAxis(c, zero) => (
            filter_axis::write_filter(&mut out, &graph, points, c, *zero),
            "[0.0; 3]",
        ),
    };
    write_exact_stages(&mut out, points, &forms);
    big::write_big_stage(&mut out, &graph, points);
    let explicit_stages = chain_stages(&forms);
    chain::write_chain(&mut out, points, &explicit_stages, explicit);
    let spans = forms
        .expansions
        .as_ref()
        .map(|_| "expansions_translated_in");
    generic::write_generic_chain(
        &mut out,
        &generic::GenericChain {
            points,
            per_axis: matches!(forms.filter, FilterKind::PerAxis(..)),
            explicit: &explicit_stages,
            spans,
        },
    );
    if uses_max {
        filter::write_max(&mut out);
    }
    // Only the stages on the stack check single coordinates.
    if forms.stack_stages().next().is_some() {
        write_in_range(&mut out);
    }
    if spans.is_some() {
        write_span_in_range(&mut out);
    }
    if forms
        .stack_stages()
        .flat_map(|(_, program)| &program.steps)
        .any(|step| matches!(step, Step::Product { .. }))
    {
        write_product(&mut out);
    }
    let mut checks = String::new();
    let warnings = checks::write_checks(
        &mut checks,
        predicate,
        &graph,
        &forms.filter,
        affine,
        notice,
    );
    Ok(Generated {
        name: predicate.name.clone(),
        stages: out,
        checks,
        warnings,
    })
}

/// The plans of the exact stages on expansions of the main form `graph`: a
/// stage on the stack is emitted only if it has a plan and its temporaries
/// fit, and a form too large for a plan on the stack is left to the stage
/// over spans, whose plan reports what no stage can do.
fn expansions<'a>(
    graph: &'a DiffGraph,
    file: &str,
    position: Position,
) -> Result<Expansions<'a>, Error> {
    let (min, max) = (PRODUCT_OPERAND_MIN, PRODUCT_OPERAND_MAX);
    let stack = |leaves| {
        let plan = exact_plan_differences(graph, leaves, file, position, min, max).ok()?;
        fits_on_stack(plan.total).then(|| {
            (
                Program::differences(graph, leaves, &plan.lengths),
                plan.range,
            )
        })
    };
    let differences = stack(Leaves::One);
    let translated = stack(Leaves::Two);
    let spans = exact_plan_spans_translated(graph, MAX_COORD_COMPONENTS, file, position, min, max)?;
    Ok(Expansions {
        differences,
        translated,
        spans_translated: (graph, spans.range),
    })
}

/// The notice that starts every generated file: where it comes from, that
/// it must not be edited, and how to regenerate it, followed by a blank
/// line.
pub(crate) fn notice(file: &str, regenerate: &str) -> String {
    let mut out = format!("// Generated by implicit-predicates-gen from {file}. Do not edit.\n");
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

fn write_header(out: &mut String, predicate: &Predicate, notice: &str, forms: &Forms, degree: u32) {
    out.push_str(notice);
    let only = if forms.expansions.is_none() {
        format!(
            " Its main form has degree {degree}: from degree {BIG_INTEGERS_ONLY_DEGREE} on, \
             [`big_integers_in`] is the only exact stage, after the filter."
        )
    } else {
        String::new()
    };
    comment(
        out,
        "//!",
        0,
        &format!(
            "The stages of the predicate `{}`.\n\n\
             Every stage takes the points {} of the definition as `p`, in that order, \
             and returns the sign of the predicate, or `None` when it cannot decide; \
             the last one, [`big_integers_in`], takes their coordinates as big integers \
             and always decides. [`staged`] runs the stages before it in order on explicit \
             points, and [`staged_in`] runs every stage on points of any type, so it \
             decides every input.{only}",
            predicate.name,
            list(&predicate.params)
        ),
    );
    // The names the module uses, in the order rustfmt sorts them.
    let spans = forms.expansions.is_some();
    let mut names = vec!["Big", "BigCoords"];
    if spans {
        names.push("ExactCoords");
    }
    names.extend(["ExactError", "Point", "Scratch", "ScratchExhausted", "Sign"]);
    if spans {
        names.extend(["Span", "Weight"]);
    }
    if forms
        .expansions
        .as_ref()
        .is_some_and(|e| e.differences.is_some())
    {
        names.push("eft");
    }
    if forms.stack_stages().next().is_some() {
        names.push("expansion");
    }
    writeln!(
        out,
        "\nuse implicit_predicates_core::{{{}}};\n",
        names.join(", ")
    )
    .unwrap();
}

/// The exact stages on expansions of the chain for explicit points, in
/// order: each stage on the stack, or the stage over spans in its place
/// when it is not emitted; none when the predicate has no stage on
/// expansions. The stage of exact differences has no counterpart over
/// spans.
fn chain_stages(forms: &Forms) -> Vec<ChainStage> {
    let Some(expansions) = &forms.expansions else {
        return Vec::new();
    };
    let mut stages = Vec::new();
    if expansions.differences.is_some() {
        stages.push(ChainStage::Stack("exact_differences"));
    }
    stages.push(match expansions.translated {
        Some(_) => ChainStage::Stack("expansions_translated"),
        None => ChainStage::Spans("expansions_translated_in"),
    });
    stages
}

/// `a`, `b` and `c`.
pub(crate) fn list(names: &[String]) -> String {
    let quoted: Vec<String> = names.iter().map(|n| format!("`{n}`")).collect();
    match quoted.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        _ => quoted.concat(),
    }
}
