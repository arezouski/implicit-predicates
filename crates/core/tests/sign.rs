//! Tests for the `Sign` type.

use indirect_predicates_core::Sign::{self, *};

#[test]
fn of_classifies_including_negative_zero() {
    assert_eq!(Sign::of(3.5), Positive);
    assert_eq!(Sign::of(-1e-300), Negative);
    assert_eq!(Sign::of(0.0), Zero);
    assert_eq!(Sign::of(-0.0), Zero);
    assert_eq!(Sign::of(f64::INFINITY), Positive);
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
