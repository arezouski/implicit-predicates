//! The chain for points of any type, over the `Point` contract of
//! `implicit-predicates-core`.
//!
//! `staged_in` runs `filter_points`, the filter on the approximations of the
//! points with the uncertainty of their errors; then, when every point is
//! explicit, `exact_stages_explicit_in`, the exact stages of the chain for
//! explicit points and then the stage on big integers; otherwise
//! `exact_stages_in`, the stages over spans on the exact coordinates of the
//! points, the translated form first, and then the stage on big integers.
//! The stage on big integers always decides, so the chain decides every
//! input; a point without exact coordinates as big integers, which the
//! contract allows only for inputs that are not finite, or with a weight
//! other than one, is a panic. Whether every
//! point is explicit is [`Point::is_explicit`] of each: for the types where
//! it is the constant `true`, such as `[f64; 3]`, the test is resolved at
//! compile time, so the errors are not read and the stages over spans are
//! not called. `filter_points` is `#[inline]` rather than
//! `#[inline(always)]`: on four `[f64; 3]` the latter compiled to a slower
//! filter, with more registers saved on entry.
//!
//! [`Point::is_explicit`]: implicit_predicates_core::Point::is_explicit

use std::fmt::Write;

use super::big::BIG_STAGE;
use super::chain::ChainStage;
use super::comment;

/// The message of the panic for a point whose exact coordinates have a
/// weight other than one.
pub(crate) const WEIGHT_PANIC: &str =
    "implicit-predicates: a point has a weight other than one, which the predicates do not support";

/// The message of the panic for a point without exact coordinates as big
/// integers.
pub(crate) const NOT_FINITE_PANIC: &str =
    "implicit-predicates: a point has no exact coordinates: an input is not finite";

/// The pieces of text that the functions over points of any type share,
/// for `points` points.
struct Signature {
    /// `<P0: Point, P1: Point>`.
    generics: String,
    /// `p0: &P0, p1: &P1`.
    params: String,
    /// `p0, p1`.
    args: String,
    /// `[p0.approx(), p1.approx()]`.
    approx: String,
    /// `[p0.err(), p1.err()]`.
    errs: String,
    /// `p0.is_explicit() & p1.is_explicit()`.
    explicit: String,
}

impl Signature {
    fn new(points: usize) -> Signature {
        let each = |f: &dyn Fn(usize) -> String, sep: &str| {
            (0..points).map(f).collect::<Vec<_>>().join(sep)
        };
        Signature {
            generics: format!("<{}>", each(&|i| format!("P{i}: Point"), ", ")),
            params: each(&|i| format!("p{i}: &P{i}"), ", "),
            args: each(&|i| format!("p{i}"), ", "),
            approx: format!("[{}]", each(&|i| format!("p{i}.approx()"), ", ")),
            errs: format!("[{}]", each(&|i| format!("p{i}.err()"), ", ")),
            explicit: each(&|i| format!("p{i}.is_explicit()"), " & "),
        }
    }
}

/// How the chain for points of any type is emitted.
pub(crate) struct GenericChain<'a> {
    /// The number of points.
    pub(crate) points: usize,
    /// Whether the filter has one uncertainty per axis, rather than one for
    /// every axis.
    pub(crate) per_axis: bool,
    /// The exact stages of the chain for explicit points, in order.
    pub(crate) explicit: &'a [ChainStage],
    /// The stages over spans, in the order the chain tries them.
    pub(crate) spans: &'a [&'static str],
}

