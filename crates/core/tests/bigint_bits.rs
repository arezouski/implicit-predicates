//! Arbitrary 64-bit words survive being stored in the `f64` arena.

use implicit_predicates_core::{Scratch, ScratchExhausted};
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

/// The words without zeros at either end.
fn normalized(words: &[u64]) -> Vec<u64> {
    let low = words.iter().take_while(|&&w| w == 0).count();
    let high = words.len() - words.iter().rev().take_while(|&&w| w == 0).count();
    if low >= high {
        Vec::new()
    } else {
        words[low..high].to_vec()
    }
}

proptest! {
    #[test]
    fn every_bit_pattern_survives_the_arena(
        words in prop::collection::vec(any::<u64>(), 0..=40),
    ) {
        let words: Vec<u64> = FIXED.iter().copied().chain(words).collect();
        let mut buf = vec![0.0; 256];
        let mut scratch = Scratch::borrowed(&mut buf);
        let expected = normalized(&words);
        let first = scratch.big_from_words(false, &words, 0).unwrap();
        // Another write above it, then read back.
        let other = scratch.big_from_words(true, &words, 0).unwrap();
        for big in [first, other] {
            prop_assert_eq!(big.len(), expected.len());
            for (i, &w) in expected.iter().enumerate() {
                prop_assert_eq!(scratch.big_limb(big, i), w);
            }
        }
    }
}

#[cfg(feature = "alloc")]
#[test]
fn bit_patterns_survive_growing_the_arena() {
    let mut scratch = Scratch::growable(1);
    let first = scratch.big_from_words(false, &FIXED, 0).unwrap();
    let big: Vec<u64> = (0..5000u64)
        .map(|i| i.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 0x7ff0_0000_0000_0000)
        .collect();
    let later = scratch.big_from_words(false, &big, 0).unwrap();
    assert!(scratch.capacity() >= FIXED.len() - 1 + big.len());
    for (b, words) in [(first, &FIXED[..]), (later, &big[..])] {
        let expected = normalized(words);
        assert_eq!(b.len(), expected.len());
        for (i, &w) in expected.iter().enumerate() {
            assert_eq!(scratch.big_limb(b, i), w);
        }
    }
}

#[test]
fn an_arena_without_room_for_the_words_is_left_as_it_was() {
    let mut buf = [0.0; 2];
    let mut scratch = Scratch::borrowed(&mut buf);
    let kept = scratch.big_from_words(false, &[u64::MAX], 0).unwrap();
    let r = scratch.big_from_words(false, &[1, 2], 0);
    assert_eq!(r.unwrap_err(), ScratchExhausted);
    assert_eq!(scratch.used(), 1);
    assert_eq!(scratch.big_limb(kept, 0), u64::MAX);
}
