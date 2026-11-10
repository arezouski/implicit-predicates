//! Exactness of the error-free transformations, checked against the oracle.

mod common;

use common::{any_finite, in_product_domain, near_cancelling, value};
use indirect_predicates_core::eft;
use indirect_predicates_oracle::Dyadic;
use proptest::prelude::*;

proptest! {
    #![proptest_config(common::config())]

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
    fn two_product_dekker_is_exact(a in in_product_domain(), b in in_product_domain()) {
        let (x, y) = eft::two_product_dekker(a, b);
        prop_assert_eq!(x, a * b);
        prop_assert_eq!(value(&[x, y]), &Dyadic::from_f64(a) * &Dyadic::from_f64(b));
    }

    #[test]
    fn two_product_alias_matches_dekker(a in in_product_domain(), b in in_product_domain()) {
        prop_assert_eq!(eft::two_product(a, b), eft::two_product_dekker(a, b));
    }

    #[cfg(any(all(target_arch = "x86_64", target_feature = "fma"), target_arch = "aarch64"))]
    #[test]
    fn two_product_fma_matches_dekker(a in in_product_domain(), b in in_product_domain()) {
        prop_assert_eq!(eft::two_product_fma(a, b), eft::two_product_dekker(a, b));
    }

    #[test]
    fn two_square_is_exact(a in in_product_domain()) {
        let (x, y) = eft::two_square(a);
        prop_assert_eq!(x, a * a);
        prop_assert_eq!(value(&[x, y]), &Dyadic::from_f64(a) * &Dyadic::from_f64(a));
    }
}

/// Checks that `product` is exact for operands at the edges of the documented
/// domain of `two_product`.
fn assert_exact_at_domain_edges(product: fn(f64, f64) -> (f64, f64)) {
    let hi = f64::from_bits(0x5DEF_FFFF_FFFF_FFFF); // just below 2^480, full mantissa
    let lo = f64::from_bits(0x21FF_FFFF_FFFF_FFFF); // just below 2^-479, full mantissa
    for (a, b) in [(hi, lo), (lo, lo), (hi, 1.0 + f64::EPSILON), (-hi, lo)] {
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
    target_arch = "aarch64"
))]
#[test]
fn two_product_fma_is_exact_at_domain_edges() {
    assert_exact_at_domain_edges(eft::two_product_fma);
}

#[test]
fn has_fma_matches_target() {
    assert_eq!(
        eft::HAS_FMA,
        cfg!(any(
            all(target_arch = "x86_64", target_feature = "fma"),
            target_arch = "aarch64"
        ))
    );
}
