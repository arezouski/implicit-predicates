//! The filter of `orient3d` on approximations of exact points: whenever it
//! decides with the uncertainty `e` of the approximations, its sign is the
//! exact sign at the true points.
//!
//! These tests guard the term `KAPPA * E` of the error bound. Random offsets
//! rarely come close to the worst case, so the approximations are also
//! pushed to the corner of their error boxes that moves the determinant
//! furthest from its true value, following the signs of its exact partial
//! derivatives.
//!
//! To first order, an approximation within errors whose two largest add up
//! to `E` moves the determinant by at most `c * E * D^2`, where `D` is the
//! scale of the differences. The largest coefficient `c` attainable for this
//! predicate is 9, reached by the extremal coplanar configuration of
//! `the_extremal_coplanar_configuration_is_not_decided` (a search over
//! 400,000 variants of it found none larger), while the derived constant
//! `KAPPA` is 18. Tests can catch a `KAPPA` below 9, but no test can tell
//! apart values between 9 and 18: that interval is the slack of the
//! derivation, which bounds every cofactor separately.

mod common;

use common::boxes::{
    PERMUTATIONS, abs, at_least, determinant, determinant_at, exact_corners, exact_points,
    extremal, partials, pushed, pushed_with, scale_of, uncertainty,
};
use common::generated::exact;
use common::{Points, pow2, refs};
use implicit_predicates::Sign;
use implicit_predicates::stages::orient3d as stages;
use implicit_predicates_oracle::Dyadic;
use implicit_predicates_test_support::{config, coplanar4, grid4, near_coplanar4, random4, sample};
use proptest::prelude::*;

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

/// How the errors are spread over the four points.
#[derive(Clone, Copy, Debug)]
enum Spread {
    /// The same error on every point.
    Equal,
    /// The error on two points, none on the others.
    Two(usize, usize),
    /// The error on one point and `2^-t` times it on the others.
    OneLarge(usize, i32),
}

fn spread() -> impl Strategy<Value = Spread> {
    prop_oneof![
        Just(Spread::Equal),
        (0usize..4, 1usize..4).prop_map(|(i, k)| Spread::Two(i, (i + k) % 4)),
        (0usize..4, 1i32..=10).prop_map(|(i, t)| Spread::OneLarge(i, t)),
    ]
}

/// The error of each point for an error `error` spread as `spread`.
fn errors(error: f64, spread: Spread) -> [f64; 4] {
    match spread {
        Spread::Equal => [error; 4],
        Spread::Two(i, j) => std::array::from_fn(|k| if k == i || k == j { error } else { 0.0 }),
        Spread::OneLarge(i, t) => {
            std::array::from_fn(|k| if k == i { error } else { error * pow2(-t) })
        }
    }
}

/// Checks the filter on `approximation` with uncertainty `e` against the
/// exact sign at `truth`.
fn check(truth: &Points, approximation: &Points, e: f64) -> Result<(), TestCaseError> {
    let expected = exact(truth);
    let result = stages::filter(refs(approximation), e);
    if let Some(sign) = result {
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
        prop_assert_eq!(result, None, "{:?} e = {:e}", approximation, e);
    }
    Ok(())
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
        check(&truth, &approximation, uncertainty(errors))?;
    }

    #[test]
    fn the_filter_with_adversarial_uncertainty_is_never_wrong(
        base in coplanar4(),
        exponent in scale(),
        size in 1i32..=40,
        spread in spread(),
    ) {
        // Coplanar truths: the filter must never decide. Every error is a
        // power of two at most 51 bits below the largest coordinate, so
        // the moves are exact.
        let truth = scaled_to(&base, exponent);
        prop_assume!(top_exponent(&truth).is_some());
        let errors = errors(pow2(exponent - size), spread);
        let e = uncertainty(errors);
        for direction in [1, -1] {
            let approximation = pushed(&truth, errors, direction);
            prop_assert!(approximation.is_some(), "an inexact move of {:?}", truth);
            check(&truth, &approximation.unwrap(), e)?;
        }
    }

    #[test]
    fn a_truth_on_the_other_side_is_never_decided(
        base in coplanar4(),
        exponent in scale(),
        start in 0usize..12,
        step in 20i32..=40,
        factor in 1i32..=4,
    ) {
        // A truth just off the plane, on the side of `sign`, and
        // approximations pushed by errors `2^factor` times larger to the
        // other side.
        let coplanar = scaled_to(&base, exponent);
        // The first coordinate from `start` on whose partial derivative is
        // not zero; there is none only when the points are collinear.
        let d = partials(&coplanar);
        let moved = (start..start + 12)
            .map(|k| (k % 12 / 3, k % 3))
            .find(|&(i, axis)| d[i][axis].sign() != 0);
        prop_assume!(moved.is_some());
        let (point, axis) = moved.unwrap();
        let mut truth = coplanar;
        truth[point][axis] += pow2(exponent - step);
        let sign = exact(&truth);
        prop_assert_ne!(sign, Sign::Zero);
        let error = pow2(exponent - step + factor);
        let e = uncertainty([error; 4]);
        let direction = if sign == Sign::Positive { -1 } else { 1 };
        let approximation = pushed(&truth, [error; 4], direction);
        prop_assert!(approximation.is_some(), "an inexact move of {:?}", truth);
        let approximation = approximation.unwrap();
        prop_assert_eq!(exact(&approximation), -sign, "{:?}", approximation);
        prop_assert_eq!(
            stages::filter(refs(&approximation), e),
            None,
            "{:?} e = {:e}, truth {:?}",
            approximation,
            e,
            truth
        );
    }
}

