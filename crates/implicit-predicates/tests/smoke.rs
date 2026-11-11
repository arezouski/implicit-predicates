//! Basic cases of `orient3d` whose sign is known by hand.

use implicit_predicates::stages::orient3d as stages;
use implicit_predicates::{Sign, orient3d};

type Stage = fn([&[f64; 3]; 4]) -> Option<Sign>;

/// Every stage of the chain, with its name; the filter for explicit points.
const STAGES: [(&str, Stage); 5] = [
    ("filter", |p| stages::filter(p, 0.0)),
    ("exact_differences", stages::exact_differences),
    ("expansions_translated", stages::expansions_translated),
    ("expansions_untranslated", stages::expansions_untranslated),
    ("staged", stages::staged),
];

#[test]
fn orient3d_of_a_unit_tetrahedron() {
    // det[a - d; b - d; c - d] is the determinant of the identity.
    let (a, b, c, d) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0; 3]);
    assert_eq!(orient3d(&a, &b, &c, &d), Some(Sign::Positive));
    // d above the plane through a, b and c: they are seen counter-clockwise
    // from d.
    assert_eq!(orient3d(&a, &b, &c, &[1.0, 1.0, 1.0]), Some(Sign::Negative));
}

#[test]
fn swapping_two_points_negates() {
    let (a, b, c, d) = (
        [0.5, -1.25, 3.0],
        [2.0, 0.75, -0.5],
        [-1.5, 4.0, 1.0],
        [0.25, 0.5, -2.0],
    );
    let s = orient3d(&a, &b, &c, &d).unwrap();
    assert_ne!(s, Sign::Zero);
    assert_eq!(orient3d(&b, &a, &c, &d), Some(-s));
    assert_eq!(orient3d(&a, &c, &b, &d), Some(-s));
    assert_eq!(orient3d(&d, &b, &c, &a), Some(-s));
    assert_eq!(orient3d(&b, &c, &a, &d), Some(s));
}

#[test]
fn coplanar_points_are_zero() {
    // All four on the plane x + y + z = 1; the filter cannot decide a zero.
    let a = [0.125, 0.375, 0.5];
    let b = [0.5, 0.25, 0.25];
    let c = [1.0, -0.5, 0.5];
    let d = [0.25, 0.25, 0.5];
    assert_eq!(orient3d(&a, &b, &c, &d), Some(Sign::Zero));
    // A repeated point.
    assert_eq!(orient3d(&a, &a, &c, &d), Some(Sign::Zero));
    // Collinear points.
    assert_eq!(
        orient3d(
            &[0.0; 3],
            &[1.0, 2.0, 3.0],
            &[2.0, 4.0, 6.0],
            &[7.0, -1.0, 0.5]
        ),
        Some(Sign::Zero)
    );
}

#[test]
fn each_stage_agrees_on_a_grid_case() {
    // Small integers: every stage can decide, and they must agree.
    let a = [1.0, 2.0, 3.0];
    let b = [-2.0, 1.0, 4.0];
    let c = [3.0, -1.0, 2.0];
    let d = [0.0, 1.0, -1.0];
    // det[a - d; b - d; c - d] = det[[1, 1, 4], [-2, 0, 5], [3, -2, 3]] = 47.
    for (name, stage) in STAGES {
        assert_eq!(stage([&a, &b, &c, &d]), Some(Sign::Positive), "{name}");
    }
    // d = b + c - a lies on the plane through a, b and c: every exact stage
    // gives zero, and the filter abstains.
    let d = [0.0, -2.0, 3.0];
    assert_eq!(stages::filter([&a, &b, &c, &d], 0.0), None);
    for (name, stage) in STAGES.into_iter().skip(1) {
        assert_eq!(stage([&a, &b, &c, &d]), Some(Sign::Zero), "{name}");
    }
}

#[test]
fn non_finite_input_is_none() {
    let points = [
        [1.0, 2.0, 3.0],
        [-2.0, 1.0, 4.0],
        [3.0, -1.0, 2.0],
        [0.0, 1.0, -1.0],
    ];
    for point in 0..4 {
        for axis in 0..3 {
            for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                let mut p = points;
                p[point][axis] = bad;
                for (name, stage) in STAGES {
                    assert_eq!(
                        stage([&p[0], &p[1], &p[2], &p[3]]),
                        None,
                        "{name} with {bad} at point {point}, axis {axis}"
                    );
                }
                assert_eq!(orient3d(&p[0], &p[1], &p[2], &p[3]), None);
            }
        }
    }
    // A non-finite error bound makes the filter abstain.
    let [a, b, c, d] = &points;
    for e in [f64::NAN, f64::INFINITY] {
        assert_eq!(stages::filter([a, b, c, d], e), None);
    }
}

#[test]
fn the_documented_range_is_the_generated_one() {
    // The documentation of `orient3d` promises a sign for every nonzero
    // magnitude in [2^-188, 2^239]: the range of its last stage.
    assert_eq!(
        stages::UNTRANSLATED_RANGE,
        (2f64.powi(-188), 2f64.powi(239))
    );
    let (min, max) = stages::UNTRANSLATED_RANGE;
    let a = [min, 0.0, 0.0];
    let b = [0.0, max, 0.0];
    let c = [0.0, 0.0, -min];
    let d = [max, max, max];
    assert!(orient3d(&a, &b, &c, &d).is_some());
}
