//! The stages over arena spans have the value of the source polynomial and
//! use only the operations of the arena; the stages on the stack are
//! emitted only when their temporaries fit.

use super::readback::{function, interpret_span_stage};
use super::{INSPHERE, NEGATIONS, ORIENT3D, expand};
use crate::emit::stages::{STACK_LIMIT, fits_on_stack};
use crate::parse::parse;

#[test]
fn the_text_of_every_stage_over_spans_has_the_value_of_the_source_polynomial() {
    let mut checked = 0;
    for source in [ORIENT3D, INSPHERE, NEGATIONS] {
        for (p, g) in parse(source, "t.pred")
            .unwrap()
            .iter()
            .zip(super::super::generate(source, "t.pred", "run the tests.").unwrap())
        {
            for stage in ["expansions_translated_in", "expansions_untranslated_in"] {
                if stage == "expansions_untranslated_in" && p.exact.is_none() {
                    assert!(!g.stages.contains(stage), "{}", p.name);
                    assert!(!g.stages.contains("SPANS_UNTRANSLATED_RANGE"), "{}", p.name);
                    continue;
                }
                let code = function(&g.stages, stage);
                // Only the arena computes: no expansion function, no
                // error-free transformation, no stack buffer.
                for forbidden in ["expansion::", "eft::", "[0.0;"] {
                    assert!(!code.contains(forbidden), "{} {stage}: {code}", p.name);
                }
                assert_eq!(
                    expand(&interpret_span_stage(code)),
                    expand(&p.main),
                    "{} {stage}",
                    p.name
                );
                checked += 1;
            }
        }
    }
    // orient3d and n have both forms, in-sphere and linear only one.
    assert_eq!(checked, 2 + 1 + 2 + 1);
}

#[test]
fn the_stack_limit_is_4096_doubles() {
    assert_eq!(STACK_LIMIT, 4096);
}

#[test]
fn a_stage_is_on_the_stack_exactly_when_its_temporaries_fit() {
    use crate::differences::difference_graph;
    use crate::range::{Leaves, exact_plan_differences, exact_plan_raw};
    use implicit_predicates_core::eft::{PRODUCT_OPERAND_MAX, PRODUCT_OPERAND_MIN};
    let (min, max) = (PRODUCT_OPERAND_MIN, PRODUCT_OPERAND_MAX);
    let (mut fit, mut not) = (0, 0);
    for source in [ORIENT3D, INSPHERE, NEGATIONS] {
        for (p, g) in parse(source, "t.pred")
            .unwrap()
            .iter()
            .zip(super::super::generate(source, "t.pred", "run the tests.").unwrap())
        {
            let diff = difference_graph(&p.main, "t.pred", p.position).unwrap();
            let mut totals = vec![
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
            if let Some(exact) = &p.exact {
                let plan = exact_plan_raw(exact, "t.pred", p.position, min, max).unwrap();
                totals.push(("expansions_untranslated", plan.total));
            }
            for (name, total) in totals {
                let emitted = g.stages.contains(&format!("pub fn {name}("));
                assert_eq!(emitted, total <= STACK_LIMIT, "{} {name}: {total}", p.name);
                if emitted { fit += 1 } else { not += 1 }
            }
        }
    }
    // In-sphere needs 4,154 doubles for its exact differences and 122,448
    // for its translated expansions.
    assert_eq!((fit, not), (3 + 3 + 2, 2));
}

#[test]
fn a_stage_fits_on_the_stack_up_to_the_limit_included() {
    assert!(fits_on_stack(0));
    assert!(fits_on_stack(STACK_LIMIT));
    assert!(!fits_on_stack(STACK_LIMIT + 1));
    assert!(!fits_on_stack(usize::MAX));
}

#[test]
fn every_stage_over_spans_declines_long_coordinates_and_other_weights() {
    for source in [ORIENT3D, INSPHERE, NEGATIONS] {
        for g in super::super::generate(source, "t.pred", "run the tests.").unwrap() {
            // The helper that every stage over spans calls on each
            // coordinate it reads.
            let helper = "fn span_in_range(e: &[f64], range: (f64, f64)) -> bool {\n\
                          e.len() <= MAX_COORD_COMPONENTS && e.iter().all(|&x| in_range(x, range))\n\
                          }\n";
            assert_eq!(g.stages.matches(helper).count(), 1, "{}", g.name);
            assert!(
                g.stages
                    .contains("pub const MAX_COORD_COMPONENTS: usize = 16;")
            );
            for stage in ["expansions_translated_in", "expansions_untranslated_in"] {
                if !g.stages.contains(&format!("pub fn {stage}(")) {
                    continue;
                }
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
}

#[test]
fn every_stage_over_spans_states_that_its_coordinates_must_be_valid_expansions() {
    for source in [ORIENT3D, INSPHERE, NEGATIONS] {
        for g in super::super::generate(source, "t.pred", "run the tests.").unwrap() {
            for stage in ["expansions_translated_in", "expansions_untranslated_in"] {
                let Some(at) = g.stages.find(&format!("pub fn {stage}(")) else {
                    continue;
                };
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
}
