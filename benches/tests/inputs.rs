//! Checks that the benchmark inputs are what the benchmarks claim, and that
//! `geometry-predicates` and this crate compute the same exact values on
//! them, so the timings compare equal work.

use indirect_predicates_benches::{PAIR_COUNT, product_pairs, scale_factor, valid_expansion};
use indirect_predicates_core::expansion;

const LENGTHS: [usize; 6] = [2, 4, 8, 16, 32, 64];

/// Lowest and highest set bit exponents of a nonzero finite `f64` whose
/// significand is an integer times a power of two within normal range.
fn bit_range(x: f64) -> (i32, i32) {
    let bits = x.abs().to_bits();
    let exponent = ((bits >> 52) & 0x7FF) as i32 - 1075;
    let mantissa = (bits & ((1 << 52) - 1)) | (1 << 52);
    let low = exponent + mantissa.trailing_zeros() as i32;
    let high = exponent + 63 - mantissa.leading_zeros() as i32;
    (low, high)
}

fn assert_valid(e: &[f64]) {
    for &x in e {
        assert!(x != 0.0 && x.is_finite());
    }
    for pair in e.windows(2) {
        assert!(
            bit_range(pair[0]).1 < bit_range(pair[1]).0,
            "overlap in {pair:?}"
        );
    }
}

#[test]
fn inputs_are_valid_expansions_of_the_stated_length() {
    for &n in &LENGTHS {
        for seed in [1, 2, 3] {
            let e = valid_expansion(seed, n);
            assert_eq!(e.len(), n);
            assert_valid(&e);
        }
    }
}

#[test]
fn inputs_stay_in_the_product_domain() {
    for &n in &LENGTHS {
        for &x in &valid_expansion(7, n) {
            assert!((-480..=480).contains(&bit_range(x).0));
            assert!((-480..=480).contains(&bit_range(x).1));
        }
    }
    for (a, b) in product_pairs(5) {
        assert!(a.abs() > 2f64.powi(-25) && a.abs() < 2f64.powi(25));
        assert!(b.abs() > 2f64.powi(-25) && b.abs() < 2f64.powi(25));
    }
    assert_eq!(product_pairs(5).len(), PAIR_COUNT);
}

/// Asserts that expansions `a` and `b` have the same exact value.
fn assert_same_value(a: &[f64], b: &[f64]) {
    let mut out = vec![0.0; a.len() + b.len()];
    assert_eq!(expansion::diff(a, b, &mut out), 0, "values differ");
}

#[test]
fn both_libraries_agree_on_sum_and_scale() {
    for &n in &LENGTHS {
        let e = valid_expansion(11, n);
        let f = valid_expansion(12, n);
        let mut ours = vec![0.0; 2 * n];
        let mut theirs = vec![0.0; 2 * n];
        let l1 = expansion::sum(&e, &f, &mut ours);
        let l2 = geometry_predicates::predicates::fast_expansion_sum_zeroelim(&e, &f, &mut theirs);
        assert_valid(&ours[..l1]);
        assert_same_value(&ours[..l1], &theirs[..l2]);

        let b = scale_factor(13);
        let l1 = expansion::scale(&e, b, &mut ours);
        let l2 = geometry_predicates::predicates::scale_expansion_zeroelim(&e, b, &mut theirs);
        assert_valid(&ours[..l1]);
        assert_same_value(&ours[..l1], &theirs[..l2]);
    }
}
