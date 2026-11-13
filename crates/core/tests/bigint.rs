//! Big integers built from `f64` values.

use implicit_predicates_core::{Big, Scratch, ScratchExhausted, Sign};
use implicit_predicates_oracle::Dyadic;
use implicit_predicates_test_support::{
    any_big, any_finite, assert_valid_big, big_to_dyadic, config,
};
use proptest::prelude::*;

fn oracle_sign(d: &Dyadic) -> Sign {
    match d.sign() {
        -1 => Sign::Negative,
        0 => Sign::Zero,
        _ => Sign::Positive,
    }
}

fn check_sum_and_difference(s: &mut Scratch<'_>, a: Big, b: Big) -> Result<(), TestCaseError> {
    let (da, db) = (big_to_dyadic(s, a), big_to_dyadic(s, b));
    let sum = s.big_add(a, b).unwrap();
    assert_valid_big(s, sum);
    prop_assert_eq!(big_to_dyadic(s, sum), &da + &db);
    let diff = s.big_sub(a, b).unwrap();
    assert_valid_big(s, diff);
    prop_assert_eq!(big_to_dyadic(s, diff), &da - &db);
    // The operands are untouched.
    prop_assert_eq!(big_to_dyadic(s, a), da);
    prop_assert_eq!(big_to_dyadic(s, b), db);
    Ok(())
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
    fn sums_and_differences_have_their_value(
        x in any_finite(),
        y in any_finite(),
        p in any_big(),
        q in any_big(),
    ) {
        let mut buf = [0.0; 400];
        let mut s = Scratch::borrowed(&mut buf);
        let a = s.big_from_f64(x).unwrap();
        let b = s.big_from_f64(y).unwrap();
        check_sum_and_difference(&mut s, a, b)?;
        let mut s = Scratch::borrowed(&mut buf);
        let a = p.build(&mut s).unwrap();
        let b = q.build(&mut s).unwrap();
        check_sum_and_difference(&mut s, a, b)?;
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
            assert!(!b.is_negative());
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
    assert!(!Big::ZERO.neg().is_negative());
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
    assert_eq!((z.exponent(), z.is_negative()), (0, false));
    assert_eq!(s.used(), 1);
    // Zero words between nonzero ones are kept.
    let m = s.big_from_words(false, &[0, 5, 0, 7, 0], 4).unwrap();
    assert_eq!((m.len(), m.exponent()), (3, 5));
    let limbs: Vec<u64> = (0..3).map(|i| s.big_limb(m, i)).collect();
    assert_eq!(limbs, [5, 0, 7]);
}

fn pow2(s: &mut Scratch<'_>, bit: u32) -> Big {
    let mut words = vec![0u64; (bit / 64) as usize + 1];
    words[(bit / 64) as usize] = 1 << (bit % 64);
    s.big_from_words(false, &words, 0).unwrap()
}

fn value_of(s: &Scratch<'_>, b: Big) -> Dyadic {
    assert_valid_big(s, b);
    big_to_dyadic(s, b)
}

#[test]
fn a_total_cancellation_is_zero() {
    let mut buf = [0.0; 64];
    let mut s = Scratch::borrowed(&mut buf);
    for x in [1.0, -3.5, 1e300, 5e-324, 2f64.powi(64)] {
        let b = s.big_from_f64(x).unwrap();
        let used = s.used();
        for z in [s.big_sub(b, b).unwrap(), s.big_add(b, b.neg()).unwrap()] {
            assert!(z.is_zero());
            assert_eq!((z.len(), z.exponent(), z.is_negative()), (0, 0, false));
            assert_eq!(s.used(), used, "nothing is left written");
        }
    }
    let p = pow2(&mut s, 200);
    let used = s.used();
    assert!(s.big_sub(p, p).unwrap().is_zero());
    assert_eq!(s.used(), used);
}

#[test]
fn a_partial_cancellation_is_normalized() {
    let mut buf = [0.0; 128];
    let mut s = Scratch::borrowed(&mut buf);
    let one = pow2(&mut s, 0);
    let p200 = pow2(&mut s, 200);
    let p64 = pow2(&mut s, 64);
    let p130 = pow2(&mut s, 130);

    let big = s.big_add(p200, one).unwrap();
    let d = s.big_sub(big, p200).unwrap();
    assert_eq!(value_of(&s, d), Dyadic::from_words(false, &[1], 0));
    assert_eq!((d.len(), d.exponent()), (1, 0));
    // Nothing but the result stays above the operands.
    let used = s.used();
    let before = s.big_sub(big, p200).unwrap();
    assert_eq!(s.used(), used + before.len());

    let x = s.big_add(p64, p130).unwrap();
    let d = s.big_sub(x, p130).unwrap();
    assert_eq!(value_of(&s, d), Dyadic::from_words(false, &[1], 64));
    assert_eq!((d.len(), d.exponent()), (1, 1));

    // 2^64 - 1 borrows across a word.
    let d = s.big_sub(p64, one).unwrap();
    assert_eq!(value_of(&s, d), Dyadic::from_words(false, &[u64::MAX], 0));
    assert_eq!(d.len(), 1);

    // (2^64 - 1) + 1 carries into a new word.
    let m = s.big_sub(p64, one).unwrap();
    let c = s.big_add(m, one).unwrap();
    assert_eq!(value_of(&s, c), Dyadic::from_words(false, &[1], 64));
    assert_eq!((c.len(), c.exponent()), (1, 1));

    // A negative result takes the sign of the larger magnitude.
    let n = s.big_sub(one, p64).unwrap();
    assert_eq!(n.sign(), Sign::Negative);
    assert_eq!(value_of(&s, n), Dyadic::from_words(true, &[u64::MAX], 0));
}

#[test]
fn distant_exponents_align() {
    let mut buf = [0.0; 128];
    let mut s = Scratch::borrowed(&mut buf);
    let tiny = s.big_from_f64(5e-324).unwrap();
    let huge = s.big_from_f64(2f64.powi(1023)).unwrap();
    let (dt, dh) = (Dyadic::from_f64(5e-324), Dyadic::from_f64(2f64.powi(1023)));
    let sum = s.big_add(tiny, huge).unwrap();
    assert_eq!(value_of(&s, sum), &dt + &dh);
    assert!(sum.len() <= 34);
    let diff = s.big_sub(huge, tiny).unwrap();
    assert_eq!(value_of(&s, diff), &dh - &dt);
    assert!(diff.len() <= 34);
    let neg = s.big_sub(tiny, huge).unwrap();
    assert_eq!(value_of(&s, neg), &dt - &dh);
}

#[test]
fn zero_operands_return_the_other_without_writing() {
    let mut buf = [0.0; 8];
    let mut s = Scratch::borrowed(&mut buf);
    let b = s.big_from_f64(-6.0).unwrap();
    let used = s.used();
    for r in [
        s.big_add(Big::ZERO, b).unwrap(),
        s.big_add(b, Big::ZERO).unwrap(),
        s.big_sub(b, Big::ZERO).unwrap(),
    ] {
        assert_eq!(value_of(&s, r), Dyadic::from_f64(-6.0));
    }
    let r = s.big_sub(Big::ZERO, b).unwrap();
    assert_eq!(value_of(&s, r), Dyadic::from_f64(6.0));
    assert!(s.big_add(Big::ZERO, Big::ZERO).unwrap().is_zero());
    assert_eq!(s.used(), used);
}

#[test]
fn sums_without_room_leave_the_arena_as_it_was() {
    // 2^-1074 and 2^1023: the frame is 34 words.
    let needed = 34;
    for capacity in 0..needed + 2 {
        let mut buf = vec![0.0; capacity];
        let mut s = Scratch::borrowed(&mut buf);
        let (Ok(a), Ok(b)) = (s.big_from_f64(5e-324), s.big_from_f64(2f64.powi(1023))) else {
            continue;
        };
        let used = s.used();
        let free = capacity - used;
        for r in [s.big_add(a, b), s.big_sub(a, b)] {
            if free < needed {
                assert_eq!(r.unwrap_err(), ScratchExhausted, "capacity {capacity}");
                assert_eq!(s.used(), used, "capacity {capacity}");
            } else {
                assert!(r.is_ok());
                s.reset(s.mark());
                break;
            }
        }
    }
}
