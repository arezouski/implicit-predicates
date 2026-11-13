//! The chain of `orient3d` ends with the stage on big integers, so it
//! decides every finite input: the calls that no stage on expansions can
//! decide get the sign of the oracle, and an input that is not finite, or a
//! point whose exact coordinates have a weight other than one, is a panic.

// `orient3d` needs the `std` feature.
#![cfg(feature = "std")]

mod common;

use common::args::{Arg, Args, Orient3d, Orient3dIn, exact_sign, with_concrete};
use implicit_predicates::stages::orient3d::BIG_INTEGERS_EXPLICIT_ARENA;
use implicit_predicates::{
    BigCoords, ExactCoords, ExactError, Lnc, Point, Scratch, Sign, Weight, orient3d, orient3d_in,
};

/// Three points of a plane, for a fourth point to be tested against.
const PLANE: [[f64; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// Each of these calls panics because an input is not finite.
mod an_input_that_is_not_finite_panics {
    use super::*;

    #[test]
    #[should_panic(expected = "not finite")]
    fn with_a_nan_in_an_array() {
        let [a, b, c] = PLANE;
        orient3d(&a, &b, &c, &[f64::NAN, 0.0, 0.0]);
    }

    #[test]
    #[should_panic(expected = "not finite")]
    fn with_an_infinity_in_an_array() {
        let [a, b, c] = PLANE;
        orient3d(&a, &b, &c, &[0.0, f64::INFINITY, 0.0]);
    }

    #[test]
    #[should_panic(expected = "not finite")]
    fn with_a_point_on_a_line_whose_parameter_is_nan() {
        let [a, b, c] = PLANE;
        orient3d(&a, &b, &c, &Lnc::new_unchecked(a, b, f64::NAN));
    }

    #[test]
    #[should_panic(expected = "not finite")]
    fn with_an_arena() {
        let [a, b, c] = PLANE;
        let mut buf = vec![0.0; 1 << 16];
        let _ = orient3d_in(
            &mut Scratch::borrowed(&mut buf),
            &a,
            &b,
            &c,
            &[0.0, 0.0, f64::NEG_INFINITY],
        );
    }
}

/// A point at the origin whose exact coordinates have the weight
/// `Weight::Expansion`, with an infinite error so that the filter never
/// decides.
struct Weighted;

impl Point for Weighted {
    fn approx(&self) -> &[f64; 3] {
        &[0.0; 3]
    }

    fn err(&self) -> &[f64; 3] {
        &[f64::INFINITY; 3]
    }

    fn is_explicit(&self) -> bool {
        false
    }

    fn exact(&self, scratch: &mut Scratch<'_>) -> Result<ExactCoords, ExactError> {
        let zero = scratch.push(&[])?;
        let one = scratch.push(&[1.0])?;
        Ok(ExactCoords {
            x: zero,
            y: zero,
            z: zero,
            w: Weight::Expansion(one),
        })
    }

    fn exact_big(&self, scratch: &mut Scratch<'_>) -> Result<BigCoords, ExactError> {
        [0.0; 3].exact_big(scratch)
    }
}

#[test]
#[should_panic(
    expected = "implicit-predicates: a point has a weight other than one, which the predicates do not support"
)]
fn a_point_with_a_weight_panics() {
    let [a, b, c] = PLANE;
    orient3d(&a, &b, &c, &Weighted);
}

#[test]
#[should_panic(
    expected = "implicit-predicates: a point has a weight other than one, which the predicates do not support"
)]
fn a_point_with_a_weight_panics_with_an_arena() {
    let [a, b, c] = PLANE;
    let mut buf = vec![0.0; 1 << 12];
    let _ = orient3d_in(&mut Scratch::borrowed(&mut buf), &a, &b, &c, &Weighted);
}

/// A point at the origin without exact coordinates, as expansions or as big
/// integers, with an infinite error so that the filter never decides.
struct NoCoordinates;

