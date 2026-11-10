//! Shared infrastructure for the property tests of this crate: proptest
//! configuration, input generators and exact checks against the oracle.

// Each integration-test crate uses only part of this module.
#![allow(dead_code)]

use indirect_predicates_oracle::Dyadic;
use proptest::prelude::*;
use proptest::test_runner::Config as ProptestConfig;

/// Number of cases per property when `PROPTEST_CASES` is not set.
const DEFAULT_CASES: u32 = 2000;

/// Proptest configuration shared by every property test.
///
/// The number of cases comes from the `PROPTEST_CASES` environment variable,
/// defaulting to [`DEFAULT_CASES`].
pub fn config() -> ProptestConfig {
    let cases = match std::env::var("PROPTEST_CASES") {
        Ok(text) => text
            .parse()
            .expect("PROPTEST_CASES must be a non-negative integer"),
        Err(_) => DEFAULT_CASES,
    };
    ProptestConfig::with_cases(cases)
}

/// Any finite `f64`, including subnormals and both zeros.
pub fn any_finite() -> impl Strategy<Value = f64> {
    mixed(-1074, f64::MAX_EXP)
}

/// Zero of either sign, or a value with magnitude in `2^-480 ..= 2^480`: the
/// documented domain of `two_product` and `two_square`.
pub fn in_product_domain() -> impl Strategy<Value = f64> {
    mixed(-480, 480)
}

/// Pairs `(a, b)` where `b` is `-a` moved by up to 4 ulps in either
/// direction, so that `a + b` cancels almost completely.
pub fn near_cancelling() -> impl Strategy<Value = (f64, f64)> {
    (any_finite(), -4i32..=4)
        .prop_map(|(a, ulps)| (a, step_ulps(-a, ulps)))
        .prop_filter("perturbation must stay finite", |(_, b)| b.is_finite())
}

/// The exact value of the sum of `xs`.
pub fn value(xs: &[f64]) -> Dyadic {
    Dyadic::sum_of(xs)
}

/// Panics unless `e` is a valid expansion: finite nonzero components sorted
/// by increasing magnitude, each nonoverlapping with the next (its highest
/// set bit lies strictly below the lowest set bit of the next component).
pub fn assert_valid_expansion(e: &[f64]) {
    for (i, &component) in e.iter().enumerate() {
        assert!(
            component.is_finite() && component != 0.0,
            "component {i} of {e:?} is zero or not finite"
        );
    }
    for (i, pair) in e.windows(2).enumerate() {
        assert!(
            highest_bit(pair[0]) < lowest_bit(pair[1]),
            "components {i} and {} of {e:?} overlap or are out of order",
            i + 1
        );
    }
}

/// Mixes, with equal weights, the value kinds that exercise floating-point
/// edge cases. Nonzero magnitudes lie in `2^min_exp ..= 2^max_exp` (strictly
/// below `2^max_exp` when that power is not representable).
fn mixed(min_exp: i32, max_exp: i32) -> impl Strategy<Value = f64> {
    let top_power = max_exp.min(f64::MAX_EXP - 1);
    prop_oneof![
        // Uniform exponent, random mantissa.
        (min_exp..max_exp, any::<u64>(), any::<bool>())
            .prop_map(|(exp, bits, negative)| with_leading_bit(exp, bits, negative)),
        // Powers of two.
        (min_exp..=top_power, any::<bool>())
            .prop_map(|(exp, negative)| with_leading_bit(exp, 0, negative)),
        // Small integers.
        (-1024i32..=1024).prop_map(f64::from),
        // Grid values: multiples of 2^-10.
        (-(1i32 << 20)..(1i32 << 20)).prop_map(|n| f64::from(n) / 1024.0),
        // Zeros of both signs.
        prop_oneof![Just(0.0), Just(-0.0)],
    ]
}

/// The finite `f64` whose highest set bit is `2^exp` (with `exp` in
/// `-1074..=1023`), whose lower significand bits are taken from `low_bits`,
/// and whose sign is negative if `negative`.
fn with_leading_bit(exp: i32, low_bits: u64, negative: bool) -> f64 {
    assert!((-1074..=1023).contains(&exp), "exponent {exp} out of range");
    let magnitude = if exp >= -1022 {
        let biased = u64::try_from(exp + 1023).expect("normal exponent");
        (biased << 52) | (low_bits & ((1 << 52) - 1))
    } else {
        let shift = u32::try_from(exp + 1074).expect("subnormal exponent");
        (1 << shift) | (low_bits & ((1 << shift) - 1))
    };
    f64::from_bits((u64::from(negative) << 63) | magnitude)
}

/// `x` moved `ulps` representable values up (positive) or down (negative).
fn step_ulps(x: f64, ulps: i32) -> f64 {
    let step = if ulps >= 0 {
        f64::next_up
    } else {
        f64::next_down
    };
    (0..ulps.unsigned_abs()).fold(x, |y, _| step(y))
}

/// The exponent of the highest set bit of a finite nonzero `x`.
fn highest_bit(x: f64) -> i32 {
    let (significand, scale) = integer_significand(x);
    scale + 63 - i32::try_from(significand.leading_zeros()).expect("small")
}

/// The exponent of the lowest set bit of a finite nonzero `x`.
fn lowest_bit(x: f64) -> i32 {
    let (significand, scale) = integer_significand(x);
    scale + i32::try_from(significand.trailing_zeros()).expect("small")
}

/// `(s, q)` with `|x| = s * 2^q` and `s` an integer below `2^53`.
fn integer_significand(x: f64) -> (u64, i32) {
    let bits = x.to_bits();
    let biased = i32::try_from((bits >> 52) & 0x7FF).expect("11-bit exponent");
    let fraction = bits & ((1 << 52) - 1);
    if biased == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1 << 52), biased - 1075)
    }
}
