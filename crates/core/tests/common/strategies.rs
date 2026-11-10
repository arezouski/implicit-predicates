//! Input generators: floating-point values, pairs of values with a given
//! relationship, and valid expansions.

use indirect_predicates_core::expansion;
use proptest::prelude::*;

use super::assert_valid_expansion;

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

/// A valid expansion of `0..=max_len` components: the exact sum of up to
/// `max_len` leaves drawn from `leaf`, accumulated with `expansion::grow`.
/// Half of the cases use at least `max_len / 2` leaves, so that long
/// expansions are common.
///
/// Every intermediate expansion is checked with [`assert_valid_expansion`],
/// so a faulty `grow` makes the generator panic instead of feeding invalid
/// inputs to the properties.
pub fn expansion_with(
    max_len: usize,
    leaf: impl Strategy<Value = f64>,
) -> impl Strategy<Value = Vec<f64>> {
    let leaf_count = prop_oneof![0..=max_len, max_len / 2..=max_len];
    (prop::collection::vec(leaf, max_len), leaf_count).prop_map(|(leaves, count)| {
        let mut e = Vec::new();
        for &leaf in &leaves[..count] {
            let mut grown = vec![0.0; e.len() + 1];
            let len = expansion::grow(&e, leaf, &mut grown);
            grown.truncate(len);
            assert_valid_expansion(&grown);
            e = grown;
        }
        e
    })
}

/// A valid expansion of `0..=max_len` components whose leaves have
/// magnitude in `2^-200 ..= 2^200`, so that every component lies well inside
/// the domain of `two_product`. See [`expansion_within`].
pub fn expansion(max_len: usize) -> impl Strategy<Value = Vec<f64>> {
    expansion_within(max_len, 200)
}

/// A valid expansion of `0..=max_len` components whose leaves have magnitude
/// in `2^-max_exp ..= 2^max_exp`.
///
/// Half of the cases draw leaves with significands of at most 4 bits spread
/// over the whole range, which mostly stay separate components and so give
/// long expansions; the other half draw leaves of any significand length
/// whose leading bits are within 60 positions of a common centre, so that
/// they overlap and interact through carries and cancellations.
pub fn expansion_within(max_len: usize, max_exp: i32) -> impl Strategy<Value = Vec<f64>> {
    assert!(
        max_exp > 61,
        "the clustered leaves need a centre 61 binades inside the range"
    );
    let centre_limit = max_exp - 61;
    prop_oneof![
        expansion_with(max_len, leaf(-max_exp, max_exp - 1, 4)),
        (-centre_limit..=centre_limit).prop_flat_map(move |centre| {
            expansion_with(max_len, leaf(centre - 60, centre + 60, 53))
        }),
    ]
}

/// Pairs `(e, f)` of valid expansions of up to `max_len <= 8` components
/// with `f = e + small`, where `|small| < |e| * 2^-54`, so that `e - f =
/// -small` cancels at least the whole leading component of `e`. Every leaf
/// has magnitude in `2^-max_exp ..= 2^max_exp`.
///
/// The leaves of `small` have leading bits in `lo ..= hi` and those of `e` in
/// `hi + gap ..`, with `gap >= 110`. A nonzero `e` is a multiple of
/// `2^(hi + gap - 52)`, and `|small| < 8 * 2^(hi + 1)`, so `|small|` is below
/// `|e| * 2^(56 - gap) <= |e| * 2^-54`. Either of `e` and `small` may be
/// empty.
pub fn near_equal(max_len: usize, max_exp: i32) -> impl Strategy<Value = (Vec<f64>, Vec<f64>)> {
    assert!(
        max_len <= 8,
        "the separation bound assumes at most 8 leaves"
    );
    assert!(max_exp >= 70, "the range must hold both parts and the gap");
    (110..=max_exp + 40)
        .prop_flat_map(move |gap| (Just(gap), -max_exp..=max_exp - 1 - gap))
        .prop_flat_map(move |(gap, hi)| {
            let lo = (hi - 60).max(-max_exp);
            let e_lo = hi + gap;
            let e_leaf = prop_oneof![
                leaf(e_lo, max_exp - 1, 4),
                leaf(e_lo, (e_lo + 60).min(max_exp - 1), 53),
            ];
            (
                expansion_with(max_len, e_leaf),
                expansion_with(max_len, leaf(lo, hi, 53)),
            )
        })
        .prop_map(|(e, small)| {
            let mut f = vec![0.0; e.len() + small.len()];
            let len = expansion::sum(&e, &small, &mut f);
            f.truncate(len);
            assert_valid_expansion(&f);
            (e, f)
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
fn step_ulps(x: f64, ulps: i32) -> f64 {
    let step = if ulps >= 0 {
        f64::next_up
    } else {
        f64::next_down
    };
    (0..ulps.unsigned_abs()).fold(x, |y, _| step(y))
}
