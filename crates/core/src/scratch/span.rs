//! Positions in an arena: spans of expansions and marks of the top.

use core::ops::Range;

/// The position of an expansion in a [`Scratch`](super::Scratch) arena.
///
/// Spans compare equal, and hash alike, when they have the same position and
/// length, whatever their contents. A span is meaningful only for the arena that created it;
/// see the [module documentation](crate::scratch).
///
/// # Examples
///
/// ```
/// use implicit_predicates_core::Scratch;
///
/// let mut buf = [0.0; 4];
/// let mut scratch = Scratch::borrowed(&mut buf);
/// let span = scratch.push(&[1e-20, 1.0])?;
/// assert_eq!(span.len(), 2);
/// assert_eq!(scratch.get(span), &[1e-20, 1.0]);
/// # Ok::<(), implicit_predicates_core::ScratchExhausted>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    pub(super) start: u32,
    pub(super) len: u32,
}

/// A saved top of a [`Scratch`](super::Scratch) arena, to return to with
/// [`Scratch::reset`](super::Scratch::reset). Marks compare equal when they
/// save the same top.
///
/// # Examples
///
/// ```
/// use implicit_predicates_core::Scratch;
///
/// let mut buf = [0.0; 4];
/// let mut scratch = Scratch::borrowed(&mut buf);
/// let mark = scratch.mark();
/// scratch.push(&[1.0])?;
/// assert_ne!(scratch.mark(), mark);
/// scratch.reset(mark);
/// assert_eq!(scratch.mark(), mark);
/// assert_eq!(scratch.used(), 0);
/// # Ok::<(), implicit_predicates_core::ScratchExhausted>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mark(pub(super) u32);

impl Span {
    /// Returns the number of components of the expansion.
    ///
    /// # Examples
    ///
    /// ```
    /// use implicit_predicates_core::Scratch;
    ///
    /// let mut buf = [0.0; 2];
    /// let mut scratch = Scratch::borrowed(&mut buf);
    /// assert_eq!(scratch.push(&[1e-20, 1.0])?.len(), 2);
    /// # Ok::<(), implicit_predicates_core::ScratchExhausted>(())
    /// ```
    #[must_use]
    pub fn len(self) -> usize {
        to_index(self.len)
    }

    /// Returns `true` if the expansion has no components, that is, if its
    /// value is zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use implicit_predicates_core::Scratch;
    ///
    /// let mut buf = [0.0; 6];
    /// let mut scratch = Scratch::borrowed(&mut buf);
    /// let a = scratch.push(&[1e-20, 1.0])?;
    /// assert!(scratch.diff(a, a)?.is_empty());
    /// # Ok::<(), implicit_predicates_core::ScratchExhausted>(())
    /// ```
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.len == 0
    }

    /// The indices of the components in the arena.
    pub(super) fn range(self) -> Range<usize> {
        let start = to_index(self.start);
        start..start + to_index(self.len)
    }
}

/// Converts an index within the arena, which never exceeds the length of
/// its buffer, to `usize`.
pub(super) fn to_index(i: u32) -> usize {
    usize::try_from(i).expect("arena indices never exceed the buffer length")
}