/// Writes `filter_points`, `exact_stages`, `exact_stages_explicit_in`,
/// `exact_stages_in`, `staged_in` and their helpers `exact`, `exact_big`
/// and `no_exact_coordinates`.
pub(crate) fn write_generic_chain(out: &mut String, chain: &GenericChain) {
    let s = Signature::new(chain.points);
    let (zero, uncertainty) = if chain.per_axis {
        ("[0.0; 3]", "uncertainty")
    } else {
        ("0.0", "uncertainty_max")
    };
    write_filter_points(out, &s, zero, uncertainty);
    write_exact_stages(out, chain);
    write_exact_stages_explicit_in(out, chain);
    write_exact_stages_in(out, &s, chain.spans);
    comment(
        out,
        "///",
        0,
        &format!(
            "The sign of the predicate for points of any type, from the first stage that \
             decides.\n\n\
             Runs [`filter_points`]; then, when every point is explicit, \
             [`exact_stages_explicit_in`] on their coordinates; otherwise \
             [`exact_stages_in`], on their exact coordinates written to `scratch`. \
             `scratch` is not used when the filter decides, and is left as it was. Every \
             call decides: the last stage, [`{BIG_STAGE}`], always does.\n\n\
             # Errors\n\n\
             [`ScratchExhausted`] if `scratch` runs out of room.\n\n\
             # Panics\n\n\
             If an input of a point is not finite, or the exact coordinates of a point \
             have a weight other than one, as [`exact_stages_in`] says."
        ),
    );
    writeln!(
        out,
        "#[inline]\npub fn staged_in{}(scratch: &mut Scratch<'_>, {}) \
         -> Result<Sign, ScratchExhausted> {{\n\
         if let Some(sign) = filter_points({}) {{\nreturn Ok(sign);\n}}\n\
         if {} {{\nreturn exact_stages_explicit_in(scratch, {});\n}}\n\
         exact_stages_in(scratch, {})\n}}\n",
        s.generics, s.params, s.args, s.explicit, s.approx, s.args
    )
    .unwrap();
    write_helpers(out);
}

/// Writes the helpers of the chain: `exact`, `exact_big` and
/// `no_exact_coordinates`.
fn write_helpers(out: &mut String) {
    comment(
        out,
        "///",
        0,
        "The exact coordinates of the point `p`, written to `scratch`, or `None` if \
         it cannot give them for any reason other than the room of the arena, such as \
         inputs outside the range of its exact arithmetic.\n\n\
         # Panics\n\n\
         If the coordinates have a weight other than [`Weight::One`], which the \
         predicates do not support.",
    );
    writeln!(
        out,
        "fn exact(scratch: &mut Scratch<'_>, p: &impl Point) \
         -> Result<Option<ExactCoords>, ScratchExhausted> {{\n\
         match p.exact(scratch) {{\n\
         Ok(c) if c.w != Weight::One => no_exact_coordinates(\n\"{WEIGHT_PANIC}\",\n),\n\
         Ok(c) => Ok(Some(c)),\n\
         Err(ExactError::Exhausted(e)) => Err(e),\n\
         Err(_) => Ok(None),\n\
         }}\n\
         }}\n"
    )
    .unwrap();
    comment(
        out,
        "///",
        0,
        "The exact coordinates of the point `p` as big integers, written to `scratch`.\n\n\
         # Panics\n\n\
         If `p` cannot give them for any reason other than the room of the arena: by \
         the contract of [`Point::exact_big`], only when an input is not finite or \
         the coordinates have a weight other than one.",
    );
    writeln!(
        out,
        "fn exact_big(scratch: &mut Scratch<'_>, p: &impl Point) \
         -> Result<BigCoords, ScratchExhausted> {{\n\
         match p.exact_big(scratch) {{\n\
         Ok(c) => Ok(c),\n\
         Err(ExactError::Exhausted(e)) => Err(e),\n\
         Err(ExactError::Weighted) => no_exact_coordinates(\n\"{WEIGHT_PANIC}\",\n),\n\
         Err(_) => no_exact_coordinates(\n\"{NOT_FINITE_PANIC}\",\n),\n\
         }}\n\
         }}\n"
    )
    .unwrap();
    comment(
        out,
        "///",
        0,
        "Ends a call whose points cannot give the exact coordinates the predicate \
         needs, with a panic of message `reason`: the chain never returns a sign it \
         has not computed exactly.",
    );
    out.push_str(
        "#[cold]\n#[inline(never)]\n\
         fn no_exact_coordinates(reason: &str) -> ! {\n\
         panic!(\"{reason}\")\n\
         }\n\n",
    );
}

/// Writes `filter_points`.
fn write_filter_points(out: &mut String, s: &Signature, zero: &str, uncertainty: &str) {
    comment(
        out,
        "///",
        0,
        &format!(
            "[`filter`] on the approximations of points of any type, with the \
             uncertainty of their errors: `{zero}` when every point is explicit, and \
             otherwise `{uncertainty}` of their errors, from \
             `implicit_predicates_core::filter`.\n\n\
             The errors are not read when every point is explicit. When that is known \
             from the types, as with `[f64; 3]`, whose `is_explicit` is the constant \
             `true`, this is `filter` with `e = {zero}`."
        ),
    );
    writeln!(
        out,
        "#[inline]\npub fn filter_points{}({}) -> Option<Sign> {{\n\
         let p = {};\n\
         let e = if {} {{\n{zero}\n}} else {{\n\
         implicit_predicates_core::filter::{uncertainty}({})\n}};\n\
         filter(p, e)\n}}\n",
        s.generics, s.params, s.approx, s.explicit, s.errs
    )
    .unwrap();
}

