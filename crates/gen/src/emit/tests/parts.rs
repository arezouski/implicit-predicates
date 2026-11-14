//! The second test of the filter, `filter_axes`, with the bound by parts,
//! read back from its text: its constants are those of the derivation, bit
//! for bit; it has the value of the source polynomial; it sums every term of
//! its tables, each a chain of products that starts with its constant; its
//! decision is made of comparisons that are false for NaN, without
//! short-circuits; it returns zero only under the guards of the certain
//! zero; and every function of the chain calls it right after `filter`. A
//! multihomogeneous form has no such test, and a form beyond the budget of
//! the bound by parts has none either, with a warning.

use super::readback::{function, interpret_statements};
use super::{INCIRCLE, INSPHERE, NEGATIONS, ORIENT3D, expand, value_at};
use crate::bound::{PartsConstants, PartsOutcome, parts_bound};
use crate::differences::{DiffGraph, DiffNode, difference_graph};
use crate::dyadic::Dyadic;
use crate::emit::edges::{KAPPA_OVER_EPSILON, SHAPE_SHIFTS, axes_edge_cases, shaped_scales};
use crate::emit::{Generated, generate, literal};
use crate::parse::{Predicate, parse};

/// A form of degree 2 that is not multihomogeneous, with the degree vectors
/// (2, 0, 0) and (0, 1, 1): no certain zero.
pub(super) const ZERO_IN_AN_AXIS: &str = "
fn zero_in_an_axis(a: Point3, b: Point3, c: Point3) -> Sign {
    (a.x - b.x) * (a.x - c.x) + (a.y - b.y) * (a.z - c.z)
}
";

/// `(ax + ay + az)^8`, by repeated squaring: 45 degree vectors at the root,
/// more than the bound by parts takes.
const OVER_BUDGET: &str = "
fn over_budget(a: Point3, b: Point3) -> Sign {
    let s0 = (a.x - b.x) + (a.y - b.y) + (a.z - b.z);
    let s1 = s0 * s0;
    let s2 = s1 * s1;
    s2 * s2
}
";

const AXES: [&str; 3] = ["x", "y", "z"];

/// A predicate with its difference form, the code generated for it and its
/// constants by parts, when the bound by parts is derived.
pub(super) struct WithParts {
    pub(super) predicate: Predicate,
    pub(super) graph: DiffGraph,
    pub(super) generated: Generated,
    pub(super) parts: Option<(PartsConstants, bool)>,
}

pub(super) fn with_parts(sources: &[&str]) -> Vec<WithParts> {
    let mut out = Vec::new();
    for source in sources {
        let predicates = parse(source, "t.pred").unwrap();
        let generated = generate(source, "t.pred", "run the tests.").unwrap();
        for (predicate, generated) in predicates.into_iter().zip(generated) {
            let graph = difference_graph(&predicate.main, "t.pred", predicate.position).unwrap();
            let parts = if graph.axis_degrees().is_some() {
                None
            } else {
                match parts_bound(&graph, "t.pred", predicate.position).unwrap() {
                    PartsOutcome::Bound(b) => Some((b.constants(), b.certain_zero())),
                    PartsOutcome::Unavailable(_) => None,
                }
            };
            out.push(WithParts {
                predicate,
                graph,
                generated,
                parts,
            });
        }
    }
    out
}

/// The test predicates whose forms are not multihomogeneous, all with the
/// bound by parts: insphere, incircle, `n` and `zero_in_an_axis`.
fn not_multihomogeneous() -> Vec<WithParts> {
    let all: Vec<WithParts> = with_parts(&[INSPHERE, INCIRCLE, NEGATIONS, ZERO_IN_AN_AXIS])
        .into_iter()
        .filter(|w| w.parts.is_some())
        .collect();
    let names: Vec<&str> = all.iter().map(|w| w.predicate.name.as_str()).collect();
    assert_eq!(names, ["insphere", "incircle", "n", "zero_in_an_axis"]);
    all
}

