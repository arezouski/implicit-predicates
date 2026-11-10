//! A bump arena for expansion arithmetic.
//!
//! A [`Scratch`] arena hands out the memory for the expansions that a
//! computation builds step by step. Its memory is a buffer the caller lends
//! it ([`Scratch::borrowed`]), so nothing allocates. With the `alloc` feature
//! an arena can instead own its memory and grow on the heap
//! (`Scratch::growable`).
//!
//! # Model
//!
//! - **Bump allocation.** The arena keeps a *top*: the buffer below it is in
//!   use, the rest is free. Each operation writes its result just above the
//!   top and raises the top by the length of that result.
//! - **Spans are indices.** Operations take and return [`Span`]s, positions
//!   in the arena rather than references to it, so a computation can hold
//!   many results while it keeps calling `&mut self` operations. Read the
//!   components of a span with [`Scratch::get`].
//! - **Marks and reset.** [`Scratch::mark`] records the current top and
//!   [`Scratch::reset`] lowers the top back to a mark, releasing at once
//!   every result built since. Spans created before the mark stay valid.
//! - **Exhaustion.** Before it writes, an operation reserves all the space
//!   it may need above the top: the longest result it can produce (the
//!   capacity documented by the slice primitive in [`expansion`]), plus, for
//!   [`Scratch::product`], room for its intermediate expansions. If that does
//!   not fit, the operation returns [`ScratchExhausted`] and leaves the arena
//!   unchanged. [`Scratch::high_water`] tells how much of the buffer a
//!   computation needed, which is how to size the buffer.
//!
//! An arena uses at most `u32::MAX` components of its buffer. An arena that
//! grows fails with [`ScratchExhausted`] only when an operation needs more
//! than that.
//!
//! A span means something only to the arena that created it, and only while
//! the top stays above it. Using a span after a [`reset`](Scratch::reset)
//! past its start, or with another arena, is a logic error: debug builds
//! detect it when the span reaches past the top, and the result is otherwise
//! unspecified (a panic or unrelated components), but never undefined
//! behavior.
//!
//! # Examples
//!
//! ```
//! use indirect_predicates_core::{Scratch, Sign};
//!
//! let mut buf = [0.0; 16];
//! let mut scratch = Scratch::borrowed(&mut buf);
//! let a = scratch.push(&[1e-20, 1.0])?;
//! let b = scratch.push(&[3.0])?;
//!
//! let mark = scratch.mark();
//! let sum = scratch.sum(a, b)?; // 1 + 3 + 1e-20
//! let scaled = scratch.scale(sum, -2.0)?;
//! assert_eq!(scratch.get(scaled), &[-2e-20, -8.0]);
//! assert_eq!(scratch.sign(scaled), Sign::Negative);
//!
//! // Release `sum` and `scaled`; `a` and `b` stay valid.
//! scratch.reset(mark);
//! assert_eq!(scratch.used(), 3);
//! assert_eq!(scratch.get(a), &[1e-20, 1.0]);
//! # Ok::<(), indirect_predicates_core::ScratchExhausted>(())
//! ```

mod error;
#[cfg(feature = "alloc")]
mod growable;
mod product;
mod reservation;
mod span;
mod storage;

pub use error::ScratchExhausted;
pub use span::{Mark, Span};

use core::fmt;

use crate::{Sign, expansion};
use span::to_index;
use storage::Storage;

/// A bump arena of `f64` components for expansion arithmetic. See the
/// [module documentation](self) for the model.
pub struct Scratch<'a> {
    storage: Storage<'a>,
    /// The top: components `..used` are in use.
    used: u32,
    /// The highest the top has been, counting worst-case reservations.
    high_water: u32,
}

