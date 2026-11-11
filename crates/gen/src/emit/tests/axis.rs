//! The emitted filter of a multihomogeneous form, read back from its text:
//! the value of the form, the maxima of each axis over its own leaves, the
//! bound as a sum of chains of products that start with their constant, the
//! decision combined without short-circuits and rejecting NaN, the branch of
//! the certain zero, and the constants; a form that is not multihomogeneous
//! keeps the global filter.

use super::readback::{function, interpret_statements};
use super::{INSPHERE, NEGATIONS, ORIENT3D, expand};
use crate::bound::{AxisConstants, FilterForm, filter_form};
use crate::differences::{DiffGraph, DiffNode, difference_graph};
use crate::emit::generate;
use crate::parse::{Predicate, parse};

/// A form of degrees (2, 1, 0): two axes, one of them squared.
const MIXED: &str = "
fn mixed(a: Point3, b: Point3, c: Point3) -> Sign {
    (a.x - c.x) * (b.x - c.x) * (a.y - b.y) - (a.x - b.x) * (a.x - b.x) * (c.y - b.y)
}
";

const AXES: [&str; 3] = ["x", "y", "z"];

/// A predicate with its difference form and the code generated for it.
pub(super) struct Emitted {
    pub(super) predicate: Predicate,
    pub(super) graph: DiffGraph,
    pub(super) stages: String,
}

fn emitted(sources: &[&str]) -> Vec<Emitted> {
    let mut out = Vec::new();
    for source in sources {
        let predicates = parse(source, "t.pred").unwrap();
        let generated = generate(source, "t.pred", "run the tests.").unwrap();
        for (predicate, g) in predicates.into_iter().zip(generated) {
            let graph = difference_graph(&predicate.main, "t.pred", predicate.position).unwrap();
            out.push(Emitted {
                predicate,
                graph,
                stages: g.stages,
            });
        }
    }
    out
}

/// The multihomogeneous test predicates (orient3d, `linear` and `mixed`),
/// with their per-axis constants and whether the certain zero applies.
pub(super) fn per_axis() -> Vec<(Emitted, AxisConstants, bool)> {
    let all = emitted(&[ORIENT3D, NEGATIONS, MIXED]);
    let out: Vec<_> = all
        .into_iter()
        .filter_map(
            |e| match filter_form(&e.graph, "t.pred", e.predicate.position).unwrap() {
                FilterForm::PerAxis(bound) => {
                    let zero = bound.certain_zero();
                    Some((e, bound.constants(), zero))
                }
                FilterForm::Global(_) => None,
            },
        )
        .collect();
    let names: Vec<&str> = out
        .iter()
        .map(|(e, ..)| e.predicate.name.as_str())
        .collect();
    assert_eq!(names, ["orient3d", "linear", "mixed"]);
    out
}

