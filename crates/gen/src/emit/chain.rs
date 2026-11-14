//! The function that runs the stages of a predicate in order.
//!
//! The exact stages are tried in the order: differences that are exact, then
//! the translated expansion form. The order is the same for every predicate
//! that has stages on expansions. A stage whose temporaries do not fit on
//! the stack is replaced by the stage over spans of the same form, run on
//! the exact coordinates of the explicit points written to an arena;
//! `staged` then takes the arena. A predicate whose form is not
//! multihomogeneous has a second test of its filter, `filter_axes`, run right
//! after `filter` and before every exact stage. A predicate without stages on
//! expansions has its filter alone in `staged`, with its second test if it
//! has one.

use std::fmt::Write;

use super::comment;

/// An exact stage of the chain for explicit points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChainStage {
    /// A stage on the stack, by name, which takes `p`.
    Stack(&'static str),
    /// A stage over spans, by name, which takes the arena and the exact
    /// coordinates of `p`.
    Spans(&'static str),
}

/// The text that the documentation of a chain gives to `filter_axes`, run
/// with no uncertainty, after the filter, when `axes` says it is emitted.
pub(crate) fn axes_doc(axes: bool) -> &'static str {
    if axes {
        ", then [`filter_axes`] with `e = [0.0; 3]`"
    } else {
        ""
    }
}

/// Writes `staged`: the filter for explicit points, called with the
/// uncertainty `explicit` (`0.0`, or `[0.0; 3]` for a per-axis filter), then
/// `filter_axes` with no uncertainty when `axes` says it is emitted, then the
/// exact `stages` in order, if there are any.
pub(crate) fn write_chain(
    out: &mut String,
    points: usize,
    stages: &[ChainStage],
    explicit: &str,
    axes: bool,
) {
    let names: Vec<String> = stages
        .iter()
        .map(|stage| match stage {
            ChainStage::Stack(name) | ChainStage::Spans(name) => format!("[`{name}`]"),
        })
        .collect();
    // The stage over spans, if any, takes the place of the last stage on the
    // stack.
    let Some((ChainStage::Spans(spans), before)) = stages.split_last() else {
        write_stack_chain(out, points, &names, stages, explicit, axes);
        return;
    };
    let axes_text = axes_doc(axes);
    comment(
        out,
        "///",
        0,
        &format!(
            "The sign of the predicate for explicit points, from the first stage that \
             decides.\n\n\
             Runs [`filter`] with `e = {explicit}`{axes_text}, then {}. The stage that takes an arena \
             runs on the exact coordinates of the points written to `scratch`, which is \
             left as it was. `Ok(None)` means that no stage can decide.\n\n\
             # Errors\n\n\
             [`ScratchExhausted`] if `scratch` runs out of room.",
            names.join(", then ")
        ),
    );
    let signature = format!(
        "(scratch: &mut Scratch<'_>, p: [&[f64; 3]; {points}]) \
         -> Result<Option<Sign>, ScratchExhausted>"
    );
    writeln!(
        out,
        "#[inline]\npub fn staged{signature} {{\n\
         if let Some(sign) = filter(p, {explicit}) {{\nreturn Ok(Some(sign));\n}}"
    )
    .unwrap();
    if axes {
        out.push_str("if let Some(sign) = filter_axes(p, [0.0; 3]) {\nreturn Ok(Some(sign));\n}\n");
    }
    for stage in before {
        if let ChainStage::Stack(name) = stage {
            writeln!(
                out,
                "if let Some(sign) = {name}(p) {{\nreturn Ok(Some(sign));\n}}"
            )
            .unwrap();
        }
    }
    writeln!(out, "staged_in_arena(scratch, p)\n}}\n").unwrap();
    // The stage over spans, on the exact coordinates.
    let q: Vec<String> = (0..points).map(|i| format!("Some(q{i})")).collect();
    let exact: Vec<String> = (0..points)
        .map(|i| format!("exact(scratch, p[{i}])?"))
        .collect();
    let refs: Vec<String> = (0..points).map(|i| format!("&q{i}")).collect();
    comment(
        out,
        "///",
        0,
        "The stage of [`staged`] that takes an arena, on the exact coordinates of the \
         points written to `scratch`, which is reset to its mark whatever it returns.",
    );
    writeln!(
        out,
        "#[cold]\n#[inline(never)]\nfn staged_in_arena{signature} {{\n\
         fn run{signature} {{\n\
         let ({},) = ({},) else {{\nreturn Ok(None);\n}};\n\
         let q = [{}];\n\
         {spans}(scratch, q)\n}}",
        q.join(", "),
        exact.join(", "),
        refs.join(", ")
    )
    .unwrap();
    writeln!(
        out,
        "let mark = scratch.mark();\n\
         let result = run(scratch, p);\n\
         scratch.reset(mark);\n\
         result\n}}\n"
    )
    .unwrap();
}

/// Writes `staged` when every stage is on the stack, or there is none after
/// the filter.
fn write_stack_chain(
    out: &mut String,
    points: usize,
    names: &[String],
    stages: &[ChainStage],
    explicit: &str,
    axes: bool,
) {
    let axes_text = axes_doc(axes);
    let doc = if names.is_empty() && axes {
        format!(
            "The sign of the predicate for explicit points, from the two tests of its \
             filter, the only stages before [`big_integers_in`].\n\n\
             Runs [`filter`] with `e = {explicit}`{axes_text}. `None` means that neither \
             can decide; [`exact_stages_explicit_in`] then decides."
        )
    } else if names.is_empty() {
        format!(
            "The sign of the predicate for explicit points, from its filter, the only \
             stage before [`big_integers_in`].\n\n\
             Runs [`filter`] with `e = {explicit}`. `None` means that it cannot decide; \
             [`exact_stages_explicit_in`] then decides."
        )
    } else {
        format!(
            "The sign of the predicate for explicit points, from the first stage that \
             decides.\n\n\
             Runs [`filter`] with `e = {explicit}`{axes_text}, then {}. `None` means that \
             no stage can decide.",
            names.join(", then ")
        )
    };
    comment(out, "///", 0, &doc);
    write!(
        out,
        "#[inline]\n\
         pub fn staged(p: [&[f64; 3]; {points}]) -> Option<Sign> {{\n\
         filter(p, {explicit})"
    )
    .unwrap();
    if axes {
        out.push_str("\n.or_else(|| filter_axes(p, [0.0; 3]))");
    }
    for stage in stages {
        if let ChainStage::Stack(name) = stage {
            write!(out, "\n.or_else(|| {name}(p))").unwrap();
        }
    }
    out.push_str("\n}\n\n");
}