#[test]
fn the_partial_derivatives_are_exact() {
    // The determinant is affine in each coordinate, so moving one by 1
    // changes it by the partial derivative there.
    for p in sample(grid4(), 200) {
        let p = scaled_to(&p, 4);
        let d = partials(&p);
        for i in 0..4 {
            for axis in 0..3 {
                let mut q = p;
                q[i][axis] += 1.0;
                assert!(&determinant(&q) - &determinant(&p) == d[i][axis], "{p:?}");
            }
        }
        assert_eq!(
            determinant(&p).sign(),
            match exact(&p) {
                Sign::Negative => -1,
                Sign::Zero => 0,
                Sign::Positive => 1,
            }
        );
    }
}

#[test]
fn the_coplanar_truth_pushed_to_its_box_corners_is_not_decided() {
    // Exactly coplanar points; every coordinate moved by 2^-30 towards the
    // corner of its box that follows the partial derivative, and `e` the
    // sum of two such errors. A `KAPPA` of 4 decides this case, wrongly.
    let truth = [
        [-2.0, 3.0, 0.0],
        [-2.0, -4.0, 5.0],
        [6.0, 2.0, -5.0],
        [6.0, -5.0, 0.0],
    ];
    assert_eq!(exact(&truth), Sign::Zero);
    let error = pow2(-30);
    for direction in [1, -1] {
        let approximation = pushed(&truth, [error; 4], direction).unwrap();
        assert_ne!(exact(&approximation), Sign::Zero);
        assert_eq!(
            stages::filter(refs(&approximation), 2.0 * error),
            None,
            "{approximation:?}"
        );
    }
}

#[test]
fn the_filter_decides_clear_cases_with_small_uncertainty() {
    // Random points on a grid of step 2^-12, each coordinate pushed by
    // 2^-40, exactly, in the worst direction: the uncertainty is tiny next
    // to the determinant, so the filter decides nearly always, and rightly.
    let cases: Vec<Points> = sample(random4(), 2000)
        .into_iter()
        .map(|p| p.map(|point| point.map(|x| (x * 4096.0).round() / 4096.0)))
        .collect();
    let error = pow2(-40);
    let mut decided = 0;
    for truth in &cases {
        let expected = exact(truth);
        for direction in [1, -1] {
            let approximation = pushed(truth, [error; 4], direction).unwrap();
            if let Some(sign) = stages::filter(refs(&approximation), 2.0 * error) {
                assert_eq!(sign, expected, "{approximation:?}");
                decided += 1;
            }
        }
    }
    assert!(
        decided * 10 > 2 * cases.len() * 9,
        "{decided} of {}",
        2 * cases.len()
    );
}

