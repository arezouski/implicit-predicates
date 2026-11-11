//! Every stage of `orient3d`, alone, against the exact sign, and the cases
//! at the edge of the error bound of the filter.

mod common;

use common::generated::exact;
use common::{Points, STAGES, all_in_range, edges, generated, orient3d, refs};
use indirect_predicates::stages::orient3d as stages;
use indirect_predicates_test_support::{
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

#[test]
fn the_bound_edge_cases_are_decided_correctly() {
    let cases = generated::bound_edge_cases();
    assert!(cases.len() >= 64, "{} cases", cases.len());
    let (mut decided, mut declined) = (0, 0);
    for p in &cases {
        let expected = exact(p);
        match stages::filter(refs(p), 0.0) {
            Some(sign) => {
                assert_eq!(sign, expected, "filter on {p:?}");
                decided += 1;
            }
            None => declined += 1,
        }
        for (name, stage) in STAGES {
            if let Some(sign) = stage(refs(p)) {
                assert_eq!(sign, expected, "{name} on {p:?}");
            }
        }
        if all_in_range(p, stages::UNTRANSLATED_RANGE) {
            assert_eq!(orient3d(p), Some(expected), "{p:?}");
        }
    }
    // The cases straddle the bound: the filter decides some, not all.
    assert!(
        decided > 0 && declined > 0,
        "{decided} decided, {declined} declined"
    );
}
