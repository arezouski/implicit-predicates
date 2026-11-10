//! Generators of valid expansions built with the expansion arithmetic of
//! `indirect-predicates-core`, which checks each intermediate result.

use indirect_predicates_core::expansion;
use proptest::prelude::*;

use crate::assert_valid_expansion;
use crate::values::leaf;

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
