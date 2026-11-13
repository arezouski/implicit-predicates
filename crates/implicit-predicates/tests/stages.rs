//! Every stage of `orient3d`, alone, against the exact sign, and the cases
//! at the edge of the error bound of the filter, without and with an
//! uncertainty, checked by the functions of the generated checks on every
//! stage and on the chain with points of several types.

// `orient3d` needs the `std` feature.
#![cfg(feature = "std")]

mod common;

use common::generated::{check_bound_edge_cases, check_uncertainty_edge_cases, exact};
use common::{Points, STAGES, all_in_range, edges, generated, orient3d, refs};
use implicit_predicates::stages::orient3d as stages;
use implicit_predicates::{
    AnyPoint, BigCoords, ExactCoords, ExactError, Explicit, Lnc, Point, Scratch, Sign,
};
use implicit_predicates_core::filter::uncertainty;
use implicit_predicates_test_support::{
    config, coplanar4, extreme4, grid4, near_coplanar4, random4, repeated4, signed_zeros4,
};
use proptest::prelude::*;

fn any_family() -> impl Strategy<Value = Points> {
    prop_oneof![
        random4(),
        coplanar4(),
        near_coplanar4(),
        grid4(),
        repeated4(),
        signed_zeros4(),
        extreme4(&edges()),
    ]
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn every_stage_is_exact_when_it_decides(p in any_family()) {
        let expected = exact(&p);
        for (name, stage) in STAGES {
            if let Some(sign) = stage(refs(&p)) {
                prop_assert_eq!(sign, expected, "{} on {:?}", name, p);
            }
        }
    }

    #[test]
    fn the_two_expansion_forms_agree(
        p in prop_oneof![random4(), coplanar4(), near_coplanar4(), grid4(), extreme4(&edges())],
    ) {
        let translated = stages::expansions_translated(refs(&p));
        let untranslated = stages::expansions_untranslated(refs(&p));
        if let (Some(t), Some(u)) = (translated, untranslated) {
            prop_assert_eq!(t, u, "{:?}", p);
        }
        // Within the range of the translated form both decide.
        if all_in_range(&p, stages::TRANSLATED_RANGE) {
            prop_assert_eq!(translated, Some(exact(&p)), "{:?}", p);
            prop_assert_eq!(untranslated, Some(exact(&p)), "{:?}", p);
        }
    }

    #[test]
    fn exact_differences_decides_grid_cases(
        p in prop_oneof![grid4(), coplanar4(), signed_zeros4()],
    ) {
        // The differences of grid points are exact and within the range.
        prop_assert_eq!(stages::exact_differences(refs(&p)), Some(exact(&p)), "{:?}", p);
    }
}

/// The bound edge cases, by the generated check, on every stage and on the
/// chain with each type of point that can hold an explicit point exactly:
/// `[f64; 3]`, `Explicit`, `AnyPoint`, and a point on a line whose parents
/// are both the point, which goes through the stages over spans.
#[test]
fn the_bound_edge_cases_are_decided_correctly() {
    let [decided, declined] = check_bound_edge_cases(|p| stages::filter(refs(p), [0.0; 3]));
    assert!(decided + declined >= 64, "{decided} + {declined} cases");
    // The cases straddle the bound: the filter decides some, not all.
    assert!(
        decided > 0 && declined > 0,
        "{decided} decided, {declined} declined"
    );
    for (_, stage) in STAGES {
        check_bound_edge_cases(|p| stage(refs(p)));
    }
    // The chain on each type, and the range in which it must decide.
    let (stack, spans) = (stages::UNTRANSLATED_RANGE, stages::SPANS_UNTRANSLATED_RANGE);
    let chains: [(&str, Chain, (f64, f64)); 4] = [
        ("arrays", |p| orient3d(p), stack),
        (
            "Explicit",
            |p| {
                let [a, b, c, d] = p.map(Explicit::new_unchecked);
                implicit_predicates::orient3d(&a, &b, &c, &d)
            },
            stack,
        ),
        (
            "AnyPoint",
            |p| {
                let [a, b, c, d] = p.map(|x| AnyPoint::from(Explicit::new_unchecked(x)));
                implicit_predicates::orient3d(&a, &b, &c, &d)
            },
            stack,
        ),
        (
            "Lnc",
            |p| {
                let [a, b, c, d] = p.map(|x| Lnc::new_unchecked(x, x, 0.375));
                implicit_predicates::orient3d(&a, &b, &c, &d)
            },
            spans,
        ),
    ];
    let cases = generated::bound_edge_cases();
    for (name, chain, range) in chains {
        let [decided, _] = check_bound_edge_cases(chain);
        let inside = cases.iter().filter(|p| all_in_range(p, range));
        for p in inside.clone() {
            assert!(chain(p).is_some(), "{name} on {p:?}");
        }
        assert!(decided >= inside.count(), "{name}");
    }
}

