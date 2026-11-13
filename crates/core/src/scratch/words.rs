//! 64-bit words stored in the arena as the bit patterns of `f64` values.
//!
//! The arena is a buffer of `f64`, but moving, copying and storing an `f64`
//! preserves its bits, including those of NaNs, and `f64::from_bits` and
//! `f64::to_bits` are exact transmutations. A word written with `from_bits`
//! therefore reads back identical with `to_bits`, as long as nothing in
//! between does arithmetic on it as a number.

use super::{Scratch, ScratchExhausted, Span};

impl Scratch<'_> {
    /// Returns the components at `s` as stored, which are the bit patterns
    /// of words written with [`write_words`](Self::write_words). Read each
    /// with `f64::to_bits`; never use one as a number.
    pub(crate) fn words(&self, s: Span) -> &[f64] {
        self.get(s)
    }

    /// Copies `bits` into the arena as words and returns their span. Only
    /// for the tests of the arena itself.
    ///
    /// # Errors
    ///
    /// [`ScratchExhausted`] if they do not fit; the arena is unchanged.
    #[doc(hidden)]
    pub fn push_bits(&mut self, bits: &[u64]) -> Result<Span, ScratchExhausted> {
        let needed = u64::try_from(bits.len()).unwrap_or(u64::MAX);
        self.write_words([], needed, |[], out| {
            for (slot, &word) in out.iter_mut().zip(bits) {
                *slot = f64::from_bits(word);
            }
            bits.len()
        })
    }

    /// Returns word `i` of the span `s`. Only for the tests of the arena
    /// itself.
    #[doc(hidden)]
    #[must_use]
    pub fn bits(&self, s: Span, i: usize) -> u64 {
        self.words(s)[i].to_bits()
    }
}
