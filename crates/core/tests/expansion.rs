//! Exactness and validity of the expansion primitives, checked against the
//! oracle.

mod common;

use common::{
    any_finite, assert_valid_expansion, cancellation_that_rounds, correlated_pair, expansion,
    expansion_with, leaf, near_equal, product_operand, value,
};
use indirect_predicates_core::{Sign, eft, expansion};
use indirect_predicates_oracle::Dyadic;
use proptest::prelude::*;

/// Largest magnitude accepted by the expansion functions without risk of
/// overflow.
const MAGNITUDE_LIMIT: f64 = f64::from_bits((1020 + 1023) << 52); // 2^1020

/// Calls `operation` with an output buffer of exactly `capacity` components
/// and returns the components it wrote, after checking that they form a
/// valid expansion.
fn run(capacity: usize, operation: impl FnOnce(&mut [f64]) -> usize) -> Vec<f64> {
    let mut out = vec![f64::NAN; capacity];
    let len = operation(&mut out);
    assert!(
        len <= capacity,
        "wrote {len} components into a capacity of {capacity}"
    );
    out.truncate(len);
    assert_valid_expansion(&out);
    out
}

/// One operation applied to the running expansion of `chained_operations_stay_exact`.
#[derive(Clone, Debug)]
enum Step {
    Sum(Vec<f64>),
    Diff(Vec<f64>),
    Scale(f64),
    Grow(f64),
}

/// Steps whose operands keep a chain of up to 10 steps inside the domain of
/// `scale`. Leaves lie in `2^-40 ..= 2^41`, so every component is a multiple
/// of `2^-92` below `2^46`; each scale factor lies in `2^-20 ..= 2^21` with at
/// most 8 significant bits, so it moves the lowest set bit down by at most
/// 27 positions and the magnitude up by at most 21. Ten steps thus stay
/// within `2^-362 ..= 2^256`.
fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        expansion_with(8, leaf(-40, 40, 53)).prop_map(Step::Sum),
        expansion_with(8, leaf(-40, 40, 53)).prop_map(Step::Diff),
        leaf(-20, 20, 8).prop_map(Step::Scale),
        leaf(-40, 40, 53).prop_map(Step::Grow),
    ]
}

/// The distance from `x` to the next representable value away from zero.
fn ulp(x: f64) -> f64 {
    x.abs().next_up() - x.abs()
}