/// Writes `exact_stages`: the exact stages of the chain for explicit
/// points.
fn write_exact_stages(out: &mut String, chain: &GenericChain) {
    let names: Vec<String> = chain
        .explicit
        .iter()
        .map(|stage| match stage {
            ChainStage::Stack(name) | ChainStage::Spans(name) => format!("[`{name}`]"),
        })
        .collect();
    let points = chain.points;
    let first_spans = chain
        .explicit
        .iter()
        .position(|stage| matches!(stage, ChainStage::Spans(_)));
    let Some(first_spans) = first_spans else {
        comment(
            out,
            "///",
            0,
            &format!(
                "The exact stages of [`staged`], after its filter: {}. `None` means that \
                 no stage can decide.",
                names.join(", then ")
            ),
        );
        write!(
            out,
            "#[cold]\n#[inline(never)]\n\
             pub fn exact_stages(p: [&[f64; 3]; {points}]) -> Option<Sign> {{\n"
        )
        .unwrap();
        for (i, stage) in chain.explicit.iter().enumerate() {
            if let ChainStage::Stack(name) = stage {
                if i == 0 {
                    write!(out, "{name}(p)").unwrap();
                } else {
                    write!(out, "\n.or_else(|| {name}(p))").unwrap();
                }
            }
        }
        out.push_str("\n}\n\n");
        return;
    };
    comment(
        out,
        "///",
        0,
        &format!(
            "The exact stages of [`staged`], after its filter: {}. The stages that take \
             an arena run on the exact coordinates of the points written to `scratch`, \
             which is left as it was. `Ok(None)` means that no stage can decide.\n\n\
             # Errors\n\n\
             [`ScratchExhausted`] if `scratch` runs out of room.",
            names.join(", then ")
        ),
    );
    writeln!(
        out,
        "#[cold]\n#[inline(never)]\n\
         pub fn exact_stages(scratch: &mut Scratch<'_>, p: [&[f64; 3]; {points}]) \
         -> Result<Option<Sign>, ScratchExhausted> {{"
    )
    .unwrap();
    for stage in &chain.explicit[..first_spans] {
        if let ChainStage::Stack(name) = stage {
            writeln!(
                out,
                "if let Some(sign) = {name}(p) {{\nreturn Ok(Some(sign));\n}}"
            )
            .unwrap();
        }
    }
    out.push_str("staged_in_arena(scratch, p)\n}\n\n");
}

/// Writes `exact_stages_explicit_in`: `exact_stages`, then the stage on big
/// integers on the coordinates of the explicit points.
fn write_exact_stages_explicit_in(out: &mut String, chain: &GenericChain) {
    let points = chain.points;
    let in_arena = chain
        .explicit
        .iter()
        .any(|stage| matches!(stage, ChainStage::Spans(_)));
    let (stages_call, stages_doc) = if in_arena {
        (
            "exact_stages(scratch, p)?",
            "[`exact_stages`], with `scratch`",
        )
    } else {
        (
            "exact_stages(p)",
            "[`exact_stages`], which does not use `scratch`",
        )
    };
    comment(
        out,
        "///",
        0,
        &format!(
            "The exact sign of the predicate for explicit points: {stages_doc}, then, \
             when it does not decide, [`{BIG_STAGE}`] on the coordinates of the points \
             written to `scratch` as big integers, which always decides. `scratch` is \
             left as it was.\n\n\
             # Errors\n\n\
             [`ScratchExhausted`] if `scratch` runs out of room.\n\n\
             # Panics\n\n\
             If a coordinate is not finite."
        ),
    );
    let signature = format!(
        "(scratch: &mut Scratch<'_>, p: [&[f64; 3]; {points}]) -> Result<Sign, ScratchExhausted>"
    );
    writeln!(
        out,
        "#[cold]\n#[inline(never)]\npub fn exact_stages_explicit_in{signature} {{\n\
         fn big{signature} {{"
    )
    .unwrap();
    for i in 0..points {
        writeln!(out, "let q{i} = exact_big(scratch, p[{i}])?;").unwrap();
    }
    let refs: Vec<String> = (0..points).map(|i| format!("&q{i}")).collect();
    writeln!(
        out,
        "{BIG_STAGE}(scratch, [{}])\n}}\n\
         if let Some(sign) = {stages_call} {{\nreturn Ok(sign);\n}}\n\
         let mark = scratch.mark();\n\
         let result = big(scratch, p);\n\
         scratch.reset(mark);\n\
         result\n}}\n",
        refs.join(", ")
    )
    .unwrap();
}

