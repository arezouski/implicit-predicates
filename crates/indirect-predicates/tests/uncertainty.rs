//! The filter of `orient3d` on approximations of exact points: whenever it
//! decides with the uncertainty `e` of the approximations, its sign is the
//! exact sign at the true points.
//!
//! These tests guard the term `KAPPA * E` of the error bound. Random offsets
//! rarely come close to the worst case, so the approximations are also
//! pushed to the corner of their error boxes that moves the determinant
//! furthest from its true value, following the signs of its exact partial
//! derivatives.

mod common;

use common::generated::exact;
use common::{Points, pow2, refs};
use indirect_predicates::Sign;
use indirect_predicates::stages::orient3d as stages;
use indirect_predicates_oracle::Dyadic;
use indirect_predicates_test_support::{config, coplanar4, grid4, near_coplanar4, random4, sample};
use proptest::prelude::*;

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

/// `e` as a caller computes it: the sum of the two largest errors, rounded
/// up.
fn uncertainty(errors: [f64; 4]) -> f64 {
    let mut errors = errors;
    errors.sort_by(|a, b| b.total_cmp(a));
    at_least(
        errors[0] + errors[1],
        &(&Dyadic::from_f64(errors[0]) + &Dyadic::from_f64(errors[1])),
    )
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

/// The exact partial derivatives of det[a - d; b - d; c - d] with respect
/// to every coordinate of every point, at `p`: the cofactors of the rows
/// `a - d`, `b - d`, `c - d`, and minus their sum for `d`.
fn partials(p: &Points) -> [[Dyadic; 3]; 4] {
    let x = |i: usize, axis: usize| Dyadic::from_f64(p[i][axis]);
    let m: [[Dyadic; 3]; 3] = std::array::from_fn(|i| std::array::from_fn(|j| &x(i, j) - &x(3, j)));
    let cofactor = |i: usize, j: usize| {
        let (i1, i2, j1, j2) = ((i + 1) % 3, (i + 2) % 3, (j + 1) % 3, (j + 2) % 3);
        &(&m[i1][j1] * &m[i2][j2]) - &(&m[i1][j2] * &m[i2][j1])
    };
    let rows: [[Dyadic; 3]; 3] = std::array::from_fn(|i| std::array::from_fn(|j| cofactor(i, j)));
    let last = std::array::from_fn(|j| -&(&(&rows[0][j] + &rows[1][j]) + &rows[2][j]));
    [rows[0].clone(), rows[1].clone(), rows[2].clone(), last]
}

/// The exact determinant det[a - d; b - d; c - d].
fn determinant(p: &Points) -> Dyadic {
    // Expanded along the first row with the cofactors of `partials`.
    let d = partials(p);
    (0..3)
        .map(|j| &(&Dyadic::from_f64(p[0][j]) - &Dyadic::from_f64(p[3][j])) * &d[0][j])
        .fold(Dyadic::from_f64(0.0), |sum, term| &sum + &term)
}

/// `truth` with every coordinate of point `i` moved by `errors[i]`, up or
/// down by the sign of the partial derivative there times `direction`: the
/// corner of the error boxes that moves the determinant furthest in that
/// direction, to first order. Returns `None` unless every move is exact.
fn pushed(truth: &Points, errors: [f64; 4], direction: i8) -> Option<Points> {
    let d = partials(truth);
    let mut p = *truth;
    for i in 0..4 {
        for axis in 0..3 {
            let s = match d[i][axis].sign() * direction {
                0 => direction,
                s => s,
            };
            let offset = f64::from(s) * errors[i];
            p[i][axis] = truth[i][axis] + offset;
            let moved = &Dyadic::from_f64(truth[i][axis]) + &Dyadic::from_f64(offset);
            if Dyadic::from_f64(p[i][axis]) != moved {
                return None;
            }
        }
    }
    Some(p)
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
