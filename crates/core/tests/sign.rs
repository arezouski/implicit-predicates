//! Tests for the `Sign` type.

use core::cmp::Ordering;

use implicit_predicates_core::Sign::{self, *};

#[test]
fn of_classifies_including_negative_zero() {
    assert_eq!(Sign::of(3.5), Positive);
    assert_eq!(Sign::of(-1e-300), Negative);
    assert_eq!(Sign::of(0.0), Zero);
    assert_eq!(Sign::of(-0.0), Zero);
    assert_eq!(Sign::of(f64::INFINITY), Positive);
    assert_eq!(Sign::of(f64::NEG_INFINITY), Negative);
}

#[test]
fn of_classifies_the_smallest_magnitudes() {
    assert_eq!(Sign::of(5e-324), Positive);
    assert_eq!(Sign::of(-5e-324), Negative);
    assert_eq!(Sign::of(f64::MIN_POSITIVE), Positive);
    assert_eq!(Sign::of(-f64::MIN_POSITIVE / 4.0), Negative);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "NaN")]
fn of_nan_panics_in_debug_builds() {
    let _ = Sign::of(f64::NAN);
}

#[test]
fn neg_and_mul_follow_integer_signs() {
    for a in [Negative, Zero, Positive] {
        assert_eq!((-a) as i8, -(a as i8));
        for b in [Negative, Zero, Positive] {
            assert_eq!((a * b) as i8, (a as i8) * (b as i8));
        }
    }
}

#[test]
fn order_and_conversions_follow_integer_signs() {
    assert!(Negative < Zero && Zero < Positive);
    for a in [Negative, Zero, Positive] {
        assert_eq!(i8::from(a), a as i8);
        assert_eq!(Sign::from(i8::from(a).cmp(&0)), a);
        for b in [Negative, Zero, Positive] {
            assert_eq!(a.cmp(&b), i8::from(a).cmp(&i8::from(b)));
        }
    }
    assert_eq!(Sign::from(Ordering::Less), Negative);
    assert_eq!(Sign::from(Ordering::Equal), Zero);
    assert_eq!(Sign::from(Ordering::Greater), Positive);
}