/// The statements of `filter_axes`, without its comments.
fn code(stages: &str) -> String {
    function(stages, "filter_axes")
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The value of the statement `let {name} = ...;` of `code`.
fn statement<'a>(code: &'a str, name: &str) -> &'a str {
    let prefix = format!("let {name} = ");
    let line = code
        .lines()
        .find(|l| l.starts_with(&prefix))
        .unwrap_or_else(|| panic!("no {name} in {code}"));
    line.strip_prefix(&prefix)
        .unwrap()
        .strip_suffix(';')
        .unwrap()
}

/// The text of the generic function `name` in the unformatted module
/// `code`, up to the documentation of the next item.
fn generic<'a>(code: &'a str, name: &str) -> &'a str {
    let start = code
        .find(&format!("pub fn {name}<"))
        .unwrap_or_else(|| panic!("no function {name}"));
    let rest = &code[start..];
    rest[..rest.find("\n///").expect("the next item")].trim_end()
}

/// A literal that reads back as exactly `x`.
fn exact_literal(x: f64) -> String {
    let text = literal(x);
    assert_eq!(
        text.parse::<f64>().unwrap().to_bits(),
        x.to_bits(),
        "{text}"
    );
    text
}

#[test]
fn the_constants_by_parts_are_those_of_the_derivation_bit_for_bit() {
    for w in not_multihomogeneous() {
        let (c, _) = w.parts.as_ref().unwrap();
        let s = &w.generated.stages;
        let epsilon: Vec<String> = c
            .epsilon
            .iter()
            .map(|t| {
                let g = t.degrees;
                format!(
                    "([{}, {}, {}], {})",
                    g[0],
                    g[1],
                    g[2],
                    exact_literal(t.value)
                )
            })
            .collect();
        let expected = format!(
            "pub const AXES_EPSILON: [([u32; 3], f64); {}] = [{}];",
            epsilon.len(),
            epsilon.join(", ")
        );
        assert!(s.contains(&expected), "{expected}\n{s}");
        let kappa: Vec<String> = c
            .kappa
            .iter()
            .map(|t| {
                let h = t.rest;
                format!(
                    "({}, [{}, {}, {}], {})",
                    t.axis,
                    h[0],
                    h[1],
                    h[2],
                    exact_literal(t.value)
                )
            })
            .collect();
        let expected = format!(
            "pub const AXES_KAPPA: [(usize, [u32; 3], f64); {}] = [{}];",
            kappa.len(),
            kappa.join(", ")
        );
        assert!(s.contains(&expected), "{expected}\n{s}");
        for (name, value) in [("AXES_D_MIN", c.d_min), ("AXES_D_MAX", c.d_max)] {
            let expected = format!("pub const {name}: f64 = {};", exact_literal(value));
            assert!(s.contains(&expected), "{expected}");
        }
    }
    // In-sphere: three terms of epsilon and nine of kappa.
    let insphere = &not_multihomogeneous()[0];
    let (c, zero) = insphere.parts.as_ref().unwrap();
    assert_eq!((c.epsilon.len(), c.kappa.len(), *zero), (3, 9, true));
}

#[test]
fn filter_axes_has_the_value_of_the_source_polynomial() {
    for w in not_multihomogeneous() {
        let code = function(&w.generated.stages, "filter_axes");
        assert_eq!(
            expand(&interpret_statements(code, "p")),
            expand(&w.predicate.main),
            "{}",
            w.predicate.name
        );
    }
}

