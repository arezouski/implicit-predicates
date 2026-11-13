//! Big integers built from `f64` values.

use implicit_predicates_core::{Big, Scratch, ScratchExhausted, Sign};
use implicit_predicates_oracle::Dyadic;
use implicit_predicates_test_support::{any_finite, assert_valid_big, big_to_dyadic, config};
use proptest::prelude::*;

fn oracle_sign(d: &Dyadic) -> Sign {
    match d.sign() {
        -1 => Sign::Negative,
        0 => Sign::Zero,
        _ => Sign::Positive,
    }
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn a_big_from_an_f64_has_its_value(x in any_finite()) {
        let mut buf = [0.0; 4];
        let mut s = Scratch::borrowed(&mut buf);
        let b = s.big_from_f64(x).unwrap();
        assert_valid_big(&s, b);
        let exact = Dyadic::from_f64(x);
        prop_assert_eq!(big_to_dyadic(&s, b), exact.clone());
        prop_assert_eq!(b.sign(), oracle_sign(&exact));
    }

    #[test]
    fn a_value_takes_at_most_two_words(x in any_finite()) {
        let mut buf = [0.0; 4];
        let mut s = Scratch::borrowed(&mut buf);
        prop_assert!(s.big_from_f64(x).unwrap().len() <= 2);
    }

    #[test]
    fn the_negation_changes_only_the_sign(x in any_finite()) {
        let mut buf = [0.0; 4];
        let mut s = Scratch::borrowed(&mut buf);
        let b = s.big_from_f64(x).unwrap();
        let n = b.neg();
        assert_valid_big(&s, n);
        prop_assert_eq!(n.len(), b.len());
        prop_assert_eq!(n.exponent(), b.exponent());
        prop_assert_eq!(big_to_dyadic(&s, n), -&big_to_dyadic(&s, b));
        prop_assert_eq!(n.neg().sign(), b.sign());
    }
}

#[test]
fn the_edges_of_f64_convert() {
    let largest_subnormal = f64::from_bits((1u64 << 52) - 1);
    let edges = [
        0.0,
        -0.0,
        5e-324,
        -5e-324,
        f64::MIN_POSITIVE,
        largest_subnormal,
        f64::MAX,
        -f64::MAX,
        1.0,
        2f64.powi(52),
        2f64.powi(53) - 1.0,
        2f64.powi(64),
        2f64.powi(-64),
        2f64.powi(63),
        1.5 * 2f64.powi(63),
        (2f64.powi(53) - 1.0) * 2f64.powi(60),
    ];
    for x in edges {
        let mut buf = [0.0; 4];
        let mut s = Scratch::borrowed(&mut buf);
        let b = s.big_from_f64(x).unwrap();
        assert_valid_big(&s, b);
        assert_eq!(big_to_dyadic(&s, b), Dyadic::from_f64(x), "{x:e}");
        if x == 0.0 {
            assert!(b.is_zero());
            assert_eq!(b.sign(), Sign::Zero);
        }
    }
}

#[test]
fn a_mantissa_straddling_two_words_takes_both() {
    let mut buf = [0.0; 4];
    let mut s = Scratch::borrowed(&mut buf);
    // (2^53 - 1) * 2^60 has 53 set bits starting at bit 60.
    let b = s
        .big_from_f64((2f64.powi(53) - 1.0) * 2f64.powi(60))
        .unwrap();
    assert_eq!(b.len(), 2);
    assert_eq!(s.big_limb(b, 0), u64::MAX << 60);
    assert_eq!(s.big_limb(b, 1), (1 << 49) - 1);
}

#[test]
#[should_panic(expected = "not finite")]
fn a_nan_panics() {
    let mut buf = [0.0; 4];
    let _ = Scratch::borrowed(&mut buf).big_from_f64(f64::NAN);
}

#[test]
#[should_panic(expected = "not finite")]
fn an_infinity_panics() {
    let mut buf = [0.0; 4];
    let _ = Scratch::borrowed(&mut buf).big_from_f64(f64::INFINITY);
}

#[test]
fn an_arena_without_room_is_left_as_it_was() {
    let mut buf = [0.0; 1];
    let mut s = Scratch::borrowed(&mut buf);
    // This value needs two words.
    let r = s.big_from_f64((2f64.powi(53) - 1.0) * 2f64.powi(60));
    assert_eq!(r.unwrap_err(), ScratchExhausted);
    assert_eq!(s.used(), 0);
}

#[test]
fn the_negation_of_zero_is_zero() {
    assert_eq!(Big::ZERO.neg().sign(), Sign::Zero);
    let mut buf = [0.0; 4];
    let mut s = Scratch::borrowed(&mut buf);
    let z = s.big_from_f64(-0.0).unwrap();
    assert_eq!(z.neg().sign(), Sign::Zero);
    assert_valid_big(&s, z.neg());
}

#[test]
fn words_are_normalized_when_stored() {
    let mut buf = [0.0; 8];
    let mut s = Scratch::borrowed(&mut buf);
    let b = s.big_from_words(true, &[0, 0, 5, 0, 0], 3).unwrap();
    assert_eq!((b.len(), b.exponent(), b.sign()), (1, 5, Sign::Negative));
    let z = s.big_from_words(true, &[0, 0], 9).unwrap();
    assert!(z.is_zero());
    assert_eq!((z.exponent(), z.sign()), (0, Sign::Zero));
    assert_eq!(s.used(), 1);
}
