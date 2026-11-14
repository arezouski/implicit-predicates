//! The stages over arena spans have the value of the source polynomial and
//! use only the operations of the arena; the stages on the stack are
//! emitted only when their temporaries fit.

use super::readback::{function, interpret_span_stage};
use super::{INCIRCLE, INSPHERE, NEGATIONS, ORIENT3D, expand};
use crate::emit::stages::{STACK_LIMIT, fits_on_stack};
use crate::parse::parse;

#[test]
fn the_text_of_every_stage_over_spans_has_the_value_of_the_source_polynomial() {
    let mut checked = 0;
    for source in [ORIENT3D, INCIRCLE, NEGATIONS] {
        for (p, g) in parse(source, "t.pred")
            .unwrap()
            .iter()
            .zip(super::super::generate(source, "t.pred", "run the tests.").unwrap())
        {
            let code = function(&g.stages, "expansions_translated_in");
            // Only the arena computes: no expansion function, no error-free
            // transformation, no stack buffer.
            for forbidden in ["expansion::", "eft::", "[0.0;"] {
                assert!(!code.contains(forbidden), "{}: {code}", p.name);
            }
            assert_eq!(
                expand(&interpret_span_stage(code)),
                expand(&p.main),
                "{}",
                p.name
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 4);
}

#[test]
fn the_stack_limit_is_4096_doubles() {
    assert_eq!(STACK_LIMIT, 4096);
}

#[test]
fn a_stage_is_on_the_stack_exactly_when_its_temporaries_fit() {
    use crate::differences::difference_graph;
    use crate::range::{Leaves, exact_plan_differences};
    use implicit_predicates_core::eft::{PRODUCT_OPERAND_MAX, PRODUCT_OPERAND_MIN};
    let (min, max) = (PRODUCT_OPERAND_MIN, PRODUCT_OPERAND_MAX);
    let (mut fit, mut not) = (0, 0);
    for source in [ORIENT3D, INCIRCLE, NEGATIONS] {
        for (p, g) in parse(source, "t.pred")
            .unwrap()
            .iter()
            .zip(super::super::generate(source, "t.pred", "run the tests.").unwrap())
        {
            let diff = difference_graph(&p.main, "t.pred", p.position).unwrap();
            let totals = vec![
                ("exact_differences", Leaves::One),
                ("expansions_translated", Leaves::Two),
            ]
            .into_iter()
            .map(|(name, leaves)| {
                let plan =
                    exact_plan_differences(&diff, leaves, "t.pred", p.position, min, max).unwrap();
                (name, plan.total)
            })
            .collect::<Vec<_>>();
            for (name, total) in totals {
                let emitted = g.stages.contains(&format!("pub fn {name}("));
                assert_eq!(emitted, total <= STACK_LIMIT, "{} {name}: {total}", p.name);
                if emitted { fit += 1 } else { not += 1 }
            }
        }
    }
    // The translated expansions of in-circle pass the limit, its exact
    // differences do not.
    assert_eq!((fit, not), (2 + 1 + 2 + 2, 1));
}

#[test]
fn a_stage_fits_on_the_stack_up_to_the_limit_included() {
    assert!(fits_on_stack(0));
    assert!(fits_on_stack(STACK_LIMIT));
    assert!(!fits_on_stack(STACK_LIMIT + 1));
    assert!(!fits_on_stack(usize::MAX));
}

/// `(a.x - b.x)^2` doubled `doublings` times: the worst-case length of the
/// result doubles each time.
pub(super) fn doubled(doublings: usize) -> String {
    let mut body = String::from("    let d = a.x - b.x;\n    let x0 = d * d;\n");
    for i in 0..doublings {
        body.push_str(&format!("    let x{} = x{i} + x{i};\n", i + 1));
    }
    format!("fn doubled(a: Point3, b: Point3) -> Sign {{\n{body}    x{doublings}\n}}\n")
}

#[test]
fn a_form_without_a_plan_on_the_stack_is_left_to_the_stage_over_spans() {
    let stages = |source: &str| {
        let all = super::super::generate(source, "t.pred", "run the tests.").unwrap();
        let stages = all[0].stages.clone();
        [
            "exact_differences",
            "expansions_translated",
            "expansions_translated_in",
        ]
        .map(|name| stages.contains(&format!("pub fn {name}(")))
    };
    // Few doublings fit on the stack.
    assert_eq!(stages(&doubled(3)), [true, true, true]);
    // The temporaries of the form on two components pass the limit of the
    // stack before those of the form on one.
    assert_eq!(stages(&doubled(9)), [true, false, true]);
    assert_eq!(stages(&doubled(12)), [false, false, true]);
    // A temporary of `2^32` components or more has no plan on the stack;
    // the stage over spans still has one.
    assert_eq!(stages(&doubled(33)), [false, false, true]);
    // The predicates of the fixtures: every stage; the exact differences
    // and, in place of the translated expansions, whose temporaries pass the
    // limit, the stage over spans; and none, from degree 5 on.
    assert_eq!(stages(ORIENT3D), [true, true, true]);
    assert_eq!(stages(INCIRCLE), [true, false, true]);
    assert_eq!(stages(INSPHERE), [false, false, false]);
}

#[test]
fn the_check_of_one_coordinate_is_emitted_only_with_a_stage_on_the_stack() {
    let uses = |source: &str| {
        let all = super::super::generate(source, "t.pred", "run the tests.").unwrap();
        let stages = &all[0].stages;
        (
            stages.matches("fn in_range(").count(),
            stages.matches(" in_range(").count() + stages.matches("!in_range(").count(),
        )
    };
    // The definition and at least one call, or neither: an unused function
    // would not compile under `deny(warnings)`.
    let (defined, called) = uses(ORIENT3D);
    assert!(defined == 1 && called > 1, "{defined} {called}");
    assert_eq!(uses(INSPHERE), (0, 0));
}

#[test]
fn every_stage_over_spans_declines_long_coordinates_and_other_weights() {
    for source in [ORIENT3D, INCIRCLE, NEGATIONS] {
        for g in super::super::generate(source, "t.pred", "run the tests.").unwrap() {
            // The helper that every stage over spans calls on each
            // coordinate it reads.
            let helper = "fn span_in_range(e: &[f64], (min, max): (f64, f64)) -> bool {\n\
                          e.len() <= MAX_COORD_COMPONENTS && e.iter().all(|&x| (min..=max).contains(&x.abs()))\n\
                          }\n";
            assert_eq!(g.stages.matches(helper).count(), 1, "{}", g.name);
            assert!(
                g.stages
                    .contains("pub const MAX_COORD_COMPONENTS: usize = 16;")
            );
            let stage = "expansions_translated_in";
            let code = function(&g.stages, stage);
            let weights = code
                .find("if !p.iter().all(|q| q.w == Weight::One) {\nreturn Ok(None);\n}")
                .unwrap_or_else(|| panic!("{} {stage}: no check of the weights", g.name));
            let range = code
                .find(".all(|s| span_in_range(scratch.get(s), SPANS_")
                .unwrap_or_else(|| panic!("{} {stage}: no check of the range", g.name));
            let mark = code.find("let mark = scratch.mark();").unwrap();
            assert!(weights < mark && range < mark, "{} {stage}", g.name);
        }
    }
}

#[test]
fn every_stage_over_spans_states_that_its_coordinates_must_be_valid_expansions() {
    for source in [ORIENT3D, INCIRCLE, NEGATIONS] {
        for g in super::super::generate(source, "t.pred", "run the tests.").unwrap() {
            let stage = "expansions_translated_in";
            let at = g.stages.find(&format!("pub fn {stage}(")).unwrap();
            let doc_start = g.stages[..at].rfind("\n\n").unwrap();
            let doc: String = g.stages[doc_start..at]
                .lines()
                .filter_map(|l| l.strip_prefix("///"))
                .collect::<Vec<_>>()
                .join("");
            let doc = doc.split_whitespace().collect::<Vec<_>>().join(" ");
            assert!(
                doc.contains(
                    "Requirement: every coordinate is a valid expansion of `scratch`: \
                     nonzero components of strictly increasing magnitude that do not \
                     overlap, as [`Point::exact`](implicit_predicates_core::Point::exact) \
                     writes them. This is not checked, and the result is unspecified \
                     otherwise."
                ),
                "{} {stage}: {doc}",
                g.name
            );
        }
    }
}
