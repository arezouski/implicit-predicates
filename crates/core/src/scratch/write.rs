//! Writing an expansion into an arena through a closure.

use super::{Scratch, ScratchExhausted, Span};
use crate::expansion;

impl Scratch<'_> {
    /// Lends `capacity` free components to `f`, which writes an expansion
    /// and returns its length; the top is left right after that length.
    ///
    /// This is how code outside the arena writes a result of its own, such
    /// as the exact coordinates of a point, without building it in a
    /// separate buffer first. `f` receives the `capacity` components just
    /// above the top, whose contents are unspecified, writes the expansion
    /// at their start and may use the rest as working space. The returned
    /// span holds the first components `f` reports; `f` must not report
    /// more than `capacity`.
    ///
    /// Requirement: what `f` writes is a valid expansion (see
    /// [`expansion`]); debug builds panic if it is not. Needs `capacity`
    /// free components, which [`high_water`](Self::high_water) counts even
    /// when the expansion turns out shorter.
    ///
    /// # Errors
    ///
    /// [`ScratchExhausted`] if `capacity` components are not free; `f` is
    /// not called and the arena is unchanged.
    ///
    /// # Panics
    ///
    /// If `f` returns a length greater than `capacity`. The top of the arena
    /// and the spans below it are then as they were before the call, but
    /// [`high_water`](Self::high_water) already counts the `capacity`
    /// components reserved, and a growable arena may already have grown.
    ///
    /// # Examples
    ///
    /// ```
    /// use implicit_predicates_core::{Scratch, eft};
    ///
    /// let mut buf = [0.0; 4];
    /// let mut scratch = Scratch::borrowed(&mut buf);
    /// // The exact sum of 1 and 2^-60, as an expansion of two components.
    /// let s = scratch.write(2, |out| {
    ///     let (high, low) = eft::two_sum(1.0, 2f64.powi(-60));
    ///     out[0] = low;
    ///     out[1] = high;
    ///     2
    /// })?;
    /// assert_eq!(scratch.get(s), &[2f64.powi(-60), 1.0]);
    /// # Ok::<(), implicit_predicates_core::ScratchExhausted>(())
    /// ```
    pub fn write(
        &mut self,
        capacity: usize,
        f: impl FnOnce(&mut [f64]) -> usize,
    ) -> Result<Span, ScratchExhausted> {
        let needed = u64::try_from(capacity).unwrap_or(u64::MAX);
        self.write_result([], needed, |[], out| {
            let len = f(out);
            assert!(
                len <= capacity,
                "the closure given to Scratch::write returned a length of {len}, \
                 more than the capacity of {capacity}"
            );
            debug_assert!(
                expansion::is_valid(&out[..len]),
                "Scratch::write of {:?}, which is not a valid expansion",
                &out[..len]
            );
            len
        })
    }
}
