//! The filter of `orient3d`: its constants against their exact derived
//! values, the ends of its range of sizes on each axis, values that
//! overflow, an uncertainty `e` that is not a valid one, the certain zero of
//! a flat axis, non-finite inputs, and approximations of exact points within
//! an uncertainty on each axis.
//!
//! The tests at the ends of the range read the emitted constants `D_MIN`
//! and `D_MAX`: they check that the filter honours them, while their values
//! are checked by `the_constants_are_the_derived_ones`.

// `orient3d` needs the `std` feature.
#![cfg(feature = "std")]

mod common;

use common::boxes::{determinant_at, exact_corners};
use common::generated::exact;
use common::{Points, STAGES, is_derived, orient3d, pow2, refs};
use implicit_predicates::Sign;
use implicit_predicates::stages::orient3d as stages;
use implicit_predicates_core::filter::uncertainty;
use implicit_predicates_oracle::Dyadic;
use implicit_predicates_test_support::{config, coplanar4, grid4, near_coplanar4, random4, sample};
use proptest::prelude::*;

#[test]
fn the_constants_are_the_derived_ones() {
    // The rules of the per-axis bound give, for orient3d, epsilon = 46 u
    // and kappa_k = 6 on each axis, plus terms of order u; numeric tests
    // cannot tell a `KAPPA[k]` of 6 from one of 3, the error reachable on
    // one axis, so the values are checked here.
    assert_eq!(stages::DEGREES, [1, 1, 1]);
    let u = Dyadic::from_f64(pow2(-53));
    assert!(is_derived(stages::EPSILON, &(&Dyadic::from_f64(46.0) * &u)));
    for kappa in stages::KAPPA {
        assert!(is_derived(kappa, &Dyadic::from_f64(6.0)), "{kappa:e}");
    }
    assert_eq!(stages::D_MIN, pow2(-324));
    assert_eq!(stages::D_MAX, pow2(339));
}

proptest! {
    #![proptest_config(config())]

    /// `d_x` just outside `D_MIN..=D_MAX`, as emitted, with `e[0]` zero or
    /// not.
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
        let e = [e, 0.0, 0.0];
        prop_assert_eq!(stages::filter(refs(&points), e), None, "{:?} e = {:?}", points, e);
    }

    /// `d_x` at the emitted `D_MIN` and `D_MAX` and just inside them, with
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
        if let Some(sign) = stages::filter(refs(&points), [0.0; 3]) {
            prop_assert_eq!(sign, exact(&points), "{:?}", points);
        }
    }

    /// Approximations of exact points within errors on each coordinate,
    /// with `e` computed from them: when the filter decides, its sign is the
    /// exact sign at the approximations and at the corners of the error
    /// boxes that move the determinant furthest each way.
    #[test]
    fn the_filter_with_uncertainty_per_axis_is_never_wrong(
        base in prop_oneof![random4(), coplanar4(), near_coplanar4(), grid4()],
        exponent in prop_oneof![2 => -4i32..=4, 1 => -330i32..=-300, 1 => 300i32..=335],
        errors in [error_exponents(), error_exponents(), error_exponents(), error_exponents()],
    ) {
        let approximation = base.map(|point| point.map(|x| x * pow2(exponent)));
        let errors = errors.map(|point| point.map(|j| j.map_or(0.0, |j| pow2((exponent - j).max(-1074)))));
        let e = uncertainty(errors.each_ref());
        let Some(sign) = stages::filter(refs(&approximation), e) else {
            return Ok(());
        };
        for truth in exact_corners(&approximation, errors) {
            let expected = match determinant_at(&truth).sign() {
                -1 => Sign::Negative,
                0 => Sign::Zero,
                _ => Sign::Positive,
            };
            prop_assert_eq!(sign, expected, "{:?} e = {:?}", approximation, e);
        }
    }
}

/// The exponent of an error below the scale, or `None` for no error.
fn error_exponents() -> impl Strategy<Value = [Option<i32>; 3]> {
    let one = prop_oneof![1 => Just(None), 3 => (1i32..=60).prop_map(Some)];
    [one.clone(), one.clone(), one]
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
/// to `delta`: the largest difference of coordinates on the axis x is then
/// exactly `delta`.
fn with_delta(p: &Points, delta: f64) -> Points {
    let mut points = p.map(|point| point.map(|x| x * delta));
    points[0][0] = delta;
    points
}

/// `a = (sx, 0, 0)`, `b = (0, sy, 0)`, `c = (0, 0, sz)` and `d = 0`: the
/// determinant is `sx sy sz`, far above the error bound, and the largest
/// difference on each axis is the size given for it.
fn unit(s: [f64; 3]) -> Points {
    [
        [s[0], 0.0, 0.0],
        [0.0, s[1], 0.0],
        [0.0, 0.0, s[2]],
        [0.0; 3],
    ]
}

#[test]
fn the_filter_decides_at_the_limits_of_each_axis_and_not_beyond() {
    let sizes = [stages::D_MIN, 1.0, stages::D_MAX];
    for &x in &sizes {
        for &y in &sizes {
            for &z in &sizes {
                let s = [x, y, z];
                let p = unit(s);
                assert_eq!(
                    stages::filter(refs(&p), [0.0; 3]),
                    Some(Sign::Positive),
                    "{s:?}"
                );
                for axis in 0..3 {
                    for beyond in [stages::D_MIN.next_down(), stages::D_MAX.next_up()] {
                        let mut t = s;
                        t[axis] = beyond;
                        let q = unit(t);
                        assert_eq!(stages::filter(refs(&q), [0.0; 3]), None, "{t:?}");
                        // The exact stages still decide where their ranges
                        // allow.
                        assert_eq!(exact(&q), Sign::Positive);
                    }
                }
            }
        }
    }
}

#[test]
fn overflowing_differences_are_decided_only_on_big_integers() {
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
                assert_eq!(orient3d(&p), exact(&p), "{p:?}");
            }
        }
    }
}

