//! The error of an arena operation that does not fit.

use core::fmt;

/// The error of a [`Scratch`](super::Scratch) operation whose worst-case result does not fit
/// in the free part of the arena.
///
/// # Examples
///
/// ```
/// use indirect_predicates_core::{Scratch, ScratchExhausted};
///
/// let mut buf = [0.0; 1];
/// let mut scratch = Scratch::borrowed(&mut buf);
/// assert_eq!(scratch.push(&[1e-20, 1.0]), Err(ScratchExhausted));
/// assert_eq!(ScratchExhausted.to_string(), "scratch arena exhausted");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScratchExhausted;

impl fmt::Display for ScratchExhausted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("scratch arena exhausted")
    }
}

impl core::error::Error for ScratchExhausted {}