#[test]
fn filter_axes_is_a_cold_stage_with_its_own_signature() {
    for w in not_multihomogeneous() {
        let s = &w.generated.stages;
        let points = w.predicate.params.len();
        let signature = format!(
            "#[allow(clippy::manual_range_contains)]\n#[cold]\n#[inline(never)]\n\
             pub fn filter_axes(p: [&[f64; 3]; {points}], e: [f64; 3]) -> Option<Sign> {{"
        );
        assert!(s.contains(&signature), "{signature}\n{s}");
        let generics: Vec<String> = (0..points).map(|i| format!("P{i}: Point")).collect();
        let points_signature = format!("pub fn filter_axes_points<{}>(", generics.join(", "));
        assert!(s.contains(&points_signature), "{points_signature}");
        let body = generic(s, "filter_axes_points");
        assert!(
            body.contains("implicit_predicates_core::filter::uncertainty(["),
            "{body}"
        );
        assert!(body.contains("{\n[0.0; 3]\n}"), "{body}");
        assert!(body.ends_with("filter_axes(p, e)\n}"), "{body}");
        // `filter` itself is the global filter, unchanged.
        assert!(s.contains(&format!(
            "#[inline]\npub fn filter(p: [&[f64; 3]; {points}], e: f64) -> Option<Sign> {{"
        )));
    }
}

#[test]
fn each_axis_of_the_range_takes_the_maximum_of_its_own_leaves() {
    for w in not_multihomogeneous() {
        let (c, _) = w.parts.as_ref().unwrap();
        let code = code(&w.generated.stages);
        let mut d = Vec::new();
        for (k, axis) in AXES.iter().enumerate() {
            let name = format!("delta_{axis}");
            if !c.range_axes[k] {
                assert!(!code.contains(&format!("let {name} = ")), "{code}");
                assert!(!code.contains(&format!("let d{axis} = ")), "{code}");
                d.push("0.0".to_string());
                continue;
            }
            let expected: Vec<String> = (0..w.graph.nodes.len())
                .filter(|&id| {
                    matches!(w.graph.nodes[id], DiffNode::Leaf { axis, .. } if usize::from(axis) == k)
                })
                .map(|id| format!("t{id}"))
                .collect();
            let mut read = Vec::new();
            for (i, line) in code
                .lines()
                .filter(|l| l.starts_with(&format!("let {name} = ")))
                .enumerate()
            {
                let value = line
                    .strip_prefix(&format!("let {name} = "))
                    .unwrap()
                    .strip_suffix(';')
                    .unwrap();
                let leaf = if i == 0 {
                    value.strip_suffix(".abs()").unwrap()
                } else {
                    value
                        .strip_prefix(&format!("max({name}, "))
                        .and_then(|v| v.strip_suffix(".abs())"))
                        .unwrap_or_else(|| panic!("{line}"))
                };
                read.push(leaf.to_string());
            }
            assert_eq!(read, expected, "{}: {code}", w.predicate.name);
            assert_eq!(
                statement(&code, &format!("d{axis}")),
                format!("{name} + e[{k}]")
            );
            d.push(format!("d{axis}"));
        }
        assert_eq!(statement(&code, "d"), format!("[{}]", d.join(", ")));
    }
}

#[test]
fn the_bound_sums_every_term_of_the_tables_as_a_chain_from_its_constant() {
    for w in not_multihomogeneous() {
        let s = &w.generated.stages;
        let code = code(s);
        // Every term of both tables, none left out, whatever `e` is.
        let bound = "let mut bound = 0.0;\n\
                     for (g, c) in AXES_EPSILON {\n\
                     bound += product_chain(c, g, d);\n\
                     }\n\
                     for (k, h, c) in AXES_KAPPA {\n\
                     bound += product_chain(c * e[k], h, d);\n\
                     }";
        assert!(code.contains(bound), "{code}");
        assert_eq!(code.matches("bound").count(), 4, "{code}");
        // The chain multiplies its start by each factor in turn.
        let chain = "#[inline(always)]\n\
                     fn product_chain(start: f64, g: [u32; 3], d: [f64; 3]) -> f64 {\n\
                     let mut x = start;\n\
                     for (&n, &dk) in g.iter().zip(&d) {\n\
                     for _ in 0..n {\n\
                     x *= dk;\n\
                     }\n\
                     }\n\
                     x\n\
                     }\n";
        assert!(s.contains(chain), "{s}");
        assert_eq!(s.matches("fn product_chain(").count(), 1);
    }
}

