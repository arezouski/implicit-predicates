//! Checks that the predicate benchmark inputs are what the benchmarks claim,
//! and that the baseline libraries and `implicit-predicates` agree with the
//! exact signs on them, on explicit points and on points of any type.
//!
//! The group of each recorded `insphere` call that the filter does not
//! decide is checked with the logic of the corpus sweep of
//! `implicit-predicates`, whose test helpers are included from that crate.

use implicit_predicates::stages::insphere;
use implicit_predicates::stages::orient3d as stages;
use implicit_predicates::{AnyPoint, Point, Scratch, orient3d_in};
use implicit_predicates_benches::predicates::{
    CORPUS_DIR, INSPHERE_MAX_IMPLICIT, ImplicitInput, InSphereGroup, InSphereImplicitInput,
    InSphereInput, InputSet, MAX_IMPLICIT, Orient3dInput, SET_SIZE, any_explicit_sets,
    big_integers_stage, exact_sets, geometry_insphere_sign, geometry_orient3d_sign,
    implicit_predicates_insphere_any_sign, implicit_predicates_orient3d_any_sign,
    implicit_predicates_orient3d_sign, implicit_recorded_signs, implicit_set_name, implicit_sets,
    insphere_any_explicit_sets, insphere_big_integers_stage, insphere_exact_sets,
    insphere_implicit_sets, insphere_set_name, insphere_sets, naive_insphere, naive_orient3d,
    orient3d_sets, out_of_range_sets, robust_insphere_sign, robust_orient3d_sign,
};
use implicit_predicates_corpus::exact;
use implicit_predicates_oracle::Dyadic;

// The helpers are written for the tests of `implicit-predicates`, whose
// `std` feature this package does not have.
#[allow(unexpected_cfgs)]
#[path = "../../crates/implicit-predicates/tests/common/mod.rs"]
mod common;

use common::insphere::{BIG_INTEGERS, FILTER, deciding_on_expansions};
use common::sign_of;
use common::why::Why;

fn exact_orient3d(p: &Orient3dInput) -> i8 {
    let [a, b, c, d] = p.map(|q| q.map(Dyadic::from_f64));
    exact::orient3d(&a, &b, &c, &d)
}

fn exact_insphere(p: &InSphereInput) -> i8 {
    let [a, b, c, d, e] = p.map(|q| q.map(Dyadic::from_f64));
    exact::insphere(&a, &b, &c, &d, &e)
}

fn set<'a, T>(sets: &'a [InputSet<T>], name: &str) -> &'a InputSet<T> {
    sets.iter()
        .find(|s| s.name.starts_with(name))
        .unwrap_or_else(|| panic!("no set named {name}"))
}

fn signum(x: f64) -> i8 {
    if x > 0.0 {
        1
    } else if x < 0.0 {
        -1
    } else {
        0
    }
}

#[test]
fn every_set_has_its_declared_size_and_is_deterministic() {
    let (first, second) = (orient3d_sets(), orient3d_sets());
    assert_eq!(first.len(), 4);
    for (x, y) in first.iter().zip(&second) {
        assert_eq!(x.name, y.name);
        assert_eq!(x.cases, y.cases, "{}", x.name);
        assert_eq!(x.cases.len(), SET_SIZE, "{}", x.name);
    }
    let (first, second) = (insphere_sets(), insphere_sets());
    assert_eq!(first.len(), 4);
    for (x, y) in first.iter().zip(&second) {
        assert_eq!(x.name, y.name);
        assert_eq!(x.cases, y.cases, "{}", x.name);
        assert_eq!(x.cases.len(), SET_SIZE, "{}", x.name);
    }
}

#[test]
fn degenerate_sets_are_exactly_zero() {
    for case in &set(&orient3d_sets(), "degenerate").cases {
        assert_eq!(exact_orient3d(case), 0, "{case:?}");
    }
    for case in &set(&insphere_sets(), "degenerate").cases {
        assert_eq!(exact_insphere(case), 0, "{case:?}");
    }
}

#[test]
fn random_sets_are_never_zero() {
    for case in &set(&orient3d_sets(), "random").cases {
        assert_ne!(exact_orient3d(case), 0, "{case:?}");
    }
    for case in &set(&insphere_sets(), "random").cases {
        assert_ne!(exact_insphere(case), 0, "{case:?}");
    }
}

