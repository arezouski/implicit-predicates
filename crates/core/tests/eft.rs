//! Exactness of the error-free transformations, checked against the oracle.

use implicit_predicates_core::eft;
use implicit_predicates_oracle::Dyadic;
use implicit_predicates_test_support::{
    any_finite, cancellation_that_rounds, config, correlated_pair, near_cancelling,
    product_operand, value,
};
use proptest::prelude::*;

proptest! {
    #![proptest_config(config())]

    #[test]
    fn two_sum_is_exact(a in any_finite(), b in any_finite()) {
        prop_assume!((a + b).is_finite());
        let (x, y) = eft::two_sum(a, b);
        prop_assert_eq!(x, a + b);
        prop_assert_eq!(value(&[x, y]), &Dyadic::from_f64(a) + &Dyadic::from_f64(b));
    }

    #[test]
    fn two_sum_is_exact_when_cancelling((a, b) in near_cancelling()) {
        let (x, y) = eft::two_sum(a, b);
        prop_assert_eq!(x, a + b);
        prop_assert_eq!(value(&[x, y]), &Dyadic::from_f64(a) + &Dyadic::from_f64(b));
    }

    #[test]
    fn two_sum_is_exact_for_correlated_pairs((a, b) in correlated_pair()) {
        let (x, y) = eft::two_sum(a, b);
        prop_assert_eq!(x, a + b);
        prop_assert_eq!(value(&[x, y]), &Dyadic::from_f64(a) + &Dyadic::from_f64(b));
    }

    #[test]
    fn two_sum_is_exact_when_cancellation_rounds((a, b) in cancellation_that_rounds()) {
        let (x, y) = eft::two_sum(a, b);
        prop_assert_eq!(x, a + b);
        prop_assert_ne!(y, 0.0, "the strategy promises a rounded sum");
        prop_assert_eq!(value(&[x, y]), &Dyadic::from_f64(a) + &Dyadic::from_f64(b));
    }

    #[test]
    fn fast_two_sum_is_exact(a in any_finite(), b in any_finite()) {
        prop_assume!((a + b).is_finite());
        let (a, b) = if a.abs() >= b.abs() { (a, b) } else { (b, a) };
        let (x, y) = eft::fast_two_sum(a, b);
        prop_assert_eq!(x, a + b);
        prop_assert_eq!(value(&[x, y]), &Dyadic::from_f64(a) + &Dyadic::from_f64(b));
    }

    #[test]
    fn two_diff_is_exact(a in any_finite(), b in any_finite()) {
        prop_assume!((a - b).is_finite());
        let (x, y) = eft::two_diff(a, b);
        prop_assert_eq!(x, a - b);
        prop_assert_eq!(value(&[x, y]), &Dyadic::from_f64(a) - &Dyadic::from_f64(b));
    }

    #[test]
    fn two_diff_is_exact_for_correlated_pairs((a, b) in correlated_pair()) {
        let (x, y) = eft::two_diff(a, b);
        prop_assert_eq!(x, a - b);
        prop_assert_eq!(value(&[x, y]), &Dyadic::from_f64(a) - &Dyadic::from_f64(b));
    }

    #[test]
    fn two_diff_is_exact_when_cancellation_rounds((a, b) in cancellation_that_rounds()) {
        // `a - (-b)` performs the same cancellation as `a + b`.
        let (x, y) = eft::two_diff(a, -b);
        prop_assert_eq!(x, a + b);
        prop_assert_ne!(y, 0.0, "the strategy promises a rounded difference");
        prop_assert_eq!(value(&[x, y]), &Dyadic::from_f64(a) + &Dyadic::from_f64(b));
    }

    #[test]
    fn two_product_dekker_is_exact(a in product_operand(), b in product_operand()) {
        let (x, y) = eft::two_product_dekker(a, b);
        prop_assert_eq!(x, a * b);
        prop_assert_eq!(value(&[x, y]), &Dyadic::from_f64(a) * &Dyadic::from_f64(b));
    }

    #[test]
    fn two_product_alias_matches_dekker(a in product_operand(), b in product_operand()) {
        prop_assert_eq!(eft::two_product(a, b), eft::two_product_dekker(a, b));
    }

    #[cfg(any(all(target_arch = "x86_64", target_feature = "fma"), all(target_arch = "aarch64", target_feature = "neon")))]
    #[test]
    fn two_product_fma_matches_dekker(a in product_operand(), b in product_operand()) {
        prop_assert_eq!(eft::two_product_fma(a, b), eft::two_product_dekker(a, b));
    }

    #[test]
    fn two_square_is_exact(a in product_operand()) {
        let (x, y) = eft::two_square(a);
        prop_assert_eq!(x, a * a);
        prop_assert_eq!(value(&[x, y]), &Dyadic::from_f64(a) * &Dyadic::from_f64(a));
    }
}

