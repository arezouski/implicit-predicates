//! The filter of `orient3d` with uncertain points, at the ends of its range
//! of scales, and with values that overflow.

mod common;

use common::generated::exact;
use common::{Points, STAGES, orient3d, refs};
use indirect_predicates::Sign;
use indirect_predicates::stages::orient3d as stages;
use indirect_predicates_oracle::Dyadic;
use indirect_predicates_test_support::{config, coplanar4, grid4, near_coplanar4, random4, sample};
use proptest::prelude::*;

/// `2^exponent`, for `exponent` in `-1074..=1023`.
fn pow2(exponent: i32) -> f64 {
    assert!((-1074..=1023).contains(&exponent), "2^{exponent}");
    if exponent >= -1022 {
        f64::from_bits(u64::try_from(exponent + 1023).unwrap() << 52)
    } else {
        f64::from_bits(1 << (exponent + 1074))
    }
}

/// `x` rounded up to an `f64` that is at least the exact value `exact`, of
/// which `x` is the rounding to nearest.
fn at_least(x: f64, exact: &Dyadic) -> f64 {
    if Dyadic::from_f64(x) < *exact {
        x.next_up()
    } else {
        x
    }
}

/// `|x|`, exactly.
fn abs(x: &Dyadic) -> Dyadic {
    if x.sign() < 0 { -x } else { x.clone() }
}

/// The binary exponent of the largest magnitude among the coordinates, or
/// `None` if they are all zero.
fn top_exponent(p: &Points) -> Option<i32> {
    let max = p.iter().flatten().fold(0.0f64, |m, x| m.max(x.abs()));
    (max > 0.0).then(|| max.log2().floor() as i32)
}

/// `p` scaled by a power of two so that its largest coordinate is in
/// `[2^exponent, 2^(exponent + 1))`.
fn scaled_to(p: &Points, exponent: i32) -> Points {
    let factor = top_exponent(p).map_or(1.0, |top| pow2(exponent - top));
    p.map(|point| point.map(|x| x * factor))
}

/// The exponents of the scales: around 1 and near both ends of the range
/// `D_MIN..=D_MAX` of the filter.
fn scale() -> impl Strategy<Value = i32> {
    let (low, high) = (stages::D_MIN.log2() as i32, stages::D_MAX.log2() as i32);
    prop_oneof![
        2 => -4i32..=4,
        1 => low - 2..=low + 6,
        1 => high - 8..=high - 1,
    ]
}

/// The perturbation of one point: the exponent of its size relative to the
/// largest coordinate, a factor `2^extra` by which its error bound exceeds
/// the perturbation, and the offset of each coordinate in units of the size.
fn perturbation() -> impl Strategy<Value = (i32, i32, [f64; 3])> {
    (
        prop_oneof![0i32..=60, Just(1000)],
        prop_oneof![3 => Just(0i32), 1 => 0i32..=8, 1 => 30i32..=60],
        [-1.0..=1.0f64, -1.0..=1.0f64, -1.0..=1.0f64],
    )
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn the_filter_with_uncertainty_is_never_wrong(
        base in prop_oneof![random4(), coplanar4(), near_coplanar4(), grid4()],
        exponent in scale(),
        perturbations in [perturbation(), perturbation(), perturbation(), perturbation()],
    ) {
        // The true points, and approximations of them within known errors.
        let truth = scaled_to(&base, exponent);
        let top = top_exponent(&truth).unwrap_or(exponent);
        let mut approximation = truth;
        let mut errors = [0.0f64; 4];
        for (i, (size, extra, offsets)) in perturbations.into_iter().enumerate() {
            let unit = pow2((top - size).max(-1074));
            let mut error = 0.0f64;
            for axis in 0..3 {
                approximation[i][axis] = truth[i][axis] + offsets[axis] * unit;
                let exact = abs(&(&Dyadic::from_f64(approximation[i][axis])
                    - &Dyadic::from_f64(truth[i][axis])));
                let rounded = (approximation[i][axis] - truth[i][axis]).abs();
                error = error.max(at_least(rounded, &exact));
            }
            errors[i] = error * pow2(extra);
            prop_assume!(errors[i].is_finite());
        }
        // `e` is the sum of the two largest errors, rounded up, as a caller
        // computes it.
        errors.sort_by(|a, b| b.total_cmp(a));
        let e = at_least(
            errors[0] + errors[1],
            &(&Dyadic::from_f64(errors[0]) + &Dyadic::from_f64(errors[1])),
        );
        let expected = exact(&truth);
        if let Some(sign) = stages::filter(refs(&approximation), e) {
            prop_assert_eq!(
                sign,
                expected,
                "approximation {:?}, e = {:e}, truth {:?}",
                approximation,
                e,
                truth
            );
        }
        if expected == Sign::Zero {
            prop_assert_eq!(stages::filter(refs(&approximation), e), None);
        }
    }

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