impl Point for NoCoordinates {
    fn approx(&self) -> &[f64; 3] {
        &[0.0; 3]
    }

    fn err(&self) -> &[f64; 3] {
        &[f64::INFINITY; 3]
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

#[test]
#[should_panic(
    expected = "implicit-predicates: a point has no exact coordinates: an input is not finite"
)]
fn a_point_without_big_coordinates_panics() {
    let [a, b, c] = PLANE;
    orient3d(&a, &b, &c, &NoCoordinates);
}

#[test]
#[should_panic(
    expected = "implicit-predicates: a point has no exact coordinates: an input is not finite"
)]
fn a_point_without_big_coordinates_panics_with_an_arena() {
    let [a, b, c] = PLANE;
    let mut buf = vec![0.0; 1 << 12];
    let _ = orient3d_in(&mut Scratch::borrowed(&mut buf), &a, &b, &c, &NoCoordinates);
}

/// The calls that the documentation of `orient3d` showed giving `None`,
/// outside the range of the stages on expansions, and one with explicit
/// points near the ends of `f64`, with the sign that its examples now show:
/// the sign of the oracle.
#[test]
fn the_examples_that_gave_none_have_the_sign_of_the_oracle() {
    let (a, c) = ([1.0, 0.25, 0.5], [0.3, 0.7, 5.0]);
    let b1 = [2f64.powi(-140), 3.0, 0.75];
    let b2 = [0.1, 3.0, 0.75];
    let (p, q, r) = ([1.0, -1.0, 0.0], [0.0, 1.0, -1.0], [1.0, 0.0, -1.0]);
    let e = Arg::Explicit;
    let cases: [(Args, Sign); 4] = [
        ([e(a), e(b1), e(c), Arg::Lnc(a, b1, 0.1)], Sign::Zero),
        (
            [e(a), e(b2), e(c), Arg::Lnc(a, b2, 2f64.powi(-200))],
            Sign::Zero,
        ),
        (
            [
                e(p),
                e(q),
                e(r),
                Arg::Lnc([0.0; 3], [0.0; 3], 2f64.powi(-600)),
            ],
            Sign::Zero,
        ),
        (
            [
                e([0.0; 3]),
                e([1e300, 0.0, 0.0]),
                e([0.0, 1e300, 0.0]),
                e([1e300, -1e300, 5e-324]),
            ],
            Sign::Negative,
        ),
    ];
    for (args, documented) in cases {
        let expected = exact_sign(&args);
        assert_eq!(expected, documented, "{args:?}");
        // The point on a line is valid: `Lnc::new` accepts it.
        for arg in args {
            if let Arg::Lnc(p, q, t) = arg {
                Lnc::new(p, q, t).unwrap();
            }
        }
        for raw in [false, true] {
            assert_eq!(
                with_concrete(&args, raw, &mut Orient3d),
                expected,
                "{args:?}"
            );
            assert_eq!(
                with_concrete(&args, raw, &mut Orient3dIn),
                expected,
                "{args:?}"
            );
        }
    }
}

/// The documentation of `orient3d_in` and the changelog give the arena that
/// suffices for explicit points.
#[test]
fn the_documented_arena_for_explicit_points_is_the_constant() {
    let words = BIG_INTEGERS_EXPLICIT_ARENA;
    let documented = format!("{},{:03} `f64`", words / 1000, words % 1000);
    for (file, text) in [
        ("lib.rs", include_str!("../src/lib.rs")),
        ("CHANGELOG.md", include_str!("../../../CHANGELOG.md")),
    ] {
        assert!(text.contains(&documented), "{file}: {documented}");
    }
    // No README quotes it, so that no other copy can drift.
    for (file, text) in [
        ("README.md", include_str!("../../../README.md")),
        (
            "crates/implicit-predicates/README.md",
            include_str!("../README.md"),
        ),
    ] {
        assert!(!text.contains(&format!("{words}")), "{file}");
        assert!(!text.contains(&documented), "{file}");
    }
}