#[test]
fn near_degenerate_sets_are_not_decided_by_the_naive_determinant_alone() {
    let sets = orient3d_sets();
    let cases = &set(&sets, "near_degenerate").cases;
    let wrong = cases
        .iter()
        .filter(|c| signum(naive_orient3d(c)) != exact_orient3d(c) || naive_orient3d(c) == 0.0)
        .count();
    assert!(
        2 * wrong >= cases.len(),
        "orient3d: {wrong} of {}",
        cases.len()
    );

    let sets = insphere_sets();
    let cases = &set(&sets, "near_degenerate").cases;
    let wrong = cases
        .iter()
        .filter(|c| signum(naive_insphere(c)) != exact_insphere(c) || naive_insphere(c) == 0.0)
        .count();
    assert!(
        2 * wrong >= cases.len(),
        "insphere: {wrong} of {}",
        cases.len()
    );
}

#[test]
fn robust_and_geometry_predicates_match_the_exact_sign_on_every_set() {
    for s in orient3d_sets() {
        for case in &s.cases {
            let exact = exact_orient3d(case);
            let [a, b, c, d] = case;
            // Every case of every set is decided before the stage on big
            // integers, which the benchmarks of explicit points do not
            // measure.
            assert!(
                stages::staged([a, b, c, d]).is_some(),
                "implicit-predicates decides only on big integers {}: {case:?}",
                s.name
            );
            assert_eq!(
                implicit_predicates_orient3d_sign(case),
                exact,
                "implicit-predicates {}: {case:?}",
                s.name
            );
            assert_eq!(
                robust_orient3d_sign(case),
                exact,
                "robust {}: {case:?}",
                s.name
            );
            assert_eq!(
                geometry_orient3d_sign(case),
                exact,
                "geometry-predicates {}: {case:?}",
                s.name
            );
        }
    }
    for s in insphere_sets() {
        for case in &s.cases {
            let exact = exact_insphere(case);
            assert_eq!(
                robust_insphere_sign(case),
                exact,
                "robust {}: {case:?}",
                s.name
            );
            assert_eq!(
                geometry_insphere_sign(case),
                exact,
                "geometry-predicates {}: {case:?}",
                s.name
            );
        }
    }
}

#[test]
fn naive_determinants_have_the_exact_sign_on_random_inputs() {
    // At magnitude below 2^10 with generic coordinates the rounding error of
    // the plain f64 determinant is far below the value, so the signs agree on
    // every case; a mismatch means the naive formula has the wrong convention.
    for case in &set(&orient3d_sets(), "random").cases {
        assert_eq!(
            signum(naive_orient3d(case)),
            exact_orient3d(case),
            "{case:?}"
        );
    }
    for case in &set(&insphere_sets(), "random").cases {
        assert_eq!(
            signum(naive_insphere(case)),
            exact_insphere(case),
            "{case:?}"
        );
    }
}

#[test]
fn the_stages_of_orient3d_decide_the_sets_they_are_benchmarked_on() {
    let sets = orient3d_sets();
    let decided = |name: &str, stage: fn([&[f64; 3]; 4]) -> bool| {
        let cases = &set(&sets, name).cases;
        let count = cases
            .iter()
            .filter(|c| stage([&c[0], &c[1], &c[2], &c[3]]))
            .count();
        (count, cases.len())
    };
    let all = |(count, len): (usize, usize)| count == len;
    assert!(all(
        decided("random", |p| stages::filter(p, [0.0; 3]).is_some())
    ));
    for name in ["near_degenerate", "degenerate"] {
        assert!(all(
            decided(name, |p| stages::expansions_translated(p).is_some())
        ));
        assert!(all(
            decided(name, |p| stages::expansions_untranslated(p).is_some())
        ));
    }
    assert!(all(
        decided("degenerate", |p| stages::exact_differences(p).is_some())
    ));
    // The moved coordinate of `near_degenerate` makes most differences
    // inexact, so `exact_differences` declines on most of that set.
    let (count, len) = decided("near_degenerate", |p| {
        stages::exact_differences(p).is_some()
    });
    assert!(10 * count < len, "{count} of {len}");
}

/// The exact coordinates of a point, from its definition: an `Lnc` with
/// parameters `p`, `q` and `t` is `q + t (p - q)`.
fn exact_point(point: &AnyPoint) -> [Dyadic; 3] {
    match point {
        AnyPoint::Explicit(e) => e.approx().map(Dyadic::from_f64),
        AnyPoint::Lnc(l) => {
            let t = Dyadic::from_f64(l.t());
            std::array::from_fn(|i| {
                let (p, q) = (Dyadic::from_f64(l.p()[i]), Dyadic::from_f64(l.q()[i]));
                &q + &(&t * &(&p - &q))
            })
        }
    }
}