proptest! {
    #![proptest_config(common::config())]

    #[test]
    fn constructors_are_exact(a in any_finite(), b in any_finite()) {
        let (da, db) = (Dyadic::from_f64(a), Dyadic::from_f64(b));
        prop_assert_eq!(value(&run(1, |out| expansion::from_f64(a, out))), da.clone());
        if (a + b).is_finite() && a.abs() < MAGNITUDE_LIMIT && b.abs() < MAGNITUDE_LIMIT {
            let s = run(2, |out| expansion::from_two_sum(a, b, out));
            prop_assert_eq!(value(&s), &da + &db);
            let d = run(2, |out| expansion::from_two_diff(a, b, out));
            prop_assert_eq!(value(&d), &da - &db);
        }
    }

    #[test]
    fn constructors_are_exact_for_correlated_pairs((a, b) in correlated_pair()) {
        let (da, db) = (Dyadic::from_f64(a), Dyadic::from_f64(b));
        prop_assert_eq!(value(&run(2, |out| expansion::from_two_sum(a, b, out))), &da + &db);
        prop_assert_eq!(value(&run(2, |out| expansion::from_two_diff(a, b, out))), &da - &db);
    }

    #[test]
    fn constructors_are_exact_when_cancellation_rounds((a, b) in cancellation_that_rounds()) {
        let (da, db) = (Dyadic::from_f64(a), Dyadic::from_f64(b));
        let s = run(2, |out| expansion::from_two_sum(a, b, out));
        prop_assert_eq!(s.len(), 2);
        prop_assert_eq!(value(&s), &da + &db);
        let d = run(2, |out| expansion::from_two_diff(a, -b, out));
        prop_assert_eq!(value(&d), &da + &db);
    }

    #[test]
    fn product_constructor_is_exact(a in product_operand(), b in product_operand()) {
        let p = run(2, |out| expansion::from_two_product(a, b, out));
        prop_assert_eq!(value(&p), &Dyadic::from_f64(a) * &Dyadic::from_f64(b));
    }

    #[test]
    fn grow_is_exact(e in expansion(16), b in any_finite()) {
        prop_assume!(b.abs() < MAGNITUDE_LIMIT);
        let g = run(e.len() + 1, |out| expansion::grow(&e, b, out));
        prop_assert_eq!(value(&g), &value(&e) + &Dyadic::from_f64(b));
    }

    #[test]
    fn sum_is_exact(e in expansion(16), f in expansion(16)) {
        let s = run(e.len() + f.len(), |out| expansion::sum(&e, &f, out));
        prop_assert_eq!(value(&s), &value(&e) + &value(&f));
    }

    #[test]
    fn diff_is_exact(e in expansion(16), f in expansion(16)) {
        let f_before = f.clone();
        let d = run(e.len() + f.len(), |out| expansion::diff(&e, &f, out));
        prop_assert_eq!(value(&d), &value(&e) - &value(&f));
        prop_assert_eq!(f, f_before);
    }

    #[test]
    fn diff_is_exact_after_deep_cancellation((e, f) in near_equal(8, 200)) {
        let d = run(e.len() + f.len(), |out| expansion::diff(&e, &f, out));
        prop_assert_eq!(value(&d), &value(&e) - &value(&f));
        let back = run(e.len() + f.len(), |out| expansion::diff(&f, &e, out));
        prop_assert_eq!(value(&back), &value(&f) - &value(&e));
    }

    #[test]
    fn diff_of_equal_expansions_is_empty(e in expansion(16)) {
        let d = run(2 * e.len(), |out| expansion::diff(&e, &e, out));
        prop_assert!(d.is_empty(), "e - e gave {:?}", d);
    }

    #[test]
    fn scale_is_exact(e in expansion(16), b in product_operand()) {
        let s = run(2 * e.len(), |out| expansion::scale(&e, b, out));
        prop_assert_eq!(value(&s), &value(&e) * &Dyadic::from_f64(b));
    }

    #[test]
    fn outputs_are_valid_inputs_to_every_operation(
        e in expansion(8),
        f in expansion(8),
        b in product_operand(),
        c in product_operand(),
    ) {
        // Feeds the output of each operation to the others, checking validity
        // at every step: each one's guarantee must meet the others' requirement.
        let scaled = run(2 * e.len(), |out| expansion::scale(&e, b, out));
        let summed = run(e.len() + f.len(), |out| expansion::sum(&e, &f, out));
        let grown = run(summed.len() + 1, |out| expansion::grow(&summed, c, out));
        let rescaled = run(2 * summed.len(), |out| expansion::scale(&summed, c, out));
        let mixed = run(scaled.len() + grown.len(), |out| expansion::diff(&scaled, &grown, out));
        let total = run(mixed.len() + rescaled.len(), |out| expansion::sum(&mixed, &rescaled, out));
        let (ve, vf) = (value(&e), value(&f));
        let (vb, vc) = (Dyadic::from_f64(b), Dyadic::from_f64(c));
        let sum_ef = &ve + &vf;
        let expected = &(&(&ve * &vb) - &(&sum_ef + &vc)) + &(&sum_ef * &vc);
        prop_assert_eq!(value(&total), expected);
    }

    #[test]
    fn chained_operations_stay_exact(
        start in expansion_with(8, leaf(-40, 40, 53)),
        steps in prop::collection::vec(step(), 6..=10),
    ) {
        let mut e = start;
        let mut expected = value(&e);
        for step in &steps {
            match step {
                Step::Sum(f) => {
                    e = run(e.len() + f.len(), |out| expansion::sum(&e, f, out));
                    expected = &expected + &value(f);
                }
                Step::Diff(f) => {
                    e = run(e.len() + f.len(), |out| expansion::diff(&e, f, out));
                    expected = &expected - &value(f);
                }
                Step::Scale(b) => {
                    prop_assert!(
                        e.iter().all(|&x| eft::in_product_domain(x)),
                        "{:?} left the domain",
                        e
                    );
                    e = run(2 * e.len(), |out| expansion::scale(&e, *b, out));
                    expected = &expected * &Dyadic::from_f64(*b);
                }
                Step::Grow(b) => {
                    e = run(e.len() + 1, |out| expansion::grow(&e, *b, out));
                    expected = &expected + &Dyadic::from_f64(*b);
                }
            }
            prop_assert_eq!(value(&e), expected.clone(), "after {:?}", step);
        }
    }

    #[test]
    fn sign_matches_oracle(e in expansion(16)) {
        prop_assert_eq!(expansion::sign(&e) as i8, value(&e).sign());
    }

    #[test]
    fn sign_is_exact_after_deep_cancellation((e, f) in near_equal(8, 200)) {
        let expected = (&value(&e) - &value(&f)).sign();
        let d = run(e.len() + f.len(), |out| expansion::diff(&e, &f, out));
        prop_assert_eq!(expansion::sign(&d) as i8, expected);
        let back = run(e.len() + f.len(), |out| expansion::diff(&f, &e, out));
        prop_assert_eq!(expansion::sign(&back) as i8, -expected);
    }

    #[test]
    fn negate_flips_value(e in expansion(16)) {
        let mut negated = e.clone();
        expansion::negate(&mut negated);
        assert_valid_expansion(&negated);
        prop_assert_eq!(value(&negated), -value(&e));
    }

    #[test]
    fn estimate_is_close(e in expansion(16)) {
        let estimate = expansion::estimate(&e);
        let error = &value(&e) - &Dyadic::from_f64(estimate);
        let bound = Dyadic::from_f64(2.0 * ulp(estimate));
        prop_assert!(-&bound <= error && error <= bound, "estimate {} of {:?}", estimate, e);
    }

    #[test]
    fn output_never_exceeds_capacity(
        e in expansion(16),
        f in expansion(16),
        a in product_operand(),
        b in product_operand(),
    ) {
        // A longer buffer filled with NaN shows both the returned length and
        // that nothing is written past it.
        let check = |capacity: usize, operation: &dyn Fn(&mut [f64]) -> usize| {
            let mut out = vec![f64::NAN; capacity + 4];
            let len = operation(&mut out);
            assert!(len <= capacity, "wrote {len} components into a capacity of {capacity}");
            assert!(out[len..].iter().all(|x| x.is_nan()), "wrote past the returned length");
        };
        check(1, &|out| expansion::from_f64(a, out));
        check(2, &|out| expansion::from_two_sum(a, b, out));
        check(2, &|out| expansion::from_two_diff(a, b, out));
        check(2, &|out| expansion::from_two_product(a, b, out));
        check(e.len() + 1, &|out| expansion::grow(&e, b, out));
        check(e.len() + f.len(), &|out| expansion::sum(&e, &f, out));
        check(e.len() + f.len(), &|out| expansion::diff(&e, &f, out));
        check(2 * e.len(), &|out| expansion::scale(&e, b, out));
    }
}

