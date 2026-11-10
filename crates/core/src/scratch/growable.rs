//! The arena that owns its memory and grows on the heap.

use alloc::vec::Vec;

use super::Scratch;
use super::storage::Storage;

impl Scratch<'static> {
    /// Creates an empty arena that owns its memory, with room for `initial`
    /// components (at most `u32::MAX` of them are used).
    ///
    /// When an operation needs more room the arena grows on the heap to at
    /// least double its capacity, or to what the operation needs if that is
    /// more, keeping its contents. Spans are indices, so they stay valid
    /// after the buffer moves. The arena fails with
    /// [`ScratchExhausted`](super::ScratchExhausted) only when an operation
    /// needs more than `u32::MAX` components, and then it is unchanged.
    ///
    /// Available only with the `alloc` cargo feature.
    ///
    /// # Examples
    ///
    /// ```
    /// use indirect_predicates_core::Scratch;
    ///
    /// let mut scratch = Scratch::growable(1);
    /// let a = scratch.push(&[1e-20, 1.0])?; // grows
    /// assert!(scratch.capacity() >= 2);
    /// assert_eq!(scratch.get(a), &[1e-20, 1.0]);
    /// # Ok::<(), indirect_predicates_core::ScratchExhausted>(())
    /// ```
    #[must_use]
    pub fn growable(initial: usize) -> Scratch<'static> {
        let mut buffer = Vec::new();
        buffer.resize(initial, 0.0);
        Scratch {
            storage: Storage::Owned(buffer),
            used: 0,
            high_water: 0,
        }
    }
}
