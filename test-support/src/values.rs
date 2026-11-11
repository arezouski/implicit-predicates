//! Generators of floating-point values and of pairs of values with a given
//! relationship.

use indirect_predicates_core::eft;
use proptest::prelude::*;

/// Any finite `f64`, including subnormals and both zeros.
pub fn any_finite() -> impl Strategy<Value = f64> {
    mixed(-1074, f64::MAX_EXP)
}

/// Any value in the product domain of `eft::in_product_domain`: zero of
/// either sign, or a magnitude in `eft::PRODUCT_OPERAND_MIN ..=
/// eft::PRODUCT_OPERAND_MAX`.
pub fn product_operand() -> impl Strategy<Value = f64> {
    mixed(
        exponent_of(eft::PRODUCT_OPERAND_MIN),
        exponent_of(eft::PRODUCT_OPERAND_MAX),
    )
}

/// The exponents `(lowest, highest)` such that every value whose set bits all
/// lie in `2^lowest ..= 2^highest` is a nonzero value of the product domain:
/// a value with those bits has magnitude in `2^lowest .. 2^(highest + 1)`.
pub fn product_domain_bits() -> (i32, i32) {
    (
        exponent_of(eft::PRODUCT_OPERAND_MIN),
        exponent_of(eft::PRODUCT_OPERAND_MAX) - 1,
    )
}

/// The binary exponent of the power of two `x`.
fn exponent_of(x: f64) -> i32 {
    let biased =
        i32::try_from(x.to_bits() >> 52).expect("a positive f64 has an 11-bit biased exponent");
    assert_eq!(
        x.to_bits() & ((1 << 52) - 1),
        0,
        "{x} is not a power of two"
    );
    biased - 1023
}

/// Pairs `(a, b)` where `b` is `-a` moved by up to 4 ulps in either
/// direction, so that `a + b` cancels almost completely. By Sterbenz's lemma
/// such sums are always exact.
pub fn near_cancelling() -> impl Strategy<Value = (f64, f64)> {
    (any_finite(), -4i32..=4)
        .prop_map(|(a, ulps)| (a, step_ulps(-a, ulps)))
        .prop_filter("perturbation must stay finite", |(_, b)| b.is_finite())
}

/// Pairs of nonzero values with random significands and signs whose leading
/// bits are at most 60 positions apart, so that their sum usually has a
/// nonzero rounding error. Both magnitudes are below `2^1021`.
pub fn correlated_pair() -> impl Strategy<Value = (f64, f64)> {
    (
        -1000i32..=960,
        -60i32..=60,
        any::<u64>(),
        any::<u64>(),
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(|(exp, offset, bits_a, bits_b, negative_a, negative_b)| {
            (
                with_leading_bit(exp, bits_a, negative_a),
                with_leading_bit(exp + offset, bits_b, negative_b),
            )
        })
}

/// Pairs `(a, b)` of opposite signs whose sum cancels the leading bit of `a`
/// and still has a nonzero rounding error.
///
/// Sterbenz's lemma makes `a + b` exact whenever `|b|` is within a factor of
/// two of `|a|`, so a sum that cancels and rounds needs `|a| > 2|b|`. Here
/// `|a| = 2^k + s 2^(k-52)` and `|b| = 2^(k-1) - t 2^(k-54)` with `t` odd, so
/// `|a + b| = 2^(k-1) + (4s + t) 2^(k-54)` lies in `[2^(k-1), 2^k)`, where
/// representable values are multiples of `2^(k-53)`, and is not one of them.
/// The order of the pair is random.
pub fn cancellation_that_rounds() -> impl Strategy<Value = (f64, f64)> {
    (
        -1020i32..=1022,
        0u64..(1 << 50),
        0u64..(1 << 51),
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(|(exp, s, half_t, negative, swap)| {
            let t = 2 * half_t + 1;
            let a = with_leading_bit(exp, s, negative);
            let b = with_leading_bit(exp - 2, (1 << 52) - t, !negative);
            if swap { (b, a) } else { (a, b) }
        })
}

/// Nonzero values with random signs whose leading bit is `2^exp` for a
/// uniform `exp` in `min_exp..=max_exp`, and whose significand has a uniform
/// random number of bits in `1..=max_bits`. Short significands occupy few bit
/// positions, so such leaves often stay separate components.
pub fn leaf(min_exp: i32, max_exp: i32, max_bits: u32) -> impl Strategy<Value = f64> {
    assert!(
        (1..=53).contains(&max_bits),
        "a significand has 1 to 53 bits"
    );
    (min_exp..=max_exp, 1..=max_bits, any::<u64>(), any::<bool>()).prop_map(
        |(exp, significant_bits, bits, negative)| {
            // Keep only the `significant_bits - 1` highest of the 52 stored bits.
            let dropped = 53 - significant_bits;
            with_leading_bit(exp, (bits >> dropped) << dropped, negative)
        },
    )
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
        let biased = u64::try_from(exp + 1023)
            .expect("a normal exponent is at least -1022, so its biased form is positive");
        (biased << 52) | (low_bits & ((1 << 52) - 1))
    } else {
        let shift = u32::try_from(exp + 1074)
            .expect("a subnormal exponent is at least -1074, so the shift is nonnegative");
        (1 << shift) | (low_bits & ((1 << shift) - 1))
    };
    f64::from_bits((u64::from(negative) << 63) | magnitude)
}

/// `x` moved `ulps` representable values up (positive) or down (negative).
pub(crate) fn step_ulps(x: f64, ulps: i32) -> f64 {
    let step = if ulps >= 0 {
        f64::next_up
    } else {
        f64::next_down
    };
    (0..ulps.unsigned_abs()).fold(x, |y, _| step(y))
}
