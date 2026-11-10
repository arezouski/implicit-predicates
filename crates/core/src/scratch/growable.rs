//! The arena that owns its memory and grows on the heap.

use alloc::vec;

use super::Scratch;
use super::span::to_index;
use super::storage::{Storage, clamp_capacity};

impl Scratch<'static> {
    /// Creates an empty arena that owns its memory, with room for `initial`
    /// components (at most `u32::MAX`, the most an arena uses).
    ///
    /// When an operation needs more room the arena grows on the heap to at
    /// least double its capacity, or to what the operation needs if that is
    /// more, keeping its contents. Spans are indices, so they stay valid
    /// after the buffer moves. The arena fails with
    /// [`ScratchExhausted`](super::ScratchExhausted) only when an operation
    /// needs more than `u32::MAX` components, and then it is unchanged.
    ///
    /// Running out of memory aborts, as for any `alloc` collection;
    /// `ScratchExhausted` is returned only when the arena would exceed
    /// `u32::MAX` components.
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
        // Components past `u32::MAX` could never be used, so never allocate them.
        let initial = to_index(clamp_capacity(initial));
        Scratch {
            storage: Storage::Owned(vec![0.0; initial]),
            used: 0,
            high_water: 0,
        }
    }
}
