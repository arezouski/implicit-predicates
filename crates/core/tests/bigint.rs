//! Big integers: construction, sums, differences, products and chains.

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

fn check_product(s: &mut Scratch<'_>, a: Big, b: Big) -> Result<(), TestCaseError> {
    let (da, db) = (big_to_dyadic(s, a), big_to_dyadic(s, b));
    let used = s.used();
    let p = s.big_mul(a, b).unwrap();
    assert_valid_big(s, p);
    prop_assert_eq!(big_to_dyadic(s, p), &da * &db);
    prop_assert_eq!(p.sign(), oracle_sign(&(&da * &db)));
    // Only the result stays above the operands.
    prop_assert_eq!(s.used(), used + p.len());
    // The operands are untouched.
    prop_assert_eq!(big_to_dyadic(s, a), da);
    prop_assert_eq!(big_to_dyadic(s, b), db);
    Ok(())
}

/// One step of a random program over the values computed so far.
#[derive(Clone, Copy, Debug)]
struct Step {
    op: u8,
    i: usize,
    j: usize,
}

fn any_program() -> impl Strategy<Value = (Vec<f64>, Vec<Step>)> {
    let step = (0u8..4, any::<usize>(), any::<usize>()).prop_map(|(op, i, j)| Step { op, i, j });
    (
        prop::collection::vec(any_finite(), 1..=6),
        prop::collection::vec(step, 0..=12),
    )
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn products_have_their_value(
        x in any_finite(),
        y in any_finite(),
        p in any_big(),
        q in any_big(),
    ) {
        let mut buf = [0.0; 400];
        let mut s = Scratch::borrowed(&mut buf);
        let a = s.big_from_f64(x).unwrap();
        let b = s.big_from_f64(y).unwrap();
        check_product(&mut s, a, b)?;
        check_product(&mut s, b, a)?;
        let mut s = Scratch::borrowed(&mut buf);
        let a = p.build(&mut s).unwrap();
        let b = q.build(&mut s).unwrap();
        check_product(&mut s, a, b)?;
        check_product(&mut s, a, a)?;
    }

    #[test]
    fn chains_have_their_value((leaves, steps) in any_program()) {
        let mut s = Scratch::growable(64);
        let start = s.mark();
        let mut values: Vec<Big> = leaves.iter().map(|&x| s.big_from_f64(x).unwrap()).collect();
        let mut exact: Vec<Dyadic> = leaves.iter().map(|&x| Dyadic::from_f64(x)).collect();
        for Step { op, i, j } in steps {
            let (i, j) = (i % values.len(), j % values.len());
            let (a, b) = (values[i], values[j]);
            let (da, db) = (&exact[i], &exact[j]);
            let (r, d) = match op {
                0 => (s.big_add(a, b).unwrap(), da + db),
                1 => (s.big_sub(a, b).unwrap(), da - db),
                2 => (s.big_mul(a, b).unwrap(), da * db),
                _ => (a.neg(), -da),
            };
            assert_valid_big(&s, r);
            prop_assert_eq!(big_to_dyadic(&s, r), d.clone());
            prop_assert_eq!(r.sign(), oracle_sign(&d));
            values.push(r);
            exact.push(d);
        }
        for (v, d) in values.iter().zip(&exact) {
            prop_assert_eq!(big_to_dyadic(&s, *v), d.clone());
        }
        s.reset(start);
        prop_assert_eq!(s.used(), 0);
    }

    #[test]
    fn a_3x3_determinant_of_extreme_values_has_its_sign(
        m in prop::collection::vec(
            prop::sample::select(vec![
                5e-324, -5e-324, f64::MAX, -f64::MAX, 1.0, -1.0, 0.0,
            ]),
            9,
        ),
    ) {
        let mut s = Scratch::growable(64);
        let v: Vec<Big> = m.iter().map(|&x| s.big_from_f64(x).unwrap()).collect();
        let d: Vec<Dyadic> = m.iter().map(|&x| Dyadic::from_f64(x)).collect();
        // Cofactor expansion along the first row.
        let mut det = Big::ZERO;
        let mut exact = Dyadic::from_f64(0.0);
        for c in 0..3 {
            let (c1, c2) = ((c + 1) % 3, (c + 2) % 3);
            let p = s.big_mul(v[3 + c1], v[6 + c2]).unwrap();
            let q = s.big_mul(v[3 + c2], v[6 + c1]).unwrap();
            let minor = s.big_sub(p, q).unwrap();
            let term = s.big_mul(v[c], minor).unwrap();
            det = s.big_add(det, term).unwrap();
            let em = &(&d[3 + c1] * &d[6 + c2]) - &(&d[3 + c2] * &d[6 + c1]);
            exact = &exact + &(&d[c] * &em);
        }
        assert_valid_big(&s, det);
        prop_assert_eq!(det.sign(), oracle_sign(&exact));
        prop_assert_eq!(big_to_dyadic(&s, det), exact);
    }

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
    // Only the result stays above the operands, whichever words were trimmed.
    let used = s.used();
    let again = s.big_sub(x, p130).unwrap();
    assert_eq!(s.used(), used + again.len());

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

#[test]
fn exponents_too_far_apart_to_align_are_an_exhausted_arena() {
    let mut grow = Scratch::growable(8);
    let mut buf = [0.0; 8];
    let mut fixed = Scratch::borrowed(&mut buf);
    for s in [&mut grow, &mut fixed] {
        let hi = s.big_from_words(false, &[1], i32::MAX).unwrap();
        let lo = s.big_from_words(false, &[1], i32::MIN).unwrap();
        let (used, capacity) = (s.used(), s.capacity());
        for r in [
            s.big_add(hi, lo),
            s.big_sub(hi, lo),
            s.big_add(lo, hi),
            s.big_sub(lo, hi),
            s.big_add(hi, lo.neg()),
        ] {
            assert_eq!(r.unwrap_err(), ScratchExhausted);
            assert_eq!((s.used(), s.capacity()), (used, capacity));
        }
    }
}

#[test]
fn a_product_with_every_carry() {
    let mut buf = [0.0; 64];
    let mut s = Scratch::borrowed(&mut buf);
    let all = |n: usize| vec![u64::MAX; n];
    for (la, lb) in [(1, 1), (2, 1), (3, 3)] {
        let a = s.big_from_words(false, &all(la), 0).unwrap();
        let b = s.big_from_words(false, &all(lb), 0).unwrap();
        let (da, db) = (value_of(&s, a), value_of(&s, b));
        let p = s.big_mul(a, b).unwrap();
        assert_eq!(value_of(&s, p), &da * &db, "{la} x {lb} words");
        // (2^(64a) - 1)(2^(64b) - 1) fills a + b words.
        assert_eq!(p.len(), la + lb);
        let n = s.big_mul(a, b.neg()).unwrap();
        assert_eq!(value_of(&s, n), -&(&da * &db));
    }
}

#[test]
fn a_product_drops_its_zero_words_at_both_ends() {
    let mut buf = [0.0; 64];
    let mut s = Scratch::borrowed(&mut buf);
    // 2^32 * 2^32 = 2^64: the low word of the partial product is zero.
    let a = s.big_from_words(false, &[1 << 32], 3).unwrap();
    let b = s.big_from_words(true, &[1 << 32], -5).unwrap();
    let used = s.used();
    let p = s.big_mul(a, b).unwrap();
    assert_eq!(
        value_of(&s, p),
        &Dyadic::from_words(true, &[1 << 32], 192) * &Dyadic::from_words(false, &[1 << 32], -320)
    );
    assert_eq!((p.len(), p.exponent(), p.is_negative()), (1, -1, true));
    assert_eq!(s.big_limb(p, 0), 1);
    assert_eq!(s.used(), used + 1);
    // Zero words inside stay.
    let c = s.big_from_words(false, &[1, 0, 1], 0).unwrap();
    let q = s.big_mul(c, c).unwrap();
    assert_eq!(value_of(&s, q), &value_of(&s, c) * &value_of(&s, c));
    assert_eq!(q.len(), 5);
}

#[test]
fn a_zero_factor_gives_zero_without_writing() {
    let mut buf = [0.0; 8];
    let mut s = Scratch::borrowed(&mut buf);
    let b = s.big_from_f64(-6.0).unwrap();
    let used = s.used();
    for z in [
        s.big_mul(Big::ZERO, b).unwrap(),
        s.big_mul(b, Big::ZERO).unwrap(),
        s.big_mul(Big::ZERO, Big::ZERO).unwrap(),
    ] {
        assert_valid_big(&s, z);
        assert!(z.is_zero());
    }
    assert_eq!(s.used(), used);
}

#[test]
fn products_without_room_leave_the_arena_as_it_was() {
    // Operands of 3 and 2 words: the product reserves 5.
    let needed = 5;
    for capacity in 0..12 {
        let mut buf = vec![0.0; capacity];
        let mut s = Scratch::borrowed(&mut buf);
        let (Ok(a), Ok(b)) = (
            s.big_from_words(false, &[1, 2, 3], 0),
            s.big_from_words(true, &[4, 5], 0),
        ) else {
            continue;
        };
        let used = s.used();
        let r = s.big_mul(a, b);
        if capacity - used < needed {
            assert_eq!(r.unwrap_err(), ScratchExhausted, "capacity {capacity}");
            assert_eq!(s.used(), used, "capacity {capacity}");
        } else {
            assert!(r.unwrap().len() >= 4);
        }
    }
}

#[test]
fn exponents_that_leave_i32_in_a_product_are_an_exhausted_arena() {
    let mut grow = Scratch::growable(8);
    let mut buf = [0.0; 8];
    let mut fixed = Scratch::borrowed(&mut buf);
    for s in [&mut grow, &mut fixed] {
        let hi = s.big_from_words(false, &[1], i32::MAX).unwrap();
        let lo = s.big_from_words(false, &[1], i32::MIN).unwrap();
        // The sum of exponents is above i32::MAX, below i32::MIN, or ends
        // up above i32::MAX only after the product loses a low word.
        let low_zero = s.big_from_words(false, &[1 << 32], i32::MAX).unwrap();
        let word = s.big_from_words(false, &[1 << 32], 0).unwrap();
        let (used, capacity) = (s.used(), s.capacity());
        for r in [
            s.big_mul(hi, hi),
            s.big_mul(lo, lo),
            s.big_mul(low_zero, word),
        ] {
            assert_eq!(r.unwrap_err(), ScratchExhausted);
            assert_eq!((s.used(), s.capacity()), (used, capacity));
        }
        // The extremes that do fit multiply.
        let r = s.big_mul(hi, lo).unwrap();
        assert_eq!(r.exponent(), -1);
    }
}

#[test]
fn a_sum_whose_result_exponent_leaves_i32_is_an_exhausted_arena() {
    let mut grow = Scratch::growable(16);
    let mut buf = [0.0; 16];
    let mut fixed = Scratch::borrowed(&mut buf);
    for s in [&mut grow, &mut fixed] {
        // 2^128 + 2^64 + 1 and 2^64 + 1 at exponent i32::MAX - 1: the
        // difference is 2^128, whose exponent is i32::MAX + 1.
        let a = s.big_from_words(false, &[1, 1, 1], i32::MAX - 1).unwrap();
        let b = s.big_from_words(false, &[1, 1], i32::MAX - 1).unwrap();
        let used = s.used();
        for r in [
            s.big_sub(a, b),
            s.big_add(a, b.neg()),
            s.big_add(b.neg(), a),
        ] {
            assert_eq!(r.unwrap_err(), ScratchExhausted);
            assert_eq!(s.used(), used);
        }
        // One word lower it fits.
        let a = s.big_from_words(false, &[1, 1, 1], i32::MAX - 2).unwrap();
        let b = s.big_from_words(false, &[1, 1], i32::MAX - 2).unwrap();
        let d = s.big_sub(a, b).unwrap();
        assert_eq!((d.len(), d.exponent()), (1, i32::MAX));
    }
}
