//! 64-bit words stored in the arena as the bit patterns of `f64` values.
//!
//! The arena is a buffer of `f64`, but moving, copying and storing an `f64`
//! preserves its bits, including those of NaNs, and `f64::from_bits` and
//! `f64::to_bits` are exact transmutations. A word written with `from_bits`
//! therefore reads back identical with `to_bits`, as long as nothing in
//! between does arithmetic on it as a number.

use super::{Scratch, Span};

impl Scratch<'_> {
    /// Returns the components at `s` as stored, which are the bit patterns
    /// of words written with [`write_words`](Self::write_words). Read each
    /// with `f64::to_bits`; never use one as a number.
    pub(crate) fn words(&self, s: Span) -> &[f64] {
        self.get(s)
    }
}