fn exact_orient3d_any(p: &ImplicitInput) -> i8 {
    let [a, b, c, d] = p.each_ref().map(exact_point);
    exact::orient3d(&a, &b, &c, &d)
}

fn implicit_count(p: &ImplicitInput) -> usize {
    p.iter().filter(|q| !q.is_explicit()).count()
}

#[test]
fn implicit_sets_have_their_size_and_counts_and_are_deterministic() {
    let (first, second) = (implicit_sets(), implicit_sets());
    assert_eq!(first.len(), MAX_IMPLICIT + 1);
    for (k, (x, y)) in first.iter().zip(&second).enumerate() {
        assert_eq!(x.name, implicit_set_name(k));
        assert_eq!(x.name, y.name);
        assert_eq!(x.cases, y.cases, "{}", x.name);
        assert_eq!(x.cases.len(), SET_SIZE, "{}", x.name);
        for case in &x.cases {
            let count = implicit_count(case);
            if k == 0 {
                assert!((1..=MAX_IMPLICIT).contains(&count), "{}: {case:?}", x.name);
            } else {
                assert_eq!(count, k, "{}: {case:?}", x.name);
            }
        }
    }
}

#[test]
fn implicit_sets_have_the_recorded_signs() {
    for (set, signs) in implicit_sets().iter().zip(implicit_recorded_signs()) {
        assert_eq!(set.cases.len(), signs.len(), "{}", set.name);
        for (case, sign) in set.cases.iter().zip(signs) {
            assert_eq!(exact_orient3d_any(case), sign, "{}: {case:?}", set.name);
        }
    }
}

#[test]
fn any_explicit_sets_are_the_explicit_sets() {
    let explicit = orient3d_sets();
    let any = any_explicit_sets();
    assert_eq!(explicit.len(), any.len());
    for (x, y) in explicit.iter().zip(&any) {
        assert_eq!(x.name, y.name);
        assert_eq!(x.cases.len(), y.cases.len(), "{}", x.name);
        for (a, b) in x.cases.iter().zip(&y.cases) {
            assert!(
                b.iter().all(|q| matches!(q, AnyPoint::Explicit(_))),
                "{}: {b:?}",
                x.name
            );
            assert_eq!(*a, b.map(|q| *q.approx()), "{}", x.name);
        }
    }
}

#[test]
fn orient3d_matches_the_exact_sign_on_points_of_any_type() {
    let mut scratch = Scratch::growable(1 << 12);
    for set in implicit_sets().iter().chain(&any_explicit_sets()) {
        for case in &set.cases {
            let exact = exact_orient3d_any(case);
            assert_eq!(
                implicit_predicates_orient3d_any_sign(case),
                exact,
                "{}: {case:?}",
                set.name
            );
            let [a, b, c, d] = case;
            let sign = orient3d_in(&mut scratch, a, b, c, d).expect("a growable arena");
            assert_eq!(i8::from(sign), exact, "{}: {case:?}", set.name);
        }
    }
}

#[test]
fn exact_sets_need_the_exact_stages_and_every_form_decides_them() {
    let mut sets = exact_sets();
    assert_eq!(sets.len(), MAX_IMPLICIT);
    for (k, set) in sets.iter_mut().enumerate() {
        assert_eq!(set.name, implicit_set_name(k + 1));
        assert_eq!(set.points.len(), SET_SIZE, "{}", set.name);
        assert_eq!(set.coords.len(), SET_SIZE, "{}", set.name);
        let used = set.scratch.used();
        for (case, coords) in set.points.iter().zip(&set.coords) {
            assert_eq!(implicit_count(case), k + 1, "{}: {case:?}", set.name);
            let [a, b, c, d] = case;
            assert_eq!(stages::filter_points(a, b, c, d), None, "{case:?}");
            let exact = Some(exact_orient3d_any(case));
            let q = coords.each_ref();
            let s = &mut set.scratch;
            let translated = stages::expansions_translated_in(s, q).expect("room");
            let untranslated = stages::expansions_untranslated_in(s, q).expect("room");
            let chain = stages::exact_stages_in(s, a, b, c, d).expect("room");
            assert_eq!(translated.map(i8::from), exact, "{case:?}");
            assert_eq!(untranslated.map(i8::from), exact, "{case:?}");
            assert_eq!(Some(i8::from(chain)), exact, "{case:?}");
            let big = big_integers_stage(s, case);
            assert_eq!(Some(i8::from(big)), exact, "{case:?}");
            assert_eq!(s.used(), used, "the stages leave the arena as it was");
        }
    }
}