#[test]
fn negative_zero_and_zero_give_the_empty_expansion() {
    let mut out = [f64::NAN; 2];
    assert_eq!(expansion::from_f64(-0.0, &mut out), 0);
    assert_eq!(expansion::from_f64(0.0, &mut out), 0);
    assert_eq!(expansion::from_two_product(-0.0, 5.0, &mut out), 0);
    assert_eq!(expansion::from_two_diff(1.5, 1.5, &mut out), 0);
}

#[test]
fn empty_operands_are_handled() {
    let e = [1e-20, 1.0];
    let mut out = [f64::NAN; 4];
    assert_eq!(expansion::sum(&e, &[], &mut out), 2);
    assert_eq!(&out[..2], &e);
    assert_eq!(expansion::sum(&[], &[], &mut out), 0);
    let n = expansion::diff(&[], &e, &mut out);
    assert_eq!(&out[..n], &[-1e-20, -1.0]);
    assert_eq!(expansion::scale(&e, 0.0, &mut out), 0);
    assert_eq!(expansion::scale(&[], 3.0, &mut out), 0);
    assert_eq!(expansion::grow(&[], 0.0, &mut out), 0);
    assert_eq!(expansion::sign(&[]), Sign::Zero);
    assert_eq!(expansion::estimate(&[]), 0.0);
}