/// Checks that `product` is exact for operands at the edges of the product
/// domain: the bounds themselves, and the full-mantissa values just below
/// `PRODUCT_OPERAND_MAX` and just below twice `PRODUCT_OPERAND_MIN`.
fn assert_exact_at_domain_edges(product: fn(f64, f64) -> (f64, f64)) {
    let (min, max) = (eft::PRODUCT_OPERAND_MIN, eft::PRODUCT_OPERAND_MAX);
    let hi = max.next_down();
    let lo = (2.0 * min).next_down();
    let edges = [
        (hi, lo),
        (lo, lo),
        (hi, 1.0 + f64::EPSILON),
        (-hi, lo),
        (max, max),
        (min, min),
        (max, -min),
        (-min, 1.0 + f64::EPSILON),
    ];
    for (a, b) in edges {
        let (x, y) = product(a, b);
        assert_eq!(value(&[x, y]), &Dyadic::from_f64(a) * &Dyadic::from_f64(b));
    }
}

#[test]
fn two_product_is_exact_at_domain_edges() {
    assert_exact_at_domain_edges(eft::two_product_dekker);
}

#[cfg(any(
    all(target_arch = "x86_64", target_feature = "fma"),
    all(target_arch = "aarch64", target_feature = "neon")
))]
#[test]
fn two_product_fma_is_exact_at_domain_edges() {
    assert_exact_at_domain_edges(eft::two_product_fma);
}

#[test]
fn product_domain_is_zero_or_magnitude_between_the_bounds() {
    assert_eq!(eft::PRODUCT_OPERAND_MIN, 2f64.powi(-480));
    assert_eq!(eft::PRODUCT_OPERAND_MAX, 2f64.powi(480));
    let (min, max) = (eft::PRODUCT_OPERAND_MIN, eft::PRODUCT_OPERAND_MAX);
    for x in [0.0, -0.0, min, -min, max, -max, 1.0, -3.5] {
        assert!(eft::in_product_domain(x), "{x:e} should be in the domain");
    }
    let outside = [
        min.next_down(),
        -min.next_down(),
        max.next_up(),
        -max.next_up(),
        f64::MIN_POSITIVE,
        5e-324,
        f64::MAX,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
    ];
    for x in outside {
        assert!(!eft::in_product_domain(x), "{x:e} should be outside");
    }
}

#[test]
fn two_square_is_exact_at_domain_edges() {
    let (min, max) = (eft::PRODUCT_OPERAND_MIN, eft::PRODUCT_OPERAND_MAX);
    for a in [min, -min, max, max.next_down(), (2.0 * min).next_down()] {
        let (x, y) = eft::two_square(a);
        assert_eq!(value(&[x, y]), &Dyadic::from_f64(a) * &Dyadic::from_f64(a));
    }
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "outside the product domain")]
fn dekker_product_checks_the_domain_in_debug_builds() {
    let _ = eft::two_product_dekker(1.5, eft::PRODUCT_OPERAND_MAX * 2.0);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "outside the product domain")]
fn dekker_product_checks_the_lower_bound_in_debug_builds() {
    let _ = eft::two_product_dekker(eft::PRODUCT_OPERAND_MIN / 2.0, 1.0);
}

#[cfg(all(
    debug_assertions,
    any(
        all(target_arch = "x86_64", target_feature = "fma"),
        all(target_arch = "aarch64", target_feature = "neon")
    )
))]
#[test]
#[should_panic(expected = "outside the product domain")]
fn fma_product_checks_the_domain_in_debug_builds() {
    let _ = eft::two_product_fma(eft::PRODUCT_OPERAND_MIN / 2.0, 1.0);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "outside the product domain")]
fn selected_product_checks_the_domain_in_debug_builds() {
    let _ = eft::two_product(3.0, -eft::PRODUCT_OPERAND_MAX * 4.0);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "outside the product domain")]
fn square_checks_the_domain_in_debug_builds() {
    let _ = eft::two_square(eft::PRODUCT_OPERAND_MIN / 2.0);
}

#[test]
fn has_fma_matches_target() {
    assert_eq!(
        eft::HAS_FMA,
        cfg!(any(
            all(target_arch = "x86_64", target_feature = "fma"),
            all(target_arch = "aarch64", target_feature = "neon")
        ))
    );
}
