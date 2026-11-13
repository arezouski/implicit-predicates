//! Arbitrary 64-bit words survive being stored in the `f64` arena.

use implicit_predicates_core::Scratch;
use proptest::prelude::*;

/// Patterns that are NaNs, infinities, zeros and extremes as `f64`.
const FIXED: [u64; 9] = [
    0,
    u64::MAX,
    0x7ff0_0000_0000_0001,
    0x7ff8_0000_0000_0000,
    0xfff4_dead_beef_0001,
    0x8000_0000_0000_0000,
    1,
    0x7ff0_0000_0000_0000,
    0xfff0_0000_0000_0000,
];

proptest! {
    #[test]
    fn every_bit_pattern_survives_the_arena(
        words in prop::collection::vec(any::<u64>(), 0..=40),
    ) {
        let words: Vec<u64> = FIXED.iter().copied().chain(words).collect();
        let mut buf = vec![0.0; 256];
        let mut scratch = Scratch::borrowed(&mut buf);
        let span = scratch.push_bits(&words).unwrap();
        // Another write above it, then read back.
        let other = scratch.push_bits(&words).unwrap();
        for (i, &w) in words.iter().enumerate() {
            prop_assert_eq!(scratch.bits(span, i), w);
            prop_assert_eq!(scratch.bits(other, i), w);
        }
    }
}

#[cfg(feature = "alloc")]
#[test]
fn bit_patterns_survive_growing_the_arena() {
    let mut scratch = Scratch::growable(1);
    let span = scratch.push_bits(&FIXED).unwrap();
    let big: Vec<u64> = (0..5000u64)
        .map(|i| i.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 0x7ff0_0000_0000_0000)
        .collect();
    let later = scratch.push_bits(&big).unwrap();
    assert!(scratch.capacity() >= FIXED.len() + big.len());
    for (i, &w) in FIXED.iter().enumerate() {
        assert_eq!(scratch.bits(span, i), w);
    }
    for (i, &w) in big.iter().enumerate() {
        assert_eq!(scratch.bits(later, i), w);
    }
}
