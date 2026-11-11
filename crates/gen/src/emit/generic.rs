//! The chain for points of any type, over the `Point` contract of
//! `implicit-predicates-core`.
//!
//! `staged_in` runs `filter_points`, the filter on the approximations of the
//! points with the uncertainty of their errors; then, when every point is
//! explicit, `exact_stages`, the exact stages of the chain for explicit
//! points; otherwise `exact_stages_in`, the stages over spans on the exact
//! coordinates of the points, the translated form first. Whether every
//! point is explicit is [`Point::is_explicit`] of each: for the types where
//! it is the constant `true`, such as `[f64; 3]`, the test is resolved at
//! compile time, so the errors are not read and the stages over spans are
//! not called. `filter_points` is `#[inline]` rather than
//! `#[inline(always)]`: on four `[f64; 3]` the latter compiled to a slower
//! filter, with more registers saved on entry.
//!
//! [`Point::is_explicit`]: implicit_predicates_core::Point::is_explicit

use std::fmt::Write;

use super::chain::ChainStage;
use super::comment;

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

/// Writes `filter_points`, `exact_stages`, `exact_stages_in`, `staged_in`
/// and their helper `exact`.
pub(crate) fn write_generic_chain(out: &mut String, chain: &GenericChain) {
    let s = Signature::new(chain.points);
    let (zero, uncertainty) = if chain.per_axis {
        ("[0.0; 3]", "uncertainty")
    } else {
        ("0.0", "uncertainty_max")
    };
    let in_arena = chain
        .explicit
        .iter()
        .any(|stage| matches!(stage, ChainStage::Spans(_)));
    write_filter_points(out, &s, zero, uncertainty);
    write_exact_stages(out, chain);
    write_exact_stages_in(out, &s, chain.spans);
    let explicit_call = if in_arena {
        format!("return exact_stages(scratch, {});", s.approx)
    } else {
        format!("return Ok(exact_stages({}));", s.approx)
    };
    let explicit_doc = if in_arena {
        "[`exact_stages`] on their coordinates, with `scratch`"
    } else {
        "[`exact_stages`] on their coordinates, which does not use `scratch`"
    };
    comment(
        out,
        "///",
        0,
        &format!(
            "The sign of the predicate for points of any type, from the first stage that \
             decides.\n\n\
             Runs [`filter_points`]; then, when every point is explicit, {explicit_doc}; \
             otherwise [`exact_stages_in`], on their exact coordinates written to \
             `scratch`. `scratch` is left as it was. `Ok(None)` means that no stage can \
             decide.\n\n\
             # Errors\n\n\
             [`ScratchExhausted`] if `scratch` runs out of room."
        ),
    );
    writeln!(
        out,
        "#[inline]\npub fn staged_in{}(scratch: &mut Scratch<'_>, {}) \
         -> Result<Option<Sign>, ScratchExhausted> {{\n\
         if let Some(sign) = filter_points({}) {{\nreturn Ok(Some(sign));\n}}\n\
         if {} {{\n{explicit_call}\n}}\n\
         exact_stages_in(scratch, {})\n}}\n",
        s.generics, s.params, s.args, s.explicit, s.args
    )
    .unwrap();
    comment(
        out,
        "///",
        0,
        "The exact coordinates of the point `p`, written to `scratch`, or `None` if \
         its inputs are outside the range of its exact arithmetic.",
    );
    out.push_str(
        "fn exact(scratch: &mut Scratch<'_>, p: &impl Point) \
         -> Result<Option<ExactCoords>, ScratchExhausted> {\n\
         match p.exact(scratch) {\n\
         Ok(c) => Ok(Some(c)),\n\
         Err(ExactError::OutOfRange) => Ok(None),\n\
         Err(ExactError::Exhausted(e)) => Err(e),\n\
         }\n\
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

/// Writes `exact_stages_in`: the stages over spans, in the order of
/// `spans`, on the exact coordinates of points of any type.
fn write_exact_stages_in(out: &mut String, s: &Signature, spans: &[&str]) {
    let names: Vec<String> = spans.iter().map(|name| format!("[`{name}`]")).collect();
    comment(
        out,
        "///",
        0,
        &format!(
            "The exact sign of the predicate for points of any type: {}, on the exact \
             coordinates of the points written to `scratch`, which is reset to its mark \
             whatever they return. `Ok(None)` means that no stage can decide: the inputs \
             of some point are outside the range of its exact arithmetic \
             ([`ExactError::OutOfRange`]), or none of the stages decides.\n\n\
             # Errors\n\n\
             [`ScratchExhausted`] if `scratch` runs out of room.",
            names.join(", then ")
        ),
    );
    let signature = format!(
        "{}(scratch: &mut Scratch<'_>, {}) -> Result<Option<Sign>, ScratchExhausted>",
        s.generics, s.params
    );
    writeln!(
        out,
        "#[cold]\n#[inline(never)]\npub fn exact_stages_in{signature} {{\nfn run{signature} {{"
    )
    .unwrap();
    let points: Vec<&str> = s.args.split(", ").collect();
    for (i, p) in points.iter().enumerate() {
        writeln!(
            out,
            "let Some(q{i}) = exact(scratch, {p})? else {{\nreturn Ok(None);\n}};"
        )
        .unwrap();
    }
    let refs: Vec<String> = (0..points.len()).map(|i| format!("&q{i}")).collect();
    writeln!(out, "let q = [{}];", refs.join(", ")).unwrap();
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
    writeln!(
        out,
        "}}\n\
         let mark = scratch.mark();\n\
         let result = run(scratch, {});\n\
         scratch.reset(mark);\n\
         result\n}}\n",
        s.args
    )
    .unwrap();
}
