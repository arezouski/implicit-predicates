//! The point contract as `[f64; 3]` implements it: an explicit point whose
//! exact coordinates are its own components.

use implicit_predicates_core::{ExactCoords, ExactError, Point, Scratch, ScratchExhausted, Weight};
use implicit_predicates_test_support::{any_finite, config};
use proptest::prelude::*;

/// The components of the three coordinates `exact` wrote.
fn exact_components(scratch: &Scratch<'_>, coords: ExactCoords) -> [Vec<f64>; 3] {
    [coords.x, coords.y, coords.z].map(|s| scratch.get(s).to_vec())
}

/// A coordinate as the expansion `exact` must write: one component, or none
/// for zero.
fn as_expansion(c: f64) -> Vec<f64> {
    if c == 0.0 { vec![] } else { vec![c] }
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn arrays_are_explicit_points(p in prop::array::uniform3(any_finite())) {
        prop_assert!(core::ptr::eq(Point::approx(&p), &p));
        prop_assert_eq!(p.err(), &[0.0; 3]);
        prop_assert!(p.is_explicit());

        let mut buf = [f64::NAN; 3];
        let mut s = Scratch::borrowed(&mut buf);
        let coords = p.exact(&mut s).unwrap();
        prop_assert_eq!(coords.w, Weight::One);
        prop_assert_eq!(exact_components(&s, coords), p.map(as_expansion));
        prop_assert_eq!(s.used(), p.iter().filter(|&&c| c != 0.0).count());
    }
}

#[test]
fn zero_coordinates_are_empty_spans() {
    let mut buf = [0.0; 3];
    let mut s = Scratch::borrowed(&mut buf);
    let coords = [0.0, -0.0, 2.5].exact(&mut s).unwrap();
    assert!(coords.x.is_empty());
    assert!(coords.y.is_empty());
    assert_eq!(s.get(coords.z), &[2.5]);
    assert_eq!(s.used(), 1);
}

#[test]
fn non_finite_coordinates_are_out_of_range() {
    for bad in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        for axis in 0..3 {
            let mut p = [1.0, 2.0, 3.0];
            p[axis] = bad;
            let mut buf = [0.0; 3];
            let mut s = Scratch::borrowed(&mut buf);
            assert_eq!(p.exact(&mut s), Err(ExactError::OutOfRange), "{p:?}");
            assert_eq!(s.used(), 0, "{p:?} left components in the arena");
        }
    }
}

#[test]
fn exhaustion_is_reported_and_leaves_the_arena_unchanged() {
    let mut buf = [0.0; 3];
    let mut s = Scratch::borrowed(&mut buf);
    let a = s.push(&[4.0]).unwrap();
    let result = [1.0, 2.0, 3.0].exact(&mut s);
    assert_eq!(result, Err(ExactError::Exhausted(ScratchExhausted)));
    assert_eq!(s.used(), 1);
    assert_eq!(s.get(a), &[4.0]);
}

#[test]
fn exact_errors_convert_and_describe_themselves() {
    use core::error::Error;
    let exhausted = ExactError::from(ScratchExhausted);
    assert_eq!(exhausted, ExactError::Exhausted(ScratchExhausted));
    assert_eq!(exhausted.to_string(), "scratch arena exhausted");
    assert!(exhausted.source().is_some());
    assert_eq!(
        ExactError::OutOfRange.to_string(),
        "point outside the range of the exact arithmetic"
    );
    assert!(ExactError::OutOfRange.source().is_none());
    assert_eq!(
        ExactError::Weighted.to_string(),
        "point with a weight other than one, which big-integer coordinates cannot carry"
    );
    assert!(ExactError::Weighted.source().is_none());
    assert_ne!(ExactError::Weighted, ExactError::OutOfRange);
}

#[test]
fn an_explicit_point_gives_its_coordinates_as_big_integers() {
    use implicit_predicates_oracle::Dyadic;
    use implicit_predicates_test_support::{assert_valid_big, big_to_dyadic};

    let points = [
        [1.5, 0.0, -2.0],
        [f64::MAX, -f64::MAX, f64::MIN_POSITIVE],
        [5e-324, -5e-324, 1e-310],
        [0.1, -0.0, 1e300],
    ];
    for p in points {
        let mut buf = [0.0; 12];
        let mut s = Scratch::borrowed(&mut buf);
        let coords = p.exact_big(&mut s).unwrap();
        for (big, c) in [coords.x, coords.y, coords.z].into_iter().zip(p) {
            assert_valid_big(&s, big);
            assert_eq!(big_to_dyadic(&s, big), Dyadic::from_f64(c), "{p:?}");
        }
    }
}

#[test]
fn a_point_that_is_not_finite_has_no_big_coordinates() {
    for bad in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        for axis in 0..3 {
            let mut p = [1.0, 2.0, 3.0];
            p[axis] = bad;
            let mut buf = [0.0; 12];
            let mut s = Scratch::borrowed(&mut buf);
            let kept = s.push(&[4.0]).unwrap();
            assert_eq!(p.exact_big(&mut s).unwrap_err(), ExactError::OutOfRange);
            assert_eq!(s.used(), 1, "{p:?} left components in the arena");
            assert_eq!(s.get(kept), &[4.0]);
        }
    }
}

#[test]
fn a_small_arena_gives_no_big_coordinates_and_reports_exhaustion() {
    let p = [1.5, 2.5, 3.5];
    let mut buf = [0.0; 2];
    let mut s = Scratch::borrowed(&mut buf);
    let result = p.exact_big(&mut s);
    assert_eq!(result.unwrap_err(), ExactError::Exhausted(ScratchExhausted));
}