impl<'a> Scratch<'a> {
    /// Creates an empty arena over `buf`, of which it uses at most
    /// `u32::MAX` components. The initial contents of `buf` do not matter.
    ///
    /// # Examples
    ///
    /// ```
    /// use indirect_predicates_core::Scratch;
    ///
    /// let mut buf = [0.0; 64];
    /// let scratch = Scratch::borrowed(&mut buf);
    /// assert_eq!(scratch.capacity(), 64);
    /// assert_eq!(scratch.used(), 0);
    /// ```
    #[must_use]
    pub fn borrowed(buf: &'a mut [f64]) -> Scratch<'a> {
        Scratch {
            storage: Storage::Borrowed(buf),
            used: 0,
            high_water: 0,
        }
    }

    /// Returns a mark of the current top, to return to with
    /// [`reset`](Self::reset).
    ///
    /// # Examples
    ///
    /// ```
    /// use indirect_predicates_core::Scratch;
    ///
    /// let mut buf = [0.0; 4];
    /// let mut scratch = Scratch::borrowed(&mut buf);
    /// scratch.push(&[1.0])?;
    /// let mark = scratch.mark();
    /// scratch.push(&[2.0])?;
    /// scratch.reset(mark);
    /// assert_eq!(scratch.used(), 1);
    /// # Ok::<(), indirect_predicates_core::ScratchExhausted>(())
    /// ```
    #[must_use]
    pub fn mark(&self) -> Mark {
        Mark(self.used)
    }

    /// Lowers the top to the mark `m`, releasing every span created since
    /// `m` was taken. [`high_water`](Self::high_water) is unchanged.
    ///
    /// `m` must not be above the current top, which happens when a reset
    /// to an earlier mark came in between. That is a logic error: debug
    /// builds panic, release builds leave the top unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use indirect_predicates_core::Scratch;
    ///
    /// let mut buf = [0.0; 4];
    /// let mut scratch = Scratch::borrowed(&mut buf);
    /// let mark = scratch.mark();
    /// scratch.push(&[1e-20, 1.0])?;
    /// scratch.reset(mark);
    /// assert_eq!(scratch.used(), 0);
    /// assert_eq!(scratch.high_water(), 2);
    /// # Ok::<(), indirect_predicates_core::ScratchExhausted>(())
    /// ```
    pub fn reset(&mut self, m: Mark) {
        debug_assert!(
            m.0 <= self.used,
            "reset to a mark at {} above the top at {}",
            m.0,
            self.used
        );
        self.used = self.used.min(m.0);
    }

    /// Returns the components of the expansion at `s`.
    ///
    /// Debug builds panic if `s` reaches past the top; see the
    /// [module documentation](self) for spans that are no longer valid.
    ///
    /// # Examples
    ///
    /// ```
    /// use indirect_predicates_core::Scratch;
    ///
    /// let mut buf = [0.0; 4];
    /// let mut scratch = Scratch::borrowed(&mut buf);
    /// let s = scratch.push(&[1e-20, 1.0])?;
    /// assert_eq!(scratch.get(s), &[1e-20, 1.0]);
    /// # Ok::<(), indirect_predicates_core::ScratchExhausted>(())
    /// ```
    #[must_use]
    pub fn get(&self, s: Span) -> &[f64] {
        self.debug_check(s);
        &self.storage.as_slice()[..self.used()][s.range()]
    }

    /// Returns the number of components in use, below the top.
    ///
    /// # Examples
    ///
    /// ```
    /// use indirect_predicates_core::Scratch;
    ///
    /// let mut buf = [0.0; 4];
    /// let mut scratch = Scratch::borrowed(&mut buf);
    /// scratch.push(&[1e-20, 1.0])?;
    /// assert_eq!(scratch.used(), 2);
    /// # Ok::<(), indirect_predicates_core::ScratchExhausted>(())
    /// ```
    #[must_use]
    pub fn used(&self) -> usize {
        to_index(self.used)
    }

    /// Returns the number of components the arena can hold: the length of
    /// its buffer, or `u32::MAX` if the buffer is longer.
    ///
    /// # Examples
    ///
    /// ```
    /// use indirect_predicates_core::Scratch;
    ///
    /// let mut buf = [0.0; 8];
    /// assert_eq!(Scratch::borrowed(&mut buf).capacity(), 8);
    /// ```
    #[must_use]
    pub fn capacity(&self) -> usize {
        to_index(self.storage.capacity())
    }

    /// Returns the highest [`used`](Self::used) ever reached, counting the
    /// space each successful operation reserves while it works, even when
    /// its result turns out shorter. A buffer this long is enough
    /// to repeat the same computation. Failed operations reserve nothing.
    ///
    /// # Examples
    ///
    /// ```
    /// use indirect_predicates_core::Scratch;
    ///
    /// let mut buf = [0.0; 8];
    /// let mut scratch = Scratch::borrowed(&mut buf);
    /// let a = scratch.push(&[1e-20, 1.0])?;
    /// let zero = scratch.diff(a, a)?; // reserves 4 components, keeps none
    /// assert!(zero.is_empty());
    /// assert_eq!(scratch.used(), 2);
    /// assert_eq!(scratch.high_water(), 6);
    /// # Ok::<(), indirect_predicates_core::ScratchExhausted>(())
    /// ```
    #[must_use]
    pub fn high_water(&self) -> usize {
        to_index(self.high_water)
    }

    /// Copies the expansion `e` into the arena and returns its span.
    ///
    /// Needs `e.len()` free components; an empty `e` always succeeds, giving
    /// an empty span. Requirement: `e` is a valid expansion (see
    /// [`expansion`]); it is copied as is, and debug builds panic if it is
    /// not valid.
    ///
    /// # Errors
    ///
    /// [`ScratchExhausted`] if `e` does not fit; the arena is unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use indirect_predicates_core::Scratch;
    ///
    /// let mut buf = [0.0; 2];
    /// let mut scratch = Scratch::borrowed(&mut buf);
    /// let s = scratch.push(&[1e-20, 1.0])?;
    /// assert_eq!(scratch.get(s), &[1e-20, 1.0]);
    /// # Ok::<(), indirect_predicates_core::ScratchExhausted>(())
    /// ```
    pub fn push(&mut self, e: &[f64]) -> Result<Span, ScratchExhausted> {
        debug_assert!(
            expansion::is_valid(e),
            "Scratch::push of {e:?}, which is not a valid expansion"
        );
        let worst_case = u64::try_from(e.len()).unwrap_or(u64::MAX);
        self.write_result([], worst_case, |[], out| {
            out.copy_from_slice(e);
            e.len()
        })
    }

    /// Writes the expansion of `a + b` and returns its span, as
    /// [`expansion::sum`] does.
    ///
    /// Needs `a.len() + b.len()` free components while it works.
    ///
    /// # Errors
    ///
    /// [`ScratchExhausted`] if that space is not free; the arena is
    /// unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use indirect_predicates_core::Scratch;
    ///
    /// let mut buf = [0.0; 8];
    /// let mut scratch = Scratch::borrowed(&mut buf);
    /// let a = scratch.push(&[1e-20, 1.0])?;
    /// let b = scratch.push(&[-1e-20, 2.0])?;
    /// let s = scratch.sum(a, b)?;
    /// assert_eq!(scratch.get(s), &[3.0]);
    /// # Ok::<(), indirect_predicates_core::ScratchExhausted>(())
    /// ```
    pub fn sum(&mut self, a: Span, b: Span) -> Result<Span, ScratchExhausted> {
        let worst_case = u64::from(a.len) + u64::from(b.len);
        self.write_result([a, b], worst_case, |[a, b], out| expansion::sum(a, b, out))
    }

    /// Writes the expansion of `a - b` and returns its span, as
    /// [`expansion::diff`] does.
    ///
    /// Needs `a.len() + b.len()` free components while it works.
    ///
    /// # Errors
    ///
    /// [`ScratchExhausted`] if that space is not free; the arena is
    /// unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use indirect_predicates_core::Scratch;
    ///
    /// let mut buf = [0.0; 8];
    /// let mut scratch = Scratch::borrowed(&mut buf);
    /// let a = scratch.push(&[1e-20, 1.0])?;
    /// let b = scratch.push(&[1e-20, 2.0])?;
    /// let d = scratch.diff(a, b)?;
    /// assert_eq!(scratch.get(d), &[-1.0]);
    /// # Ok::<(), indirect_predicates_core::ScratchExhausted>(())
    /// ```
    pub fn diff(&mut self, a: Span, b: Span) -> Result<Span, ScratchExhausted> {
        let worst_case = u64::from(a.len) + u64::from(b.len);
        self.write_result([a, b], worst_case, |[a, b], out| expansion::diff(a, b, out))
    }

    /// Writes the expansion of `a * b` and returns its span, as
    /// [`expansion::scale`] does, with the same requirements on the
    /// magnitudes.
    ///
    /// Needs `2 * a.len()` free components while it works.
    ///
    /// # Errors
    ///
    /// [`ScratchExhausted`] if that space is not free; the arena is
    /// unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use indirect_predicates_core::Scratch;
    ///
    /// let mut buf = [0.0; 8];
    /// let mut scratch = Scratch::borrowed(&mut buf);
    /// let a = scratch.push(&[0.25, 4.0])?;
    /// let s = scratch.scale(a, 3.0)?;
    /// assert_eq!(scratch.get(s), &[12.75]);
    /// # Ok::<(), indirect_predicates_core::ScratchExhausted>(())
    /// ```
    pub fn scale(&mut self, a: Span, b: f64) -> Result<Span, ScratchExhausted> {
        let worst_case = 2 * u64::from(a.len);
        self.write_result([a], worst_case, |[a], out| expansion::scale(a, b, out))
    }

    /// Returns the exact sign of the expansion at `a`, as
    /// [`expansion::sign`] does.
    ///
    /// # Examples
    ///
    /// ```
    /// use indirect_predicates_core::{Scratch, Sign};
    ///
    /// let mut buf = [0.0; 2];
    /// let mut scratch = Scratch::borrowed(&mut buf);
    /// let a = scratch.push(&[1e-20, -1.0])?;
    /// assert_eq!(scratch.sign(a), Sign::Negative);
    /// # Ok::<(), indirect_predicates_core::ScratchExhausted>(())
    /// ```
    #[must_use]
    pub fn sign(&self, a: Span) -> Sign {
        expansion::sign(self.get(a))
    }
}

/// Shows the accounting of the arena, not its contents.
impl fmt::Debug for Scratch<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Scratch")
            .field("used", &self.used)
            .field("capacity", &self.storage.capacity())
            .field("high_water", &self.high_water)
            .finish()
    }
}