#[test]
fn sum_and_diff_are_exact_with_subnormals() {
    let e = [5e-324, 1.0];
    let f = [1e-310, 2.0];
    let mut out = [0.0; 4];
    let n = expansion::sum(&e, &f, &mut out);
    assert_valid_expansion(&out[..n]);
    assert_eq!(value(&out[..n]), &value(&e) + &value(&f));
    let n = expansion::diff(&e, &f, &mut out);
    assert_valid_expansion(&out[..n]);
    assert_eq!(value(&out[..n]), &value(&e) - &value(&f));
}

#[test]
fn sum_of_a_rounding_tie_followed_by_cancellation_is_exact() {
    // FAST-EXPANSION-SUM gives `[0.5, -3.0]` here: `2^52 + 0.5` is a tie that
    // rounds to even, then `2^52 - (2^52 + 3)` is exact. That output is
    // nonoverlapping but not strongly nonoverlapping, which disproves the
    // invariant FAST-EXPANSION-SUM needs from its inputs.
    let two_52 = 4_503_599_627_370_496.0;
    let e = [0.5, two_52];
    let f = [-(two_52 + 3.0)];
    let mut out = [f64::NAN; 3];
    let n = expansion::sum(&e, &f, &mut out);
    assert_valid_expansion(&out[..n]);
    assert_eq!(value(&out[..n]), &value(&e) + &value(&f));
}

#[test]
fn grow_and_scale_by_zero_keep_the_value() {
    let e = [1e-20, 1.0];
    let mut out = [f64::NAN; 4];
    let n = expansion::grow(&e, -0.0, &mut out);
    assert_eq!(&out[..n], &e);
    assert_eq!(expansion::scale(&e, -0.0, &mut out), 0);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "outside the product domain")]
fn product_constructor_checks_the_domain_in_debug_builds() {
    let mut out = [0.0; 2];
    let _ = expansion::from_two_product(eft::PRODUCT_OPERAND_MAX * 2.0, 1.0, &mut out);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "outside the product domain")]
fn scale_checks_its_components_in_debug_builds() {
    let mut out = [0.0; 4];
    let _ = expansion::scale(&[eft::PRODUCT_OPERAND_MIN / 2.0, 1.0], 3.0, &mut out);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "outside the product domain")]
fn scale_checks_its_factor_in_debug_builds() {
    let mut out = [0.0; 4];
    let _ = expansion::scale(&[0.25, 1.0], eft::PRODUCT_OPERAND_MAX * 2.0, &mut out);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "not finite")]
fn from_f64_checks_finiteness_in_debug_builds() {
    let mut out = [0.0; 1];
    let _ = expansion::from_f64(f64::NAN, &mut out);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "not finite")]
fn grow_checks_finiteness_in_debug_builds() {
    let mut out = [0.0; 2];
    let _ = expansion::grow(&[1.0], f64::INFINITY, &mut out);
}

/// The result needs two components and the buffer holds one.
fn sum_into_a_buffer_too_small() {
    let mut out = [0.0; 1];
    let _ = expansion::sum(&[1e-30], &[1.0], &mut out);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "capacity")]
fn insufficient_capacity_fails_the_capacity_check() {
    sum_into_a_buffer_too_small();
}

#[cfg(not(debug_assertions))]
#[test]
#[should_panic(expected = "out of bounds")]
fn insufficient_capacity_panics_instead_of_writing_past_the_buffer() {
    sum_into_a_buffer_too_small();
}