#[test]
fn the_decision_is_made_of_comparisons_that_are_false_for_nan() {
    for w in not_multihomogeneous() {
        let (c, zero) = w.parts.as_ref().unwrap();
        let code = code(&w.generated.stages);
        let valid: Vec<String> = (0..3)
            .map(|k| format!("(e[{k}] >= 0.0) & (e[{k}] <= f64::MAX)"))
            .collect();
        assert_eq!(statement(&code, "valid"), valid.join(" & "));
        let mut decided = vec!["valid".to_string()];
        for k in (0..3).filter(|&k| c.range_axes[k]) {
            let axis = AXES[k];
            decided.push(format!("(AXES_D_MIN <= d{axis}) & (d{axis} <= AXES_D_MAX)"));
        }
        decided.push("(det.abs() > bound)".to_string());
        assert_eq!(statement(&code, "decided"), decided.join(" & "));
        for forbidden in ["&&", "||", "!", "contains", "<= bound", "/"] {
            assert!(!code.contains(forbidden), "{forbidden} in {code}");
        }
        assert!(
            code.contains(
                "\nlet sign = if det > 0.0 { Sign::Positive } else { Sign::Negative };\n"
            )
        );
        let name = &w.predicate.name;
        if !zero {
            assert!(!code.contains("Sign::Zero"), "{name}: {code}");
            assert!(!code.contains("== 0.0"), "{name}: {code}");
            assert!(code.ends_with("decided.then_some(sign)"), "{code}");
            continue;
        }
        let axes: Vec<usize> = (0..3).filter(|&k| c.range_axes[k]).collect();
        let flat: Vec<String> = axes
            .iter()
            .map(|&k| format!("(d{} == 0.0)", AXES[k]))
            .collect();
        assert_eq!(statement(&code, "flat"), flat.join(" | "));
        let finite: Vec<String> = axes
            .iter()
            .map(|&k| format!("(d{} <= f64::MAX)", AXES[k]))
            .collect();
        assert_eq!(statement(&code, "finite"), finite.join(" & "));
        assert_eq!(
            statement(&code, "zero"),
            "valid & finite & flat & (det == 0.0)"
        );
        assert!(
            code.contains("\nlet sign = if zero { Sign::Zero } else { sign };\n"),
            "{code}"
        );
        assert_eq!(code.matches("Sign::Zero").count(), 1, "{code}");
        assert!(code.ends_with("(decided | zero).then_some(sign)"), "{code}");
    }
}

#[test]
fn only_insphere_has_the_certain_zero() {
    let zeros: Vec<(String, bool)> = not_multihomogeneous()
        .into_iter()
        .map(|w| (w.predicate.name.clone(), w.parts.unwrap().1))
        .collect();
    assert_eq!(
        zeros,
        [
            ("insphere".to_string(), true),
            ("incircle".to_string(), false),
            ("n".to_string(), false),
            ("zero_in_an_axis".to_string(), false)
        ]
    );
}

