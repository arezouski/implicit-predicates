//! Behavior of the exact dyadic number.

use implicit_predicates_oracle::Dyadic;
use proptest::prelude::*;
use proptest::test_runner::Config as ProptestConfig;

/// Proptest configuration: the number of cases comes from the
/// `PROPTEST_CASES` environment variable, defaulting to 2000, as in the
/// rest of the workspace. The shared test-support crate depends on
/// `implicit-predicates-core`, which the oracle must stay independent of,
/// so this small piece is repeated here.
fn config() -> ProptestConfig {
    let cases = match std::env::var("PROPTEST_CASES") {
        Ok(text) => text
            .parse()
            .expect("PROPTEST_CASES must be a non-negative integer"),
        Err(_) => 2000,
    };
    ProptestConfig::with_cases(cases)
}

/// Any finite `f64`: drawn by class (normal, subnormal, zero, either sign)
/// or from a random bit pattern.
fn finite() -> impl Strategy<Value = f64> {
    use proptest::num::f64::{NEGATIVE, NORMAL, POSITIVE, SUBNORMAL, ZERO};
    prop_oneof![
        POSITIVE | NEGATIVE | NORMAL | SUBNORMAL | ZERO,
        any::<u64>()
            .prop_map(f64::from_bits)
            .prop_filter("finite", |x| x.is_finite()),
    ]
}

/// Pairs of finite values: independent, equal, neighbours, opposite, or of
/// opposite zeros, so that comparisons see every outcome.
fn finite_pair() -> impl Strategy<Value = (f64, f64)> {
    prop_oneof![
        (finite(), finite()),
        finite().prop_map(|a| (a, a)),
        finite()
            .prop_map(|a| (a, a.next_up()))
            .prop_filter("finite", |(_, b)| b.is_finite()),
        finite()
            .prop_map(|a| (a, a.next_down()))
            .prop_filter("finite", |(_, b)| b.is_finite()),
        finite().prop_map(|a| (a, -a)),
        Just((0.0, -0.0)),
    ]
}

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
    #![proptest_config(config())]

    #[test]
    fn ordering_matches_f64_ordering((a, b) in finite_pair()) {
        // `partial_cmp` treats `-0.0` and `0.0` as equal, as `Dyadic` does.
        let expected = a.partial_cmp(&b).expect("finite values are ordered");
        prop_assert_eq!(Dyadic::from_f64(a).cmp(&Dyadic::from_f64(b)), expected);
    }

    #[test]
    fn adding_then_subtracting_gives_back_the_value((a, b) in finite_pair()) {
        let (da, db) = (Dyadic::from_f64(a), Dyadic::from_f64(b));
        prop_assert_eq!(&(&da + &db) - &db, da);
    }

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