/// The extremal coplanar configuration, pushed to the corners of equal error
/// boxes, must never be decided: with `KAPPA` below 9 the filter would give
/// it a sign, which is wrong since the true points are coplanar.
///
/// The attainable first-order coefficient for this predicate is 9, reached
/// here; the derived `KAPPA` is 18, and no test can distinguish values
/// between 9 and 18, the slack of the derivation. The variants cover scales
/// over the whole range of the filter, with and without a power-of-two
/// scale, translations, the negation of any subset of the axes, every
/// permutation of the roles of `a`, `b` and `c` and of the axes, both push
/// directions, and errors from `2^-8` to `2^-40` of the scale.
#[test]
fn the_extremal_coplanar_configuration_is_not_decided() {
    let mut scales = Vec::new();
    for exponent in [-320, -100, -1, 0, 7, 150, 330] {
        for mantissa in [1.0, 3.0] {
            scales.push(mantissa * pow2(exponent));
        }
    }
    scales.push(5.0);
    let translations = [[0.0, 0.0, 0.0], [3.0, -5.0, 7.0], [256.0, -128.0, 1.0]];
    let mut reached = false;
    let mut cases = 0;
    for &s in &scales {
        for t in translations.map(|t| t.map(|x| x * s)) {
            for roles in PERMUTATIONS {
                for axes in PERMUTATIONS {
                    for negated in 0..8 {
                        let truth = extremal(s, t, roles, axes, negated);
                        assert_eq!(exact(&truth), Sign::Zero, "{truth:?}");
                        let d = partials(&truth);
                        for size in [8, 24, 40] {
                            let error = s * pow2(-size);
                            let e = uncertainty([error; 4]);
                            for direction in [1, -1] {
                                let approximation = pushed_with(&truth, &d, [error; 4], direction);
                                assert!(approximation.is_some(), "an inexact move of {truth:?}");
                                let approximation = approximation.unwrap();
                                assert_eq!(
                                    stages::filter(refs(&approximation), e),
                                    None,
                                    "{approximation:?} e = {e:e}, truth {truth:?}"
                                );
                                // Whether the approximation is more than
                                // `8.9 e D^2` from the plane.
                                let x = exact_points(&approximation);
                                let big_d = scale_of(&x, e);
                                let unit = &(&Dyadic::from_f64(e) * &big_d) * &big_d;
                                let far = &abs(&determinant_at(&x)) * &Dyadic::from_f64(10.0);
                                reached |= far > &unit * &Dyadic::from_f64(89.0);
                                cases += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(cases, scales.len() * 3 * 36 * 8 * 3 * 2);
    // The worst case reaches a coefficient above 8.9: these cases would be
    // decided, wrongly, with any `KAPPA` up to that.
    assert!(reached);
}

/// A subnormal `e`, with differences near `D_MIN` and far above it: when
/// the filter decides, its sign is the exact sign at every true point
/// tried within the boxes, and the uncertainty does not keep it from
/// deciding clear cases.
#[test]
fn a_subnormal_uncertainty_is_handled() {
    let low = stages::D_MIN.log2() as i32;
    // Random points first: at scale 1 they are clear cases.
    let random = sample(random4(), 40);
    let bases: Vec<Points> = random
        .iter()
        .copied()
        .chain(sample(near_coplanar4(), 40))
        .chain(sample(grid4(), 40))
        .collect();
    let tiny = pow2(-1074);
    let error_sets = [
        [tiny, tiny, 0.0, 0.0],
        [3.0 * tiny, 0.0, tiny, 0.0],
        [pow2(-1050); 4],
        [pow2(-1024); 4],
    ];
    let (mut decided, mut clear) = (0, 0);
    for (index, base) in bases.iter().enumerate() {
        for exponent in [low - 1, low + 1, low + 4, -300, 0, 100, 300] {
            let approximation = scaled_to(base, exponent);
            for errors in error_sets {
                let e = uncertainty(errors);
                assert!(e > 0.0 && e < f64::MIN_POSITIVE, "{e:e}");
                let Some(sign) = stages::filter(refs(&approximation), e) else {
                    continue;
                };
                decided += 1;
                clear += usize::from(exponent == 0 && index < random.len());
                for truth in exact_corners(&approximation, errors) {
                    let expected = match determinant_at(&truth).sign() {
                        -1 => Sign::Negative,
                        0 => Sign::Zero,
                        _ => Sign::Positive,
                    };
                    assert_eq!(sign, expected, "{approximation:?} e = {e:e}");
                }
            }
        }
    }
    // 40 random cases at scale 1, with four sets of errors each.
    assert!(clear >= 150, "{clear} of 160 clear cases decided");
    assert!(decided > clear, "{decided} cases decided");
}
