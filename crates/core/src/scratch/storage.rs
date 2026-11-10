//! The memory behind an arena.

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use super::ScratchExhausted;
#[cfg(feature = "alloc")]
use super::span::to_index;

/// The memory behind an arena.
pub(super) enum Storage<'a> {
    /// A buffer lent by the caller, of which at most `u32::MAX` components
    /// are used.
    Borrowed(&'a mut [f64]),
    /// A buffer the arena owns and grows on the heap. Its length is the
    /// number of components that are available.
    #[cfg(feature = "alloc")]
    Owned(Vec<f64>),
}

impl Storage<'_> {
    /// The number of components the arena may use: the buffer length, at
    /// most `u32::MAX`.
    pub(super) fn capacity(&self) -> u32 {
        clamp_capacity(self.as_slice().len())
    }

    /// Succeeds if the first `len` components are available, growing an
    /// owned buffer if needed. Fails, changing nothing, if they are not.
    pub(super) fn make_available(&mut self, len: u32) -> Result<(), ScratchExhausted> {
        if len <= self.capacity() {
            return Ok(());
        }
        match self {
            Storage::Borrowed(_) => Err(ScratchExhausted),
            #[cfg(feature = "alloc")]
            Storage::Owned(buffer) => {
                let target = to_index(grown_capacity(buffer.len(), len));
                buffer.reserve_exact(target - buffer.len());
                buffer.resize(target, 0.0);
                Ok(())
            }
        }
    }

    /// The whole buffer.
    pub(super) fn as_slice(&self) -> &[f64] {
        match self {
            Storage::Borrowed(buffer) => buffer,
            #[cfg(feature = "alloc")]
            Storage::Owned(buffer) => buffer,
        }
    }

    /// The whole buffer, mutably.
    pub(super) fn as_mut_slice(&mut self) -> &mut [f64] {
        match self {
            Storage::Borrowed(buffer) => buffer,
            #[cfg(feature = "alloc")]
            Storage::Owned(buffer) => buffer,
        }
    }
}

/// The capacity an owned buffer of `current` components grows to when
/// `needed` components must be available: at least double, or `needed` if
/// that is more, but never past `u32::MAX`. Requires `current < needed`.
#[cfg(feature = "alloc")]
fn grown_capacity(current: usize, needed: u32) -> u32 {
    let doubled = u64::try_from(current)
        .unwrap_or(u64::MAX)
        .saturating_mul(2)
        .min(u64::from(u32::MAX));
    // `doubled` is at most `u32::MAX`, so the conversion cannot fail.
    needed.max(u32::try_from(doubled).unwrap_or(u32::MAX))
}

/// The number of components an arena uses from a buffer of `len`.
pub(super) fn clamp_capacity(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::clamp_capacity;
    #[cfg(feature = "alloc")]
    use super::grown_capacity;

    #[test]
    fn buffers_longer_than_u32_max_are_used_up_to_u32_max() {
        assert_eq!(clamp_capacity(7), 7);
        assert_eq!(clamp_capacity(0xFFFF_FFFF), u32::MAX);
        if let Some(longer) = 0xFFFF_FFFFusize.checked_add(1) {
            assert_eq!(clamp_capacity(longer), u32::MAX);
        }
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn growth_doubles_or_takes_what_is_needed_without_passing_u32_max() {
        assert_eq!(grown_capacity(0, 1), 1);
        assert_eq!(grown_capacity(4, 5), 8);
        assert_eq!(grown_capacity(4, 25), 25);
        assert_eq!(grown_capacity(0x8000_0000, 0x8000_0001), u32::MAX);
        assert_eq!(grown_capacity(0xFFFF_FFFE, u32::MAX), u32::MAX);
        assert_eq!(grown_capacity(usize::MAX, u32::MAX), u32::MAX);
    }
}