#[test]
fn the_stage_on_big_integers_has_the_exact_sign_on_the_explicit_sets_it_is_benchmarked_on() {
    let mut scratch = Scratch::growable(1 << 16);
    let sets = any_explicit_sets();
    for name in ["near_degenerate", "degenerate"] {
        for case in &set(&sets, name).cases {
            let used = scratch.used();
            let sign = big_integers_stage(&mut scratch, case);
            assert_eq!(
                Some(i8::from(sign)),
                Some(exact_orient3d_any(case)),
                "{case:?}"
            );
            assert_eq!(scratch.used(), used, "the stage leaves the arena as it was");
        }
    }
}

#[test]
fn out_of_range_sets_are_decided_by_no_stage_on_the_stack_and_have_the_exact_sign() {
    for set in out_of_range_sets() {
        assert_eq!(set.cases.len(), SET_SIZE, "{}", set.name);
        for case in &set.cases {
            let p = [&case[0], &case[1], &case[2], &case[3]];
            assert_eq!(stages::filter(p, [0.0; 3]), None, "{}: {case:?}", set.name);
            assert_eq!(stages::exact_stages(p), None, "{}: {case:?}", set.name);
            let sign = implicit_predicates::orient3d(&case[0], &case[1], &case[2], &case[3]);
            assert_eq!(
                i8::from(sign),
                exact_orient3d(case),
                "{}: {case:?}",
                set.name
            );
        }
    }
}

fn exact_insphere_any(p: &InSphereImplicitInput) -> i8 {
    let [a, b, c, d, e] = p.each_ref().map(exact_point);
    exact::insphere(&a, &b, &c, &d, &e)
}

#[test]
fn insphere_implicit_contender_has_the_exact_sign() {
    for (set, _) in insphere_implicit_sets() {
        for case in &set.cases {
            assert_eq!(
                implicit_predicates_insphere_any_sign(case),
                exact_insphere_any(case),
                "{}: {case:?}",
                set.name
            );
        }
    }
}

fn insphere_implicit_count(p: &InSphereImplicitInput) -> usize {
    p.iter().filter(|q| !q.is_explicit()).count()
}

#[test]
fn the_stages_of_insphere_decide_the_sets_they_are_benchmarked_on() {
    let sets = insphere_sets();
    let decided = |name: &str| {
        let cases = &set(&sets, name).cases;
        let count = cases
            .iter()
            .filter(|c| insphere::filter([&c[0], &c[1], &c[2], &c[3], &c[4]], 0.0).is_some())
            .count();
        (count, cases.len())
    };
    let (count, len) = decided("random");
    assert_eq!(count, len, "the filter on random");
    // One coordinate moved by a few ulps is far below what the filter can
    // resolve: it declines on every case of `near_degenerate`.
    let (count, _) = decided("near_degenerate");
    assert_eq!(count, 0, "the filter on near_degenerate");
}

#[test]
fn insphere_any_explicit_sets_are_the_explicit_sets() {
    let explicit = insphere_sets();
    let any = insphere_any_explicit_sets();
    assert_eq!(explicit.len(), any.len());
    for (x, y) in explicit.iter().zip(&any) {
        assert_eq!(x.name, y.name);
        assert_eq!(x.cases.len(), y.cases.len(), "{}", x.name);
        for (a, b) in x.cases.iter().zip(&y.cases) {
            assert!(
                b.iter().all(|q| matches!(q, AnyPoint::Explicit(_))),
                "{}: {b:?}",
                x.name
            );
            assert_eq!(*a, b.map(|q| *q.approx()), "{}", x.name);
        }
    }
}

#[test]
fn insphere_implicit_sets_have_their_counts_and_the_recorded_and_exact_signs() {
    let (first, second) = (insphere_implicit_sets(), insphere_implicit_sets());
    assert_eq!(first.len(), INSPHERE_MAX_IMPLICIT + 1);
    let mut scratch = Scratch::growable(1 << 16);
    for (k, ((x, signs), (y, _))) in first.iter().zip(&second).enumerate() {
        assert_eq!(x.name, insphere_set_name(k));
        assert_eq!(x.cases, y.cases, "{}", x.name);
        assert_eq!(x.cases.len(), SET_SIZE, "{}", x.name);
        assert_eq!(signs.len(), SET_SIZE, "{}", x.name);
        for (case, &sign) in x.cases.iter().zip(signs) {
            let count = insphere_implicit_count(case);
            if k == 0 {
                assert!(
                    (1..=INSPHERE_MAX_IMPLICIT).contains(&count),
                    "{}: {case:?}",
                    x.name
                );
            } else {
                assert_eq!(count, k, "{}: {case:?}", x.name);
            }
            let exact = exact_insphere_any(case);
            assert_eq!(exact, sign, "{}: {case:?}", x.name);
            let [a, b, c, d, e] = case;
            let staged = insphere::staged_in(&mut scratch, a, b, c, d, e).expect("room");
            assert_eq!(i8::from(staged), exact, "{}: {case:?}", x.name);
            if let Some(filtered) = insphere::filter_points(a, b, c, d, e) {
                assert_eq!(i8::from(filtered), exact, "{}: {case:?}", x.name);
            }
        }
    }
}

