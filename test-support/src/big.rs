//! Checks and exact values of big integers, read from their stored words
//! without the arithmetic under test.

use implicit_predicates_core::{Big, Scratch};
use implicit_predicates_oracle::Dyadic;

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
        assert_eq!(
            b.sign(),
            implicit_predicates_core::Sign::Zero,
            "zero has no sign"
        );
        assert_eq!(b.neg().sign(), implicit_predicates_core::Sign::Zero);
        return;
    }
    assert_ne!(s.big_limb(b, 0), 0, "the lowest word is zero");
    assert_ne!(s.big_limb(b, b.len() - 1), 0, "the highest word is zero");
}
