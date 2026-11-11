//! The stages over arena spans have the value of the source polynomial and
//! use only the operations of the arena; the stages on the stack are
//! emitted only when their temporaries fit.

use super::readback::{function, interpret_span_stage};
use super::{INSPHERE, NEGATIONS, ORIENT3D, expand};
use crate::emit::stages::STACK_LIMIT;
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
