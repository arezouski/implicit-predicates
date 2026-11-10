//! Checks that the benchmark inputs are what the benchmarks claim, and that
//! the computations they time give correct results: both libraries agree on
//! `sum` and `scale`, `product` and `det3` agree with the exact oracle.
//!
//! The checks use the same seeds and lengths as the benchmarks.

use indirect_predicates_benches::{
    DET3_CAPACITY, FIRST_OPERAND_SEED, LENGTHS, MATRICES_SEED, Matrix3, PAIR_COUNT, PAIRS_SEED,
    PRODUCT_LENGTHS, SCALE_SEED, SECOND_OPERAND_SEED, det3_sign, near_singular_matrices,
    product_capacity, product_operands, product_pairs, random_matrices, scale_factor,
    singular_matrices, valid_expansion,
};
use indirect_predicates_core::{Scratch, Sign, eft, expansion};
use indirect_predicates_oracle::Dyadic;

/// Lowest and highest set bit exponents of a nonzero finite `f64`.
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
        for seed in [FIRST_OPERAND_SEED, SECOND_OPERAND_SEED, 99] {
            let e = valid_expansion(seed, n);
            assert_eq!(e.len(), n);
            assert_valid(&e);
        }
    }
}

#[test]
fn inputs_stay_in_the_product_domain() {
    for &n in LENGTHS.iter().chain(&PRODUCT_LENGTHS) {
        for seed in [FIRST_OPERAND_SEED, SECOND_OPERAND_SEED] {
            for x in valid_expansion(seed, n) {
                assert!(eft::in_product_domain(x), "{x:e} of length {n}");
            }
        }
    }
    assert!(eft::in_product_domain(scale_factor(SCALE_SEED)));
    let pairs = product_pairs(PAIRS_SEED);
    assert_eq!(pairs.len(), PAIR_COUNT);
    for (a, b) in pairs {
        assert!(eft::in_product_domain(a) && eft::in_product_domain(b));
        assert!(a.abs() > 2f64.powi(-25) && a.abs() < 2f64.powi(25));
        assert!(b.abs() > 2f64.powi(-25) && b.abs() < 2f64.powi(25));
    }
}

/// Asserts that expansions `a` and `b` have the same exact value.
fn assert_same_value(a: &[f64], b: &[f64]) {
    assert_eq!(Dyadic::sum_of(a), Dyadic::sum_of(b), "values differ");
}

#[test]
fn both_libraries_agree_on_sum_and_scale() {
    for &n in &LENGTHS {
        let e = valid_expansion(FIRST_OPERAND_SEED, n);
        let f = valid_expansion(SECOND_OPERAND_SEED, n);
        let mut ours = vec![0.0; 2 * n];
        let mut theirs = vec![0.0; 2 * n];
        let expected = &Dyadic::sum_of(&e) + &Dyadic::sum_of(&f);
        let l1 = expansion::sum(&e, &f, &mut ours);
        let l2 = geometry_predicates::predicates::fast_expansion_sum_zeroelim(&e, &f, &mut theirs);
        assert_valid(&ours[..l1]);
        assert_eq!(Dyadic::sum_of(&ours[..l1]), expected);
        assert_same_value(&ours[..l1], &theirs[..l2]);

        let b = scale_factor(SCALE_SEED);
        let expected = &Dyadic::sum_of(&e) * &Dyadic::from_f64(b);
        let l1 = expansion::scale(&e, b, &mut ours);
        let l2 = geometry_predicates::predicates::scale_expansion_zeroelim(&e, b, &mut theirs);
        assert_valid(&ours[..l1]);
        assert_eq!(Dyadic::sum_of(&ours[..l1]), expected);
        assert_same_value(&ours[..l1], &theirs[..l2]);
    }
}

#[test]
fn product_is_exact_on_the_benchmark_operands() {
    for &n in &PRODUCT_LENGTHS {
        let (e, f) = product_operands(n);
        let mut buf = vec![0.0; product_capacity(n)];
        let mut scratch = Scratch::borrowed(&mut buf);
        let a = scratch.push(&e).expect("capacity");
        let b = scratch.push(&f).expect("capacity");
        let p = scratch.product(a, b).expect("capacity");
        assert_valid(scratch.get(p));
        let expected = &Dyadic::sum_of(&e) * &Dyadic::sum_of(&f);
        assert_eq!(Dyadic::sum_of(scratch.get(p)), expected, "length {n}");
    }
}

/// The exact sign of the determinant of `m`, from the oracle.
fn oracle_sign(m: &Matrix3) -> Sign {
    let d = |i: usize, j: usize| Dyadic::from_f64(m[i][j]);
    let minor =
        |r: usize, c: usize, s: usize, t: usize| &(&d(1, r) * &d(2, c)) - &(&d(1, s) * &d(2, t));
    let det = &(&(&d(0, 0) * &minor(1, 2, 2, 1)) - &(&d(0, 1) * &minor(0, 2, 2, 0)))
        + &(&d(0, 2) * &minor(0, 1, 1, 0));
    match det.sign() {
        -1 => Sign::Negative,
        0 => Sign::Zero,
        _ => Sign::Positive,
    }
}

/// Checks every matrix of `set` against the oracle and returns the signs.
fn checked_signs(set: &[Matrix3]) -> Vec<Sign> {
    let mut buf = vec![0.0; DET3_CAPACITY];
    let mut scratch = Scratch::borrowed(&mut buf);
    set.iter()
        .map(|m| {
            let sign = det3_sign(&mut scratch, m);
            assert_eq!(sign, oracle_sign(m), "wrong sign for {m:?}");
            assert_eq!(scratch.used(), 0, "the arena was not restored");
            sign
        })
        .collect()
}

#[test]
fn det3_matches_the_oracle_and_the_sets_have_the_claimed_signs() {
    let random = checked_signs(&random_matrices(MATRICES_SEED));
    assert!(random.iter().all(|&s| s != Sign::Zero));
    assert!(random.contains(&Sign::Positive) && random.contains(&Sign::Negative));

    let near = checked_signs(&near_singular_matrices(MATRICES_SEED));
    assert!(near.iter().all(|&s| s != Sign::Zero));
    assert!(near.contains(&Sign::Positive) && near.contains(&Sign::Negative));

    let singular = checked_signs(&singular_matrices(MATRICES_SEED));
    assert!(singular.iter().all(|&s| s == Sign::Zero));
}