/// The chain on four points of one type.
type Chain = fn(&Points) -> Option<Sign>;

/// A point given by an approximation and the error of each coordinate,
/// without exact coordinates: for the filter only.
struct Approx([f64; 3], [f64; 3]);

impl Point for Approx {
    fn approx(&self) -> &[f64; 3] {
        &self.0
    }

    fn err(&self) -> &[f64; 3] {
        &self.1
    }

    fn is_explicit(&self) -> bool {
        false
    }

    fn exact(&self, _: &mut Scratch<'_>) -> Result<ExactCoords, ExactError> {
        Err(ExactError::OutOfRange)
    }

    fn exact_big(&self, _: &mut Scratch<'_>) -> Result<BigCoords, ExactError> {
        Err(ExactError::OutOfRange)
    }
}

/// The uncertainty edge cases, by the generated check, on the filter given
/// the uncertainty of the errors, and on the filter of points of a type
/// that carries them.
#[test]
fn the_uncertainty_edge_cases_are_decided_correctly() {
    let filter =
        check_uncertainty_edge_cases(|p, err| stages::filter(refs(p), uncertainty(err.each_ref())));
    let points = check_uncertainty_edge_cases(|p, err| {
        let [a, b, c, d] = std::array::from_fn(|i| Approx(p[i], err[i]));
        stages::filter_points(&a, &b, &c, &d)
    });
    assert_eq!(filter, points);
    // The cases straddle the bound in every kind: the filter decides some,
    // not all.
    for (kind, [decided, declined]) in filter.iter().enumerate() {
        assert!(
            *decided > 0 && *declined > 0,
            "kind {kind}: {decided} decided, {declined} declined"
        );
    }
}

/// Every stage gives the same sign when it decides, so the results of
/// `staged` cannot show the order of its stages. This reads the order from
/// the generated source instead, and checks that the `CHAIN` of the tests
/// lists the same stages in the same order.
#[test]
fn the_chain_of_the_tests_is_the_order_of_staged() {
    let source = include_str!("../src/generated/orient3d.rs");
    let start = source.find("pub fn staged").expect("a staged function");
    let body = &source[start..];
    let body = &body[..body.find("\n}").expect("the end of staged")];
    let mut calls: Vec<(usize, &str)> = common::CHAIN
        .iter()
        .map(|(name, _)| {
            let call = if *name == "filter" {
                "filter(p, [0.0; 3])"
            } else {
                name
            };
            let at = body
                .find(call)
                .unwrap_or_else(|| panic!("{name} is not in staged: {body}"));
            (at, *name)
        })
        .collect();
    let listed: Vec<&str> = calls.iter().map(|(_, name)| *name).collect();
    calls.sort();
    let ordered: Vec<&str> = calls.iter().map(|(_, name)| *name).collect();
    assert_eq!(listed, ordered, "{body}");
    assert_eq!(
        body.matches("(p)").count() + 1,
        common::CHAIN.len(),
        "{body}"
    );
}

/// `exact_stages`, which the chain for points of any type runs on explicit
/// points after `filter_points`, runs the exact stages of `CHAIN` in the
/// same order.
#[test]
fn the_exact_stages_for_points_of_any_type_are_those_of_staged() {
    let source = include_str!("../src/generated/orient3d.rs");
    let start = source.find("pub fn exact_stages(").expect("exact_stages");
    let body = &source[start..];
    let body = &body[..body.find("\n}").expect("the end of exact_stages")];
    let at: Vec<usize> = common::CHAIN[1..]
        .iter()
        .map(|(name, _)| {
            body.find(&format!("{name}(p)"))
                .unwrap_or_else(|| panic!("{name} is not in exact_stages: {body}"))
        })
        .collect();
    assert!(at.is_sorted(), "{body}");
    assert_eq!(
        body.matches("(p)").count(),
        common::CHAIN.len() - 1,
        "{body}"
    );
}