/// Writes `exact_stages_in`: the stages over spans, in the order of
/// `spans`, on the exact coordinates of points of any type, then the stage
/// on big integers on their coordinates as big integers.
fn write_exact_stages_in(out: &mut String, s: &Signature, spans: &[&str]) {
    let names: Vec<String> = spans.iter().map(|name| format!("[`{name}`]")).collect();
    comment(
        out,
        "///",
        0,
        &format!(
            "The exact sign of the predicate for points of any type: {}, on the exact \
             coordinates of the points written to `scratch`, then, when they do not \
             decide, [`{BIG_STAGE}`] on the coordinates of the points as big integers \
             written to `scratch`, which always decides. The stages over spans are \
             skipped when a point cannot give its exact coordinates as expansions, as when \
             its inputs are outside the range of its exact arithmetic \
             ([`ExactError::OutOfRange`]). Each attempt starts from the mark \
             of `scratch` on entry, to which it is reset whatever they return.\n\n\
             # Errors\n\n\
             [`ScratchExhausted`] if `scratch` runs out of room.\n\n\
             # Panics\n\n\
             If the exact coordinates of a point have a weight other than \
             [`Weight::One`], which the predicates do not support, or a point cannot give \
             its exact coordinates as big integers, which by the contract of \
             [`Point::exact_big`] happens only when an input is not finite or the coordinates \
             have a weight other than one.",
            names.join(", then ")
        ),
    );
    let signature = |result: &str| {
        format!(
            "{}(scratch: &mut Scratch<'_>, {}) -> Result<{result}, ScratchExhausted>",
            s.generics, s.params
        )
    };
    writeln!(
        out,
        "#[cold]\n#[inline(never)]\npub fn exact_stages_in{} {{\nfn spans{} {{",
        signature("Sign"),
        signature("Option<Sign>")
    )
    .unwrap();
    let points: Vec<&str> = s.args.split(", ").collect();
    for (i, p) in points.iter().enumerate() {
        writeln!(out, "let q{i} = exact(scratch, {p})?;").unwrap();
    }
    let some: Vec<String> = (0..points.len()).map(|i| format!("Some(q{i})")).collect();
    let q: Vec<String> = (0..points.len()).map(|i| format!("q{i}")).collect();
    let refs: Vec<String> = (0..points.len()).map(|i| format!("&q{i}")).collect();
    writeln!(
        out,
        "let ({}) = ({}) else {{\nreturn Ok(None);\n}};\nlet q = [{}];",
        some.join(", "),
        q.join(", "),
        refs.join(", ")
    )
    .unwrap();
    if let Some((last, rest)) = spans.split_last() {
        for name in rest {
            writeln!(
                out,
                "if let Some(sign) = {name}(scratch, q)? {{\nreturn Ok(Some(sign));\n}}"
            )
            .unwrap();
        }
        writeln!(out, "{last}(scratch, q)").unwrap();
    } else {
        out.push_str("let _ = q;\nOk(None)\n");
    }
    writeln!(out, "}}\nfn big{} {{", signature("Sign")).unwrap();
    for (i, p) in points.iter().enumerate() {
        writeln!(out, "let q{i} = exact_big(scratch, {p})?;").unwrap();
    }
    writeln!(
        out,
        "{BIG_STAGE}(scratch, [{}])\n}}\n\
         let mark = scratch.mark();\n\
         let decided = spans(scratch, {args});\n\
         scratch.reset(mark);\n\
         if let Some(sign) = decided? {{\nreturn Ok(sign);\n}}\n\
         let result = big(scratch, {args});\n\
         scratch.reset(mark);\n\
         result\n}}\n",
        refs.join(", "),
        args = s.args
    )
    .unwrap();
}
