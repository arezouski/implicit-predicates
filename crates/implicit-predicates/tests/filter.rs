//! The filter of `orient3d` at the ends of its range of scales, with values
//! that overflow, and with an uncertainty `e` that is not a valid one.
//!
//! The tests at the ends of the range read the emitted constants `D_MIN`
//! and `D_MAX`: they check that the filter honours them, while their values
//! are guarded by the derivation tests of the generator and by the
//! regeneration check, not by these tests.

mod common;

use common::generated::exact;
use common::{Points, STAGES, orient3d, pow2, refs};
use implicit_predicates::Sign;
use implicit_predicates::stages::orient3d as stages;
use implicit_predicates_test_support::{config, random4, sample};
use proptest::prelude::*;

proptest! {
    #![proptest_config(config())]

    /// `D` just outside `D_MIN..=D_MAX`, as emitted, with `e` zero or not.
    #[test]
    fn the_filter_does_not_decide_outside_d_min_and_d_max(
        d in prop_oneof![
            Just(stages::D_MIN.next_down()),
            Just(stages::D_MAX.next_up()),
            (1i32..=60).prop_map(|j| stages::D_MIN * pow2(-j)),
            (1i32..=60).prop_map(|j| stages::D_MAX * pow2(j)),
        ],
        share in prop_oneof![Just(None), (1i32..=8).prop_map(Some)],
        p in points_at_scale(),
    ) {
        // With `e = d 2^-share`, `delta = d - e`, so that `delta + e` rounds
        // to `d`.
        let e = share.map_or(0.0, |j| d * pow2(-j));
        let delta = d - e;
        prop_assume!(delta + e == d);
        let points = with_delta(&p, delta);
        prop_assert_eq!(stages::filter(refs(&points), e), None, "{:?} e = {:e}", points, e);
    }

    /// `D` at the emitted `D_MIN` and `D_MAX` and just inside them, with
    /// coordinates whose products underflow near `D_MIN`.
    #[test]
    fn the_filter_is_right_just_inside_the_limits(
        d in prop_oneof![
            Just(stages::D_MIN),
            Just(stages::D_MIN.next_up()),
            Just(stages::D_MAX),
            Just(stages::D_MAX.next_down()),
            (1i32..=8).prop_map(|j| stages::D_MIN * pow2(j)),
            (1i32..=8).prop_map(|j| stages::D_MAX * pow2(-j)),
        ],
        p in points_at_scale(),
    ) {
        let points = with_delta(&p, d);
        if let Some(sign) = stages::filter(refs(&points), 0.0) {
            prop_assert_eq!(sign, exact(&points), "{:?}", points);
        }
    }
}

/// Coordinates for [`with_delta`]: each a fraction in `[-1, 1]` times a
/// power of two down to `2^-400`, so that near `D_MIN` some products
/// underflow.
fn points_at_scale() -> impl Strategy<Value = Points> {
    let value = (
        -1.0..=1.0f64,
        prop_oneof![3 => Just(0i32), 1 => 0i32..=60, 1 => 60i32..=400],
    )
        .prop_map(|(x, j)| x * pow2(-j));
    let point = [value.clone(), value.clone(), value];
    [point.clone(), point.clone(), point].prop_map(|[a, b, c]| [a, b, c, [0.0; 3]])
}

/// The points `p`, whose last point is the origin and whose coordinates are
/// fractions in `[-1, 1]`, scaled by `delta`, with the first coordinate set
/// to `delta`: the largest difference of coordinates is then exactly
/// `delta`.
fn with_delta(p: &Points, delta: f64) -> Points {
    let mut points = p.map(|point| point.map(|x| x * delta));
    points[0][0] = delta;
    points
}

#[test]
fn the_filter_decides_at_each_limit_and_not_beyond() {
    // det[a - d; b - d; c - d] = delta^3, far above the error bound.
    let unit = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0; 3]];
    for (limit, beyond) in [
        (stages::D_MIN, stages::D_MIN.next_down()),
        (stages::D_MAX, stages::D_MAX.next_up()),
    ] {
        let p = with_delta(&unit, limit);
        assert_eq!(
            stages::filter(refs(&p), 0.0),
            Some(Sign::Positive),
            "{limit:e}"
        );
        let p = with_delta(&unit, beyond);
        assert_eq!(stages::filter(refs(&p), 0.0), None, "{beyond:e}");
        // The exact stages still decide where their ranges allow.
        assert_eq!(exact(&p), Sign::Positive);
    }
}

#[test]
fn overflowing_differences_do_not_decide() {
    let base = [
        [1.0, 2.0, 3.0],
        [-2.0, 1.0, 4.0],
        [3.0, -1.0, 2.0],
        [0.0, 1.0, -1.0],
    ];
    for i in 0..4 {
        for j in 0..4 {
            if i == j {
                continue;
            }
            for axis in 0..3 {
                // `1e308 - -1e308` overflows to infinity.
                let mut p: Points = base;
                p[i][axis] = 1e308;
                p[j][axis] = -1e308;
                for (name, stage) in STAGES {
                    assert_eq!(stage(refs(&p)), None, "{name} on {p:?}");
                }
                assert_eq!(orient3d(&p), None);
            }
        }
    }
}

#[test]
fn the_filter_decides_most_random_cases() {
    let cases = sample(random4(), 10_000);
    let decided = cases
        .iter()
        .filter(|p| stages::filter(refs(p), 0.0).is_some())
        .count();
    assert!(
        decided * 100 > cases.len() * 95,
        "{decided} of {}",
        cases.len()
    );
}

/// Random cases that the filter decides with `e = 0`.
fn decided_cases() -> Vec<Points> {
    let cases: Vec<Points> = sample(random4(), 200)
        .into_iter()
        .filter(|p| stages::filter(refs(p), 0.0).is_some())
        .collect();
    assert!(cases.len() > 150);
    cases
}

#[test]
fn a_negative_uncertainty_never_decides() {
    for p in decided_cases() {
        for e in [
            -f64::from_bits(1),
            -f64::MIN_POSITIVE,
            -1e-300,
            -0.5,
            -1.0,
            -f64::MAX,
            f64::NEG_INFINITY,
        ] {
            assert_eq!(stages::filter(refs(&p), e), None, "{p:?} e = {e:e}");
        }
        // Negative zero is zero.
        assert_eq!(
            stages::filter(refs(&p), -0.0),
            stages::filter(refs(&p), 0.0)
        );
    }
}

#[test]
fn a_nan_uncertainty_never_decides() {
    for p in decided_cases() {
        for e in [f64::NAN, -f64::NAN, f64::from_bits(0x7ff0_0000_0000_0001)] {
            assert!(e.is_nan());
            assert_eq!(stages::filter(refs(&p), e), None, "{p:?}");
        }
    }
}
