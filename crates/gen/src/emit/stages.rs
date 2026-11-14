//! The constants and the exact stages of a predicate: the stages on the
//! stack that fit within [`STACK_LIMIT`], and the stages over arena spans.

use std::fmt::Write;

use super::exact::{Check, Stage, write_stage};
use super::filter::{self, FilterKind};
use super::filter_axis;
use super::program::Program;
use super::spans::{SpanStage, write_span_stage};
use super::{comment, literal, power_of_two};
use crate::differences::DiffGraph;
use crate::range::{ExactRange, MAX_COORD_COMPONENTS};

/// The most `f64` of temporaries, [`crate::ExactPlan::total`], that an
/// exact stage may take to be emitted on the stack. A stage that needs more
/// is not emitted, and the chain for explicit points uses the stage over
/// spans in its place.
pub(crate) const STACK_LIMIT: usize = 4096;

/// Whether a stage whose temporaries take `total` `f64` is emitted on the
/// stack: at most [`STACK_LIMIT`].
pub(crate) fn fits_on_stack(total: usize) -> bool {
    total <= STACK_LIMIT
}

/// The forms of a predicate, with the plans of their exact stages. A stage
/// on the stack is `None` when its temporaries exceed [`STACK_LIMIT`].
pub(crate) struct Forms<'a> {
    pub(crate) filter: FilterKind,
    pub(crate) differences: Option<(Program, ExactRange)>,
    pub(crate) translated: Option<(Program, ExactRange)>,
    /// The main form, and the range of its stage over spans.
    pub(crate) spans_translated: (&'a DiffGraph, ExactRange),
}

impl Forms<'_> {
    /// The stages on the stack that are emitted, with their names.
    pub(crate) fn stack_stages(&self) -> impl Iterator<Item = (&'static str, &Program)> {
        [
            ("exact_differences", &self.differences),
            ("expansions_translated", &self.translated),
        ]
        .into_iter()
        .filter_map(|(name, stage)| stage.as_ref().map(|(program, _)| (name, program)))
    }
}

pub(crate) fn write_constants(out: &mut String, forms: &Forms) {
    let constant = |out: &mut String, doc: &str, name: &str, ty: &str, value: String| {
        comment(out, "///", 0, doc);
        writeln!(out, "pub const {name}: {ty} = {value};\n").unwrap();
    };
    match &forms.filter {
        FilterKind::Global(c) => filter::write_constants(out, c),
        FilterKind::PerAxis(c, _) => filter_axis::write_constants(out, c),
    }
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
    if let Some((_, r)) = &forms.differences {
        range(
            out,
            "DIFFERENCES_RANGE",
            "[`exact_differences`] decides when every difference of coordinates is exact \
             in `f64` and every nonzero one",
            *r,
        );
    }
    if let Some((_, r)) = &forms.translated {
        range(
            out,
            "TRANSLATED_RANGE",
            "[`expansions_translated`] decides when every nonzero coordinate",
            *r,
        );
    }
    constant(
        out,
        "The most components of a coordinate that [`expansions_translated_in`] accepts.",
        "MAX_COORD_COMPONENTS",
        "usize",
        MAX_COORD_COMPONENTS.to_string(),
    );
    range(
        out,
        "SPANS_TRANSLATED_RANGE",
        "[`expansions_translated_in`] decides when every nonzero component of every coordinate",
        forms.spans_translated.1,
    );
}

pub(crate) fn write_exact_stages(out: &mut String, points: usize, forms: &Forms) {
    if let Some((program, _)) = &forms.differences {
        let doc = "The exact sign of the predicate: its main form evaluated with expansions \
             on the differences of coordinates, each computed in `f64`.\n\n\
             `None` means that this stage cannot decide: some difference is not exact in \
             `f64`, or some nonzero difference has a magnitude outside [`DIFFERENCES_RANGE`], \
             as when a coordinate is not finite.";
        let check = Check::Differences("DIFFERENCES_RANGE");
        let name = "exact_differences";
        write_stage(
            out,
            &Stage {
                name,
                doc,
                program,
                check,
            },
            points,
        );
    }
    if let Some((program, _)) = &forms.translated {
        let doc = "The exact sign of the predicate: its main form evaluated with expansions, \
             each difference of coordinates as an exact expansion of two components.\n\n\
             `None` means that this stage cannot decide: some nonzero coordinate has a \
             magnitude outside [`TRANSLATED_RANGE`] or is not finite.";
        let check = Check::Coordinates("TRANSLATED_RANGE");
        let name = "expansions_translated";
        write_stage(
            out,
            &Stage {
                name,
                doc,
                program,
                check,
            },
            points,
        );
    }
    let doc = "The exact sign of the predicate for points given by their exact coordinates: \
         its main form evaluated with expansions in `scratch`, each difference of \
         coordinates computed exactly.\n\n\
         `Ok(None)` means that this stage cannot decide: some point has a weight other \
         than [`Weight::One`], or some coordinate has more than \
         [`MAX_COORD_COMPONENTS`] components or a nonzero component of magnitude \
         outside [`SPANS_TRANSLATED_RANGE`] or not finite.\n\n\
         Requirement: every coordinate is a valid expansion of `scratch`: nonzero \
         components of strictly increasing magnitude that do not overlap, as \
         [`Point::exact`](implicit_predicates_core::Point::exact) writes them. This is \
         not checked, and the result is unspecified otherwise.\n\n\
         The arena is left as it was, on success and on error.\n\n\
         # Errors\n\n\
         [`ScratchExhausted`] if `scratch` runs out of room.";
    let stage = SpanStage {
        name: "expansions_translated_in",
        doc,
        graph: forms.spans_translated.0,
        range: "SPANS_TRANSLATED_RANGE",
    };
    write_span_stage(out, &stage, points);
}
