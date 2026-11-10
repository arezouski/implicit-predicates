//! Behavior of the exact dyadic number.

use indirect_predicates_oracle::Dyadic;
use proptest::prelude::*;

#[test]
fn from_f64_is_exact_for_known_values() {
    let one = Dyadic::from_f64(1.0);
    assert_eq!(&one + &one, Dyadic::from_f64(2.0));
    // 0.1 is 3602879701896397 * 2^-55
    let tenth = Dyadic::from_f64(0.1);
    let mut scaled = tenth.clone();
    for _ in 0..55 {
        scaled = &scaled + &scaled;
    }
    assert_eq!(scaled, Dyadic::from_f64(3602879701896397.0));
}

#[test]
fn extremes_and_subnormals_are_exact() {
    let tiny = Dyadic::from_f64(5e-324); // 2^-1074
    assert_eq!(tiny.sign(), 1);
    assert_eq!(&tiny + &tiny, Dyadic::from_f64(1e-323));
    let max = Dyadic::from_f64(f64::MAX);
    assert!(&max + &max > max); // no overflow in the oracle
    assert_eq!((&max - &max).sign(), 0);
}

#[test]
fn negative_zero_is_zero() {
    assert!(Dyadic::from_f64(-0.0).is_zero());
    assert_eq!(Dyadic::from_f64(-0.0), Dyadic::from_f64(0.0));
}

#[test]
fn equality_compares_values_not_representations() {
    // 2^3 built two ways
    let a = Dyadic::from_f64(8.0);
    let b = &Dyadic::from_f64(2.0) * &Dyadic::from_f64(4.0);
    assert_eq!(a, b);
}

#[test]
fn sum_of_adds_all_components() {
    assert_eq!(
        Dyadic::sum_of(&[1e-30, 1.0, 1e30]),
        &(&Dyadic::from_f64(1e-30) + &Dyadic::from_f64(1.0)) + &Dyadic::from_f64(1e30)
    );
    assert!(Dyadic::sum_of(&[]).is_zero());
}

#[test]
fn owned_operators_match_reference_operators() {
    let a = Dyadic::from_f64(0.75);
    let b = Dyadic::from_f64(-6.5);
    assert_eq!(a.clone() + b.clone(), &a + &b);
    assert_eq!(a.clone() - b.clone(), &a - &b);
    assert_eq!(a.clone() * b.clone(), &a * &b);
    assert_eq!(-a.clone(), -&a);
    assert_eq!(&-&a + &a, Dyadic::from_f64(0.0));
}

#[test]
#[should_panic(expected = "Dyadic::from_f64: non-finite input")]
fn nan_panics() {
    let _ = Dyadic::from_f64(f64::NAN);
}

#[test]
#[should_panic(expected = "Dyadic::from_f64: non-finite input")]
fn infinity_panics() {
    let _ = Dyadic::from_f64(f64::INFINITY);
}

fn sign_i128(x: i128) -> i8 {
    x.signum() as i8
}

proptest! {
    #[test]
    fn matches_i128_arithmetic(
        a in -(1i64 << 40)..(1i64 << 40),
        b in -(1i64 << 40)..(1i64 << 40),
        c in -(1i64 << 40)..(1i64 << 40),
    ) {
        let (da, db, dc) = (
            Dyadic::from_f64(a as f64),
            Dyadic::from_f64(b as f64),
            Dyadic::from_f64(c as f64),
        );
        let product = &da * &db;
        let (ia, ib, ic) = (a as i128, b as i128, c as i128);
        prop_assert_eq!((&product + &dc).sign(), sign_i128(ia * ib + ic));
        prop_assert_eq!((&product - &dc).sign(), sign_i128(ia * ib - ic));
        prop_assert_eq!(product.cmp(&dc), (ia * ib).cmp(&ic));
    }
}
