//! Checks and exact values of big integers, read from their stored words
//! without the arithmetic under test.

use implicit_predicates_core::{Big, Scratch, ScratchExhausted};
use implicit_predicates_oracle::Dyadic;
use proptest::prelude::*;

/// A description of a big integer: sign, words (least significant first,
/// possibly with zero words at either end) and exponent. [`build`](Self::build)
/// stores it normalized.
#[derive(Clone, Debug)]
pub struct BigSpec {
    /// The sign flag.
    pub negative: bool,
    /// The words of the magnitude, least significant first.
    pub words: Vec<u64>,
    /// The exponent, in words.
    pub exp: i32,
}

impl BigSpec {
    /// Stores the integer in `s`.
    pub fn build(&self, s: &mut Scratch<'_>) -> Result<Big, ScratchExhausted> {
        s.big_from_words(self.negative, &self.words, self.exp)
    }
}

/// Integers of 1 to 6 words, with words that favor the edges that carry and
/// borrow (zero, one, all ones) and exponents in -40..40.
pub fn any_big() -> impl Strategy<Value = BigSpec> {
    let word = prop_oneof![
        Just(0u64),
        Just(1),
        Just(u64::MAX),
        Just(1 << 63),
        any::<u64>()
    ];
    (
        any::<bool>(),
        prop::collection::vec(word, 1..=6),
        -40i32..40,
    )
        .prop_map(|(negative, words, exp)| BigSpec {
            negative,
            words,
            exp,
        })
}

/// The exact value of `b`, from its words and exponent.
pub fn big_to_dyadic(s: &Scratch<'_>, b: Big) -> Dyadic {
    let words: Vec<u64> = (0..b.len()).map(|i| s.big_limb(b, i)).collect();
    Dyadic::from_words(
        b.sign() == implicit_predicates_core::Sign::Negative,
        &words,
        i64::from(b.exponent()) * 64,
    )
}

/// Panics unless `b` is in its unique representation: the lowest and highest
/// words of a nonzero magnitude are not zero, and zero has no words, a
/// positive sign and exponent zero.
pub fn assert_valid_big(s: &Scratch<'_>, b: Big) {
    if b.is_zero() {
        assert_eq!(b.len(), 0, "zero has no words");
        assert_eq!(b.exponent(), 0, "zero has exponent zero");
        assert!(!b.is_negative(), "zero is not negative");
        assert_eq!(
            b.sign(),
            implicit_predicates_core::Sign::Zero,
            "zero has no sign"
        );
        assert_eq!(b.neg().sign(), implicit_predicates_core::Sign::Zero);
        assert!(
            !b.neg().is_negative(),
            "the negation of zero is not negative"
        );
        return;
    }
    assert_ne!(s.big_limb(b, 0), 0, "the lowest word is zero");
    assert_ne!(s.big_limb(b, b.len() - 1), 0, "the highest word is zero");
}