#[test]
fn the_filter_decides_most_random_cases() {
    let cases = sample(random4(), 10_000);
    let decided = cases
        .iter()
        .filter(|p| stages::filter(refs(p), [0.0; 3]).is_some())
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
        .filter(|p| stages::filter(refs(p), [0.0; 3]).is_some())
        .collect();
    assert!(cases.len() > 150);
    cases
}

/// `e` with `x` on the axis `axis` and zero on the others.
fn on_axis(axis: usize, x: f64) -> [f64; 3] {
    let mut e = [0.0; 3];
    e[axis] = x;
    e
}

#[test]
fn an_invalid_uncertainty_on_any_axis_never_decides() {
    let invalid = [
        -f64::from_bits(1),
        -f64::MIN_POSITIVE,
        -1e-300,
        -0.5,
        -1.0,
        -f64::MAX,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NAN,
        -f64::NAN,
        f64::from_bits(0x7ff0_0000_0000_0001),
    ];
    for p in decided_cases() {
        for axis in 0..3 {
            for x in invalid {
                let e = on_axis(axis, x);
                assert_eq!(stages::filter(refs(&p), e), None, "{p:?} e = {e:?}");
            }
            // Negative zero is zero.
            assert_eq!(
                stages::filter(refs(&p), on_axis(axis, -0.0)),
                stages::filter(refs(&p), [0.0; 3])
            );
        }
    }
}

/// Grid points whose coordinates on `axis` are all `c`: that axis is flat,
/// and the predicate is exactly zero.
fn flat(p: &Points, axis: usize, c: f64) -> Points {
    p.map(|mut point| {
        point[axis] = c;
        point
    })
}

#[test]
fn a_flat_axis_with_exact_points_is_a_certain_zero() {
    let mut zeros = 0;
    for p in sample(grid4(), 200) {
        for axis in 0..3 {
            for c in [0.0, -0.0, 0.75, -3.0, 1e300] {
                let q = flat(&p, axis, c);
                assert_eq!(exact(&q), Sign::Zero, "{q:?}");
                assert_eq!(
                    stages::filter(refs(&q), [0.0; 3]),
                    Some(Sign::Zero),
                    "{q:?}"
                );
                zeros += 1;
                // An uncertainty on the other axes keeps the axis flat.
                for other in (0..3).filter(|&k| k != axis) {
                    let e = on_axis(other, 0.5);
                    assert_eq!(stages::filter(refs(&q), e), Some(Sign::Zero), "{q:?}");
                }
                // An uncertainty on the flat axis means that the true points
                // may not be flat: no zero, and no sign either.
                for x in [f64::from_bits(1), 1e-300, 0.5] {
                    let e = on_axis(axis, x);
                    assert_eq!(stages::filter(refs(&q), e), None, "{q:?} e = {e:?}");
                }
                // An invalid uncertainty anywhere decides nothing.
                for k in 0..3 {
                    for x in [-1.0, f64::NAN, f64::INFINITY] {
                        assert_eq!(stages::filter(refs(&q), on_axis(k, x)), None, "{q:?}");
                    }
                }
            }
        }
    }
    assert_eq!(zeros, 200 * 3 * 5);
}

#[test]
fn a_non_finite_coordinate_never_decides_even_on_a_flat_axis() {
    let bases: Vec<Points> = sample(random4(), 20)
        .into_iter()
        .chain(sample(grid4(), 20).into_iter().map(|p| flat(&p, 1, 2.0)))
        .collect();
    for p in bases {
        for point in 0..4 {
            for axis in 0..3 {
                for bad in [f64::NAN, -f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                    let mut q = p;
                    q[point][axis] = bad;
                    for e in [[0.0; 3], [0.25; 3]] {
                        assert_eq!(stages::filter(refs(&q), e), None, "{q:?} e = {e:?}");
                    }
                }
            }
        }
        // Every coordinate of a flat axis at infinity: its leaves are NaN.
        let q = flat(&p, 0, f64::INFINITY);
        assert_eq!(stages::filter(refs(&q), [0.0; 3]), None, "{q:?}");
    }
}
