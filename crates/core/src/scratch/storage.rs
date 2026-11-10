//! The memory behind an arena.

use super::ScratchExhausted;

/// The memory behind an arena.
pub(super) enum Storage<'a> {
    /// A buffer lent by the caller, of which at most `u32::MAX` components
    /// are used.
    Borrowed(&'a mut [f64]),
}

impl Storage<'_> {
    /// The number of components the arena may use: the buffer length, at
    /// most `u32::MAX`.
    pub(super) fn capacity(&self) -> u32 {
        clamp_capacity(self.as_slice().len())
    }

    /// Succeeds if the first `len` components are available.
    pub(super) fn make_available(&mut self, len: u32) -> Result<(), ScratchExhausted> {
        if len <= self.capacity() {
            Ok(())
        } else {
            Err(ScratchExhausted)
        }
    }

    /// The whole buffer.
    pub(super) fn as_slice(&self) -> &[f64] {
        match self {
            Storage::Borrowed(buffer) => buffer,
        }
    }

    /// The whole buffer, mutably.
    pub(super) fn as_mut_slice(&mut self) -> &mut [f64] {
        match self {
            Storage::Borrowed(buffer) => buffer,
        }
    }
}

/// The number of components an arena uses from a buffer of `len`.
fn clamp_capacity(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::clamp_capacity;

    #[test]
    fn buffers_longer_than_u32_max_are_used_up_to_u32_max() {
        assert_eq!(clamp_capacity(7), 7);
        assert_eq!(clamp_capacity(0xFFFF_FFFF), u32::MAX);
        if let Some(longer) = 0xFFFF_FFFFusize.checked_add(1) {
            assert_eq!(clamp_capacity(longer), u32::MAX);
        }
    }
}