#[test]
fn every_function_of_the_chain_calls_filter_axes_right_after_filter() {
    let points_args = |n: usize| -> String {
        (0..n)
            .map(|i| format!("p{i}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    for w in not_multihomogeneous() {
        let s = &w.generated.stages;
        let name = &w.predicate.name;
        let staged = function(s, "staged");
        let explicit_in = function(s, "exact_stages_explicit_in");
        let any_in = generic(s, "exact_stages_in");
        let args = points_args(w.predicate.params.len());
        // Points of any type: `filter_axes_points` before any exact stage.
        let first = format!(
            "}}\nif let Some(sign) = filter_axes_points({args}) {{\nreturn Ok(sign);\n}}\n\
             let mark = scratch.mark();"
        );
        assert!(any_in.contains(&first), "{name}: {any_in}");
        match name.as_str() {
            "insphere" => {
                // No stage on expansions: the filter, then `filter_axes`.
                assert!(
                    staged.ends_with("{\nfilter(p, 0.0)\n.or_else(|| filter_axes(p, [0.0; 3]))"),
                    "{staged}"
                );
                assert!(!s.contains("pub fn exact_stages("));
                let start = "if let Some(sign) = filter_axes(p, [0.0; 3]) {\nreturn Ok(sign);\n}\n\
                             let mark = scratch.mark();";
                assert!(explicit_in.contains(start), "{explicit_in}");
            }
            "incircle" => {
                // `staged` takes an arena for its stage over spans.
                let start = "if let Some(sign) = filter(p, 0.0) {\nreturn Ok(Some(sign));\n}\n\
                             if let Some(sign) = filter_axes(p, [0.0; 3]) {\nreturn Ok(Some(sign));\n}\n\
                             if let Some(sign) = exact_differences(p) {";
                assert!(staged.contains(start), "{staged}");
                let stages = function(s, "exact_stages");
                let start = "{\nif let Some(sign) = filter_axes(p, [0.0; 3]) {\nreturn Ok(Some(sign));\n}\n\
                             if let Some(sign) = exact_differences(p) {";
                assert!(stages.contains(start), "{stages}");
                assert!(!explicit_in.contains("filter_axes"), "{explicit_in}");
                assert!(explicit_in.contains("exact_stages(scratch, p)?"));
            }
            _ => {
                // Every stage on the stack.
                assert!(
                    staged.ends_with(
                        "{\nfilter(p, 0.0)\n.or_else(|| filter_axes(p, [0.0; 3]))\n\
                         .or_else(|| exact_differences(p))\n.or_else(|| expansions_translated(p))"
                    ),
                    "{name}: {staged}"
                );
                let stages = function(s, "exact_stages");
                assert!(
                    stages.ends_with(
                        "{\nfilter_axes(p, [0.0; 3])\n.or_else(|| exact_differences(p))\n\
                         .or_else(|| expansions_translated(p))"
                    ),
                    "{name}: {stages}"
                );
                assert!(!explicit_in.contains("filter_axes"), "{explicit_in}");
            }
        }
        // `staged_in` keeps its form: `filter_points`, then the functions
        // above.
        let staged_in = generic(s, "staged_in");
        assert!(!staged_in.contains("filter_axes"), "{staged_in}");
        assert!(
            w.generated.warnings.is_empty(),
            "{:?}",
            w.generated.warnings
        );
    }
}

#[test]
fn a_multihomogeneous_form_has_no_second_test() {
    for w in with_parts(&[ORIENT3D]) {
        assert!(w.parts.is_none());
        for text in [&w.generated.stages, &w.generated.checks] {
            for absent in ["filter_axes", "AXES_", "product_chain", "axes_edge_cases"] {
                assert!(!text.contains(absent), "{absent}");
            }
        }
        assert!(
            w.generated.warnings.is_empty(),
            "{:?}",
            w.generated.warnings
        );
    }
}

#[test]
fn a_form_beyond_the_budget_has_no_second_test_and_a_warning() {
    let all = with_parts(&[OVER_BUDGET]);
    let w = &all[0];
    assert!(w.parts.is_none());
    for text in [&w.generated.stages, &w.generated.checks] {
        for absent in ["filter_axes", "AXES_", "product_chain", "axes_edge_cases"] {
            assert!(!text.contains(absent), "{absent}");
        }
    }
    let warning = w
        .generated
        .warnings
        .iter()
        .find(|m| m.contains("by parts"))
        .unwrap_or_else(|| panic!("{:?}", w.generated.warnings));
    assert!(warning.contains("45 degree vectors"), "{warning}");
    assert!(
        warning.contains("`over_budget` has no `filter_axes`"),
        "{warning}"
    );
}

#[test]
fn the_checks_have_the_edge_cases_of_filter_axes() {
    for w in not_multihomogeneous() {
        let checks = &w.generated.checks;
        let points = w.predicate.params.len();
        assert!(checks.contains(&format!(
            "pub fn axes_edge_cases() -> Vec<([[f64; 3]; {points}], [f64; 3])> {{"
        )));
        assert!(checks.contains(&format!(
            "pub fn check_axes_edge_cases(\nmut decide: impl FnMut(&[[f64; 3]; {points}], [f64; 3]) -> Option<Sign>,\n) -> [[usize; 2]; 2] {{"
        )), "{checks}");
        for line in [
            "for (p, e) in &cases {",
            "assert_eq!(decide(below, twice), None,",
            "assert_eq!(decide(above, half), Some(exact(above)),",
        ] {
            assert!(checks.contains(line), "{line}: {checks}");
        }
    }
}

/// The exact bound by parts with the emitted constants `c` at the points `p`
/// with the uncertainty `e`, for the sizes `d_k = delta_k + e[k]` computed
/// in `f64` as `filter_axes` computes them, in its two parts, the terms of
/// epsilon and the terms of kappa; or `None` when a size is outside the
/// range.
fn parts_bound_at(
    graph: &DiffGraph,
    c: &PartsConstants,
    p: &[[f64; 3]],
    e: [f64; 3],
) -> Option<(Dyadic, Dyadic)> {
    let mut d = [0.0f64; 3];
    for node in &graph.nodes {
        if let DiffNode::Leaf {
            axis,
            minuend,
            subtrahend,
        } = *node
        {
            let k = usize::from(axis);
            d[k] = d[k].max((p[minuend][k] - p[subtrahend][k]).abs());
        }
    }
    for k in 0..3 {
        d[k] += e[k];
        if c.range_axes[k] && !(c.d_min <= d[k] && d[k] <= c.d_max) {
            return None;
        }
    }
    let product = |start: Dyadic, g: [u32; 3]| {
        let mut term = start;
        for (&n, &dk) in g.iter().zip(&d) {
            for _ in 0..n {
                term = &term * &Dyadic::from_f64(dk);
            }
        }
        term
    };
    let mut epsilon = Dyadic::integer(0);
    for t in &c.epsilon {
        epsilon = &epsilon + &product(Dyadic::from_f64(t.value), t.degrees);
    }
    let mut kappa = Dyadic::integer(0);
    for t in &c.kappa {
        let start = &Dyadic::from_f64(t.value) * &Dyadic::from_f64(e[t.axis]);
        kappa = &kappa + &product(start, t.rest);
    }
    Some((epsilon, kappa))
}

/// The edge cases of `filter_axes` straddle its bound, computed here apart
/// from the search, with their uncertainty; those with an uncertainty have
/// terms of kappa above `2^KAPPA_OVER_EPSILON` times the terms of epsilon, and sizes in the
/// range with half the uncertainty, which `check_axes_edge_cases` relies
/// on; and there are cases with no uncertainty and with one on each axis of
/// the range, on grids of one scale and of a scale per axis.
#[test]
fn the_edge_cases_of_filter_axes_straddle_its_bound() {
    for w in not_multihomogeneous()
        .into_iter()
        .filter(|w| ["insphere", "zero_in_an_axis"].contains(&w.predicate.name.as_str()))
    {
        let (c, _) = w.parts.as_ref().unwrap();
        let points = w.predicate.params.len();
        let name = &w.predicate.name;
        let cases = axes_edge_cases(&w.graph, points, c);
        assert!(cases.len() >= 64, "{name}: {} pairs", cases.len());
        let above = |q: &[[f64; 3]], e: [f64; 3]| {
            let (epsilon, kappa) =
                parts_bound_at(&w.graph, c, q, e).unwrap_or_else(|| panic!("{q:?}"));
            let bound = &epsilon + &kappa;
            let value = value_at(&w.predicate.main, q);
            value > bound || value < &Dyadic::integer(0) - &bound
        };
        // With no uncertainty and with one on each axis of the range.
        let mut kinds = [0usize; 4];
        let mut shaped = 0;
        for (e, below, over) in &cases {
            assert!(!above(below, *e), "{below:?} e = {e:?}");
            assert!(above(over, *e), "{over:?} e = {e:?}");
            let moved = below
                .iter()
                .flatten()
                .zip(over.iter().flatten())
                .filter(|(a, b)| a.to_bits() != b.to_bits())
                .count();
            assert_eq!(moved, 1, "{below:?} {over:?}");
            let sizes = |q: &[[f64; 3]]| {
                parts_bound_at(&w.graph, c, q, [0.0; 3]).map(|(epsilon, _)| epsilon)
            };
            shaped += usize::from(
                sizes(below).is_some() && {
                    let x = below.iter().flatten().fold(0.0f64, |m, v| m.max(v.abs()));
                    // Some axis far shorter than the largest coordinate.
                    (0..3).any(|k| c.range_axes[k] && below.iter().all(|q| q[k].abs() < x * 1e-5))
                },
            );
            if *e == [0.0; 3] {
                kinds[3] += 1;
                continue;
            }
            for k in 0..3 {
                kinds[k] += usize::from(e[k] > 0.0);
                assert!(e[k] == 0.0 || c.range_axes[k], "{e:?}");
            }
            let half = e.map(|x| x / 2.0);
            for q in [below, over] {
                let (epsilon, kappa) = parts_bound_at(&w.graph, c, q, *e).unwrap();
                assert!(
                    kappa > &epsilon * &Dyadic::pow2(KAPPA_OVER_EPSILON),
                    "{q:?} e = {e:?}"
                );
                assert!(parts_bound_at(&w.graph, c, q, half).is_some(), "{q:?}");
            }
        }
        for k in 0..3 {
            assert!(!c.range_axes[k] || kinds[k] >= 4, "{name}: {kinds:?}");
        }
        assert!(kinds[3] >= 32, "{name}: {kinds:?}");
        assert!(shaped >= 8, "{name}: {shaped}");
    }
}

/// The grids of `axes_edge_cases` are given a scale on every axis, in the
/// range of the bound or not, only when it is that of a normal `f64`, so
/// that building them cannot panic.
#[test]
fn a_shaped_grid_below_the_normal_numbers_is_skipped() {
    let [a, b] = SHAPE_SHIFTS;
    assert_eq!(shaped_scales(-100, None, b), Some([-100; 3]));
    assert_eq!(
        shaped_scales(-100, Some((1, false)), a),
        Some([-100, -100 - a, -100])
    );
    assert_eq!(
        shaped_scales(-100, Some((1, true)), a),
        Some([-100 - a, -100, -100 - a])
    );
    // One axis below the normal numbers skips the grid, whether it is the
    // shorter one or one of the others, which may be outside the range.
    assert_eq!(shaped_scales(-1022 + b - 1, Some((1, true)), b), None);
    assert_eq!(shaped_scales(-1022 + b - 1, Some((0, false)), b), None);
    assert_eq!(
        shaped_scales(-1022 + b, Some((0, false)), b),
        Some([-1022, -1022 + b, -1022 + b])
    );
    assert_eq!(shaped_scales(-1023, None, a), None);
}

/// The README of the generator quotes the margin of the terms of kappa and
/// the shapes of the grids of `axes_edge_cases`.
#[test]
fn the_readme_quotes_the_shapes_and_the_margin_of_the_edge_cases_of_filter_axes() {
    let readme: String = include_str!("../../../README.md")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let [a, b] = SHAPE_SHIFTS;
    for quoted in [
        format!("one axis `2^-{a}` or `2^-{b}` times as short or as long"),
        format!("more than `2^{KAPPA_OVER_EPSILON}` times the terms of epsilon"),
    ] {
        assert!(readme.contains(&quoted), "{quoted}");
    }
}