/// The statements of the filter, without its comments.
fn filter_code(stages: &str) -> String {
    function(stages, "filter")
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

#[test]
fn every_emitted_filter_has_the_value_of_the_source_polynomial() {
    let all = emitted(&[ORIENT3D, INSPHERE, NEGATIONS, MIXED]);
    assert_eq!(all.len(), 5);
    for e in all {
        let code = function(&e.stages, "filter");
        assert_eq!(
            expand(&interpret_statements(code, "p")),
            expand(&e.predicate.main),
            "{}",
            e.predicate.name
        );
    }
}

#[test]
fn each_axis_takes_the_maximum_of_its_own_leaves() {
    for (e, constants, _) in per_axis() {
        let code = filter_code(&e.stages);
        for (k, axis) in AXES.iter().enumerate() {
            let expected: Vec<String> = (0..e.graph.nodes.len())
                .filter(|&id| matches!(e.graph.nodes[id], DiffNode::Leaf { axis, .. } if usize::from(axis) == k))
                .map(|id| format!("t{id}"))
                .collect();
            let name = format!("delta_{axis}");
            let lines: Vec<&str> = code
                .lines()
                .filter(|l| l.starts_with(&format!("let {name} = ")))
                .collect();
            if constants.degrees[k] == 0 {
                assert!(expected.is_empty() && lines.is_empty(), "{code}");
                assert!(!code.contains(&format!("e[{k}]")), "{code}");
                continue;
            }
            let mut read = Vec::new();
            for (i, line) in lines.iter().enumerate() {
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
            assert_eq!(read, expected, "{}: {code}", e.predicate.name);
            assert_eq!(
                statement(&code, &format!("d{axis}")),
                format!("{name} + e[{k}]")
            );
        }
    }
}

#[test]
fn the_bound_is_a_sum_of_chains_of_products_that_start_with_their_constant() {
    for (e, constants, _) in per_axis() {
        let code = filter_code(&e.stages);
        let g = constants.degrees;
        let d_factors = |skip: Option<usize>| -> Vec<String> {
            let mut factors = Vec::new();
            for k in 0..3 {
                let n = g[k] - u32::from(skip == Some(k));
                factors.extend((0..n).map(|_| format!("d{}", AXES[k])));
            }
            factors
        };
        let mut expected = vec![("EPSILON".to_string(), d_factors(None))];
        for k in (0..3).filter(|&k| g[k] >= 1) {
            let mut factors = vec![format!("e[{k}]")];
            factors.extend(d_factors(Some(k)));
            expected.push((format!("KAPPA[{k}]"), factors));
        }
        // `let bound = EPSILON * ...;`, then one sum per axis of positive
        // degree, made only when its `e[k]` is positive.
        let lines: Vec<&str> = code
            .lines()
            .filter(|l| l.starts_with("let bound = "))
            .map(|l| {
                l.strip_prefix("let bound = ")
                    .unwrap()
                    .strip_suffix(';')
                    .unwrap()
            })
            .collect();
        assert_eq!(lines.len(), expected.len(), "{code}");
        assert!(lines.len() <= 4, "{code}");
        let mut terms = vec![lines[0]];
        for line in &lines[1..] {
            let (k, rest) = line
                .strip_prefix("if e[")
                .and_then(|l| l.split_once("] > 0.0 { bound + "))
                .unwrap_or_else(|| panic!("{line}"));
            let term = rest
                .strip_suffix(" } else { bound }")
                .unwrap_or_else(|| panic!("{line}"));
            assert!(term.starts_with(&format!("KAPPA[{k}] * ")), "{line}");
            terms.push(term);
        }
        let mut read: Vec<(String, Vec<String>)> = terms
            .iter()
            .map(|term| {
                assert!(!term.contains(['(', ')', '/', '-', '+']), "{term}");
                let mut factors: Vec<String> = term.split(" * ").map(String::from).collect();
                let constant = factors.remove(0);
                factors.sort();
                (constant, factors)
            })
            .collect();
        read.sort();
        for (_, factors) in &mut expected {
            factors.sort();
        }
        expected.sort();
        assert_eq!(read, expected, "{}: {code}", e.predicate.name);
    }
}

#[test]
fn the_decision_checks_every_axis_without_short_circuits_and_rejects_nan() {
    for (e, constants, _) in per_axis() {
        let code = filter_code(&e.stages);
        let decided = statement(&code, "decided");
        let mut expected = Vec::new();
        for k in (0..3).filter(|&k| constants.degrees[k] >= 1) {
            let axis = AXES[k];
            expected.push(format!("(e[{k}] >= 0.0)"));
            expected.push(format!("(D_MIN <= d{axis})"));
            expected.push(format!("(d{axis} <= D_MAX)"));
        }
        expected.push("(det.abs() > bound)".to_string());
        let mut read: Vec<String> = decided.split(" & ").map(String::from).collect();
        read.sort();
        expected.sort();
        assert_eq!(read, expected, "{}: {decided}", e.predicate.name);
        for forbidden in ["&&", "||", "!", "contains", "<= bound", "/"] {
            assert!(!code.contains(forbidden), "{forbidden} in {code}");
        }
        // The sign is chosen by a comparison, and the decision is not a
        // branch.
        let last = code.lines().last().unwrap();
        assert!(
            [
                "decided.then_some(sign)",
                "(decided | zero).then_some(sign)"
            ]
            .contains(&last),
            "{code}"
        );
    }
}

#[test]
fn only_a_flat_axis_with_an_exact_zero_returns_zero() {
    for (e, constants, certain_zero) in per_axis() {
        let code = filter_code(&e.stages);
        let name = &e.predicate.name;
        assert_eq!(certain_zero, name == "orient3d", "{name}");
        if !certain_zero {
            assert!(!code.contains("Sign::Zero"), "{code}");
            assert!(!code.contains("== 0.0"), "{code}");
            continue;
        }
        let axes: Vec<usize> = (0..3).filter(|&k| constants.degrees[k] >= 1).collect();
        let flat: Vec<String> = axes
            .iter()
            .map(|&k| format!("(d{} == 0.0)", AXES[k]))
            .collect();
        assert_eq!(statement(&code, "flat"), flat.join(" | "));
        let valid: Vec<String> = axes.iter().map(|&k| format!("(e[{k}] >= 0.0)")).collect();
        let finite: Vec<String> = axes
            .iter()
            .map(|&k| format!("(e[{k}] <= f64::MAX)"))
            .collect();
        assert_eq!(
            statement(&code, "zero"),
            format!(
                "{} & {} & flat & (det == 0.0)",
                valid.join(" & "),
                finite.join(" & ")
            )
        );
        // The zero is chosen by a select, and nothing else gives it.
        assert!(
            code.contains("\nlet sign = if zero { Sign::Zero } else { sign };\n"),
            "{code}"
        );
        assert_eq!(code.matches("Sign::Zero").count(), 1, "{code}");
        assert!(code.ends_with("(decided | zero).then_some(sign)"), "{code}");
    }
}

#[test]
fn the_per_axis_constants_are_the_derived_ones() {
    for (e, constants, _) in per_axis() {
        let s = &e.stages;
        let g = constants.degrees;
        assert!(s.contains(&format!(
            "pub const DEGREES: [u32; 3] = [{}, {}, {}];",
            g[0], g[1], g[2]
        )));
        assert!(s.contains(&format!(
            "pub const EPSILON: f64 = {:?};",
            constants.epsilon
        )));
        let k = constants.kappa;
        assert!(
            s.contains(&format!(
                "pub const KAPPA: [f64; 3] = [{:?}, {:?}, {:?}];",
                k[0], k[1], k[2]
            )),
            "{s}"
        );
        assert!(s.contains(&format!("pub const D_MIN: f64 = {:?};", constants.d_min)));
        assert!(s.contains(&format!("pub const D_MAX: f64 = {:?};", constants.d_max)));
        assert!(!s.contains("pub const DEGREE:"));
        for (kappa, degree) in constants.kappa.iter().zip(g) {
            assert_eq!(*kappa == 0.0, degree == 0);
        }
        assert!(
            s.contains(&format!(
                "#[allow(clippy::manual_range_contains)]\n#[inline(always)]\npub fn filter(p: [&[f64; 3]; {}], e: [f64; 3]) -> Option<Sign> {{",
                e.predicate.params.len()
            )),
            "{s}"
        );
        let staged = function(s, "staged");
        assert!(staged.contains("filter(p, [0.0; 3])"), "{staged}");
    }
}

#[test]
fn a_form_that_is_not_multihomogeneous_keeps_the_global_filter() {
    let all = emitted(&[INSPHERE, NEGATIONS]);
    for e in all.iter().filter(|e| e.predicate.name != "linear") {
        assert!(matches!(
            filter_form(&e.graph, "t.pred", e.predicate.position).unwrap(),
            FilterForm::Global(_)
        ));
        let s = &e.stages;
        let points = e.predicate.params.len();
        assert!(s.contains(&format!(
            "pub fn filter(p: [&[f64; 3]; {points}], e: f64) -> Option<Sign> {{"
        )));
        let degree = e.graph.degrees[e.graph.root];
        assert!(s.contains(&format!("pub const DEGREE: u32 = {degree};")));
        assert!(s.contains("pub const KAPPA: f64 = "));
        let code = filter_code(s);
        let mut bound = String::from("(EPSILON * d + KAPPA * e)");
        for _ in 1..degree {
            bound.push_str(" * d");
        }
        assert_eq!(statement(&code, "bound"), bound);
        assert_eq!(statement(&code, "d"), "delta + e");
        assert!(!code.contains("Sign::Zero"), "{code}");
        assert!(!code.contains('/'), "{code}");
        assert!(function(s, "staged").contains("filter(p, 0.0)"));
    }
}
