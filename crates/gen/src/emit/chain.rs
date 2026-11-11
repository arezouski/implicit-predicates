//! The function that runs the stages of a predicate in order.
//!
//! The exact stages are tried in the order: differences that are exact, the
//! translated expansion form, then the `#[exact]` form if one is given. The
//! order is the same for every predicate for now. A stage whose temporaries
//! do not fit on the stack is replaced by the stage over spans of the same
//! form, run on the exact coordinates of the explicit points written to an
//! arena; `staged` then takes the arena.

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

/// Writes `staged`: the filter for explicit points, called with the
/// uncertainty `explicit` (`0.0`, or `[0.0; 3]` for a per-axis filter), then
/// the exact `stages` in order.
pub(crate) fn write_chain(out: &mut String, points: usize, stages: &[ChainStage], explicit: &str) {
    let names: Vec<String> = stages
        .iter()
        .map(|stage| match stage {
            ChainStage::Stack(name) | ChainStage::Spans(name) => format!("[`{name}`]"),
        })
        .collect();
    let first_spans = stages
        .iter()
        .position(|stage| matches!(stage, ChainStage::Spans(_)));
    let Some(first_spans) = first_spans else {
        write_stack_chain(out, points, &names, stages, explicit);
        return;
    };
    comment(
        out,
        "///",
        0,
        &format!(
            "The sign of the predicate for explicit points, from the first stage that \
             decides.\n\n\
             Runs [`filter`] with `e = {explicit}`, then {}. The stages that take an arena \
             run on the exact coordinates of the points written to `scratch`, which is \
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
    for stage in &stages[..first_spans] {
        if let ChainStage::Stack(name) = stage {
            writeln!(
                out,
                "if let Some(sign) = {name}(p) {{\nreturn Ok(Some(sign));\n}}"
            )
            .unwrap();
        }
    }
    writeln!(out, "staged_in_arena(scratch, p)\n}}\n").unwrap();
    // The stages from the first one over spans, on the exact coordinates.
    let q: Vec<String> = (0..points).map(|i| format!("Some(q{i})")).collect();
    let exact: Vec<String> = (0..points)
        .map(|i| format!("exact(scratch, p[{i}])?"))
        .collect();
    let refs: Vec<String> = (0..points).map(|i| format!("&q{i}")).collect();
    comment(
        out,
        "///",
        0,
        "The stages of [`staged`] from the first one that takes an arena, on the exact \
         coordinates of the points written to `scratch`, which is reset to its mark \
         whatever they return.",
    );
    writeln!(
        out,
        "#[cold]\n#[inline(never)]\nfn staged_in_arena{signature} {{\n\
         fn run{signature} {{\n\
         let ({},) = ({},) else {{\nreturn Ok(None);\n}};\n\
         let q = [{}];",
        q.join(", "),
        exact.join(", "),
        refs.join(", ")
    )
    .unwrap();
    for stage in &stages[first_spans..] {
        match stage {
            ChainStage::Stack(name) => {
                writeln!(
                    out,
                    "if let Some(sign) = {name}(p) {{\nreturn Ok(Some(sign));\n}}"
                )
            }
            ChainStage::Spans(name) => writeln!(
                out,
                "if let Some(sign) = {name}(scratch, q)? {{\nreturn Ok(Some(sign));\n}}"
            ),
        }
        .unwrap();
    }
    writeln!(
        out,
        "Ok(None)\n}}\n\
         let mark = scratch.mark();\n\
         let result = run(scratch, p);\n\
         scratch.reset(mark);\n\
         result\n}}\n"
    )
    .unwrap();
}

/// Writes `staged` when every stage is on the stack.
fn write_stack_chain(
    out: &mut String,
    points: usize,
    names: &[String],
    stages: &[ChainStage],
    explicit: &str,
) {
    comment(
        out,
        "///",
        0,
        &format!(
            "The sign of the predicate for explicit points, from the first stage that \
             decides.\n\n\
             Runs [`filter`] with `e = {explicit}`, then {}. `None` means that no stage \
             can decide.",
            names.join(", then ")
        ),
    );
    write!(
        out,
        "#[inline]\n\
         pub fn staged(p: [&[f64; 3]; {points}]) -> Option<Sign> {{\n\
         filter(p, {explicit})"
    )
    .unwrap();
    for stage in stages {
        if let ChainStage::Stack(name) = stage {
            write!(out, "\n.or_else(|| {name}(p))").unwrap();
        }
    }
    out.push_str("\n}\n\n");
}
