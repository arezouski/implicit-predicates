//! Reserving space above the top of an arena and writing results into it.

use super::span::to_index;
use super::{Scratch, ScratchExhausted, Span};

impl Scratch<'_> {
    /// The single path of every operation that writes a result: reserves
    /// `needed` components above the top, calls `write` with the operands
    /// and that reserved region, and raises the top by the length `write`
    /// returns. `write` leaves its result at the start of the region and may
    /// use the rest of it as working space.
    ///
    /// On [`ScratchExhausted`] nothing changes. The operands lie below the
    /// top and the region above it, so they never alias.
    ///
    /// # Panics
    ///
    /// If `write` returns a length longer than the region.
    pub(super) fn write_result<const N: usize>(
        &mut self,
        operands: [Span; N],
        needed: u64,
        write: impl FnOnce([&[f64]; N], &mut [f64]) -> usize,
    ) -> Result<Span, ScratchExhausted> {
        self.write_words(operands, needed, write)
    }

    /// The reservation path itself, as [`write_result`](Self::write_result)
    /// describes it. Nothing here treats the components as numbers, so the
    /// result may hold 64-bit words stored as bit patterns.
    pub(crate) fn write_words<const N: usize>(
        &mut self,
        operands: [Span; N],
        needed: u64,
        write: impl FnOnce([&[f64]; N], &mut [f64]) -> usize,
    ) -> Result<Span, ScratchExhausted> {
        for operand in operands {
            self.debug_check(operand);
        }
        let start = self.used;
        let end = self.reserve(needed)?;
        let (below, reserved) =
            self.storage.as_mut_slice()[..to_index(end)].split_at_mut(to_index(start));
        let len = write(operands.map(|operand| &below[operand.range()]), reserved);
        assert!(len <= reserved.len(), "a result exceeded its reservation");
        let len = u32::try_from(len).expect("a result fits in its reservation, below u32::MAX");
        self.used = start + len;
        Ok(Span { start, len })
    }

    /// Makes `needed` components above the top available and returns the
    /// end of that region, recording it in the high-water mark. Fails,
    /// changing nothing, if the region would end past the capacity or past
    /// `u32::MAX`.
    fn reserve(&mut self, needed: u64) -> Result<u32, ScratchExhausted> {
        let end = u64::from(self.used)
            .checked_add(needed)
            .ok_or(ScratchExhausted)?;
        let end = u32::try_from(end).map_err(|_| ScratchExhausted)?;
        self.storage.make_available(end)?;
        self.high_water = self.high_water.max(end);
        Ok(end)
    }

    /// Panics in debug builds if `s` reaches past the top.
    pub(super) fn debug_check(&self, s: Span) {
        debug_assert!(
            u64::from(s.start) + u64::from(s.len) <= u64::from(self.used),
            "{s:?} reaches past the top at {}: it was released by a reset or \
             belongs to another arena",
            self.used
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{Scratch, ScratchExhausted};

    #[test]
    fn reservations_past_u64_max_fail_without_changes() {
        let mut buf = [0.0; 4];
        let mut s = Scratch::borrowed(&mut buf);
        let _ = s.push(&[1.0]).unwrap();
        assert_eq!(s.reserve(u64::MAX), Err(ScratchExhausted));
        assert_eq!(s.reserve(u64::from(u32::MAX)), Err(ScratchExhausted));
        assert_eq!((s.used(), s.high_water()), (1, 1));
        assert_eq!(s.reserve(3), Ok(4));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn growable_reservations_past_u32_max_fail_before_allocating() {
        let mut s = Scratch::growable(2);
        let _ = s.push(&[1.0]).unwrap();
        assert_eq!(s.reserve(u64::from(u32::MAX)), Err(ScratchExhausted));
        assert_eq!(s.reserve(u64::MAX), Err(ScratchExhausted));
        assert_eq!((s.used(), s.capacity(), s.high_water()), (1, 2, 1));
    }
}