/// Every case of every set of [`insphere_exact_sets`] is in its group by the
/// logic of the corpus sweep, which decides the calls of the group
/// [`InSphereGroup::Spans`] on a stage over spans and gives
/// [`Why::ComponentOutOfRange`] for the calls of
/// [`InSphereGroup::OutOfRange`]; and every stage measured on it has the
/// sign of the independent oracle, which is also the recorded one.
#[test]
fn insphere_exact_sets_are_in_their_group_and_have_the_exact_sign_at_every_stage() {
    let mut sets = insphere_exact_sets();
    assert!(!sets.is_empty());
    let mut sweep = Scratch::growable(1 << 16);
    for set in &mut sets {
        let name = set.name();
        assert_eq!(set.points.len(), SET_SIZE, "{name}");
        assert_eq!(set.coords.len(), SET_SIZE, "{name}");
        assert_eq!(set.recorded.len(), SET_SIZE, "{name}");
        let used = set.scratch.used();
        for ((case, coords), &recorded) in set.points.iter().zip(&set.coords).zip(&set.recorded) {
            assert_eq!(
                insphere_implicit_count(case),
                set.implicit,
                "{name}: {case:?}"
            );
            let exact = exact_insphere_any(case);
            assert_eq!(recorded, exact, "{name}: {case:?}");
            let sign = Some(sign_of(exact));
            let [a, b, c, d, e] = case;
            assert_eq!(
                insphere::filter_points(a, b, c, d, e),
                None,
                "{name}: {case:?}"
            );
            let decision = deciding_on_expansions(&mut sweep, case);
            let q = coords.each_ref();
            let s = &mut set.scratch;
            let translated = insphere::expansions_translated_in(s, q).expect("room");
            let untranslated = insphere::expansions_untranslated_in(s, q).expect("room");
            match set.group {
                InSphereGroup::Spans => {
                    let decision = decision.unwrap_or_else(|why| panic!("{name}: {why:?}"));
                    assert_ne!(decision.stage, FILTER, "{name}: {case:?}");
                    assert_ne!(decision.stage, BIG_INTEGERS, "{name}: {case:?}");
                    assert_eq!(Some(decision.sign), sign, "{name}: {case:?}");
                    assert_eq!(translated, sign, "{name}: {case:?}");
                    assert_eq!(untranslated, sign, "{name}: {case:?}");
                }
                InSphereGroup::OutOfRange => {
                    assert_eq!(decision, Err(Why::ComponentOutOfRange), "{name}: {case:?}");
                    assert_eq!(translated, None, "{name}: {case:?}");
                    assert_eq!(untranslated, None, "{name}: {case:?}");
                }
            }
            let chain = insphere::exact_stages_in(s, a, b, c, d, e).expect("room");
            assert_eq!(Some(chain), sign, "{name}: {case:?}");
            let big = insphere_big_integers_stage(s, case);
            assert_eq!(Some(big), sign, "{name}: {case:?}");
            assert_eq!(s.used(), used, "the stages leave the arena as it was");
        }
    }
}

/// The versioned samples have no `insphere` call that no stage over spans
/// decides, so they give only the group [`InSphereGroup::Spans`], for every
/// number of implicit points. The other group needs larger corpora, named by
/// [`CORPUS_DIR`].
#[test]
fn the_versioned_samples_give_only_calls_that_a_stage_over_spans_decides() {
    if std::env::var_os(CORPUS_DIR).is_some() {
        return;
    }
    let names: Vec<_> = insphere_exact_sets().iter().map(|s| s.name()).collect();
    let expected: Vec<_> = (0..=INSPHERE_MAX_IMPLICIT)
        .map(|k| format!("spans/{k}_implicit"))
        .collect();
    assert_eq!(names, expected);
}
