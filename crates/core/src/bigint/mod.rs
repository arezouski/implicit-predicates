//! Exact integers of arbitrary size, stored in a [`Scratch`] arena.
//!
//! A [`Big`] is `±magnitude · 2^(64·exponent)`: the magnitude is a string of
//! 64-bit words, least significant first, kept in the arena, and the exponent
//! counts whole words. The representation is unique: neither the lowest nor
//! the highest word of a nonzero magnitude is zero, and zero is the empty
//! magnitude with a positive sign and exponent zero.
//!
//! [`Scratch::big_from_f64`] converts a finite `f64` exactly, and
//! [`Scratch::big_add`], [`Scratch::big_sub`] and [`Scratch::big_mul`] are
//! exact whatever the size of the operands, so intermediate values never
//! overflow or round. Every operation writes its result above the top of
//! the arena and fails with [`ScratchExhausted`] if the words do not fit,
//! leaving the arena as it was; a result that is no longer needed is
//! released with [`Scratch::reset`], like any other span.
//!
//! # Examples
//!
//! The determinant of `[[a, b], [c, d]]` for values whose products overflow
//! `f64`:
//!
//! ```
//! use implicit_predicates_core::{Scratch, Sign};
//!
//! // In `f64` both products are infinite and the difference is NaN.
//! assert!((1e200_f64 * 1e200 - 1e200 * 1e200).is_nan());
//!
//! let mut buf = [0.0; 64];
//! let mut scratch = Scratch::borrowed(&mut buf);
//! let a = scratch.big_from_f64(1e200)?;
//! let d = scratch.big_from_f64(1e200)?;
//! let (b, c) = (a, a);
//! let ad = scratch.big_mul(a, d)?;
//! let bc = scratch.big_mul(b, c)?;
//! // The determinant is exactly zero.
//! assert_eq!(scratch.big_sub(ad, bc)?.sign(), Sign::Zero);
//!
//! // Flip the sign of `d` and the determinant is exactly negative.
//! let ad = scratch.big_mul(a, d.neg())?;
//! assert_eq!(scratch.big_sub(ad, bc)?.sign(), Sign::Negative);
//! # Ok::<(), implicit_predicates_core::ScratchExhausted>(())
//! ```

mod limbs;
mod ops;

use crate::scratch::{Scratch, ScratchExhausted, Span};
use crate::sign::Sign;

/// An exact integer in an arena: `±magnitude · 2^(64·exponent)`.
///
/// Like a [`Span`], it is meaningful only for the arena that created it, and
/// only while the words it describes have not been released with
/// [`Scratch::reset`].
///
/// # Examples
///
/// ```
/// use implicit_predicates_core::{Scratch, Sign};
///
/// let mut buf = [0.0; 4];
/// let mut scratch = Scratch::borrowed(&mut buf);
/// let b = scratch.big_from_f64(-6.0)?;
/// assert_eq!(b.sign(), Sign::Negative);
/// assert_eq!(b.neg().sign(), Sign::Positive);
/// assert_eq!(scratch.big_limb(b, 0), 6);
/// # Ok::<(), implicit_predicates_core::ScratchExhausted>(())
/// ```
#[derive(Clone, Copy, Debug)]
pub struct Big {
    limbs: Span,
    exp: i32,
    negative: bool,
}

impl Big {
    /// The value zero.
    pub const ZERO: Big = Big {
        limbs: Span::EMPTY,
        exp: 0,
        negative: false,
    };

    /// Returns the sign of the value.
    ///
    /// # Examples
    ///
    /// ```
    /// use implicit_predicates_core::{Big, Sign};
    ///
    /// assert_eq!(Big::ZERO.sign(), Sign::Zero);
    /// ```
    #[must_use]
    pub fn sign(self) -> Sign {
        if self.is_zero() {
            Sign::Zero
        } else if self.negative {
            Sign::Negative
        } else {
            Sign::Positive
        }
    }

    /// Returns the raw sign flag, which is `false` for zero. Only for tests
    /// of the representation; use [`sign`](Self::sign).
    #[doc(hidden)]
    #[must_use]
    pub fn is_negative(self) -> bool {
        self.negative
    }

    /// Returns the opposite value, sharing the same words. The negation of
    /// zero is zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use implicit_predicates_core::{Big, Sign};
    ///
    /// assert_eq!(Big::ZERO.neg().sign(), Sign::Zero);
    /// ```
    #[must_use]
    #[allow(clippy::should_implement_trait)] // a plain method keeps the call sites free of imports
    pub fn neg(self) -> Big {
        Big {
            negative: !self.negative && !self.is_zero(),
            ..self
        }
    }

    /// Returns the number of 64-bit words of the magnitude; zero for the
    /// value zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use implicit_predicates_core::Big;
    ///
    /// assert_eq!(Big::ZERO.len(), 0);
    /// ```
    #[must_use]
    #[allow(clippy::len_without_is_empty)] // `is_zero` is the emptiness test
    pub fn len(self) -> usize {
        self.limbs.len()
    }

    /// Returns `true` if the value is zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use implicit_predicates_core::Big;
    ///
    /// assert!(Big::ZERO.is_zero());
    /// ```
    #[must_use]
    pub fn is_zero(self) -> bool {
        self.limbs.is_empty()
    }

    /// Returns the exponent of two, in whole 64-bit words, that scales the
    /// magnitude.
    ///
    /// # Examples
    ///
    /// ```
    /// use implicit_predicates_core::Scratch;
    ///
    /// let mut buf = [0.0; 2];
    /// let mut scratch = Scratch::borrowed(&mut buf);
    /// // 2^64 is one word, the value 1, scaled by 2^(64 * 1).
    /// assert_eq!(scratch.big_from_f64(18446744073709551616.0)?.exponent(), 1);
    /// # Ok::<(), implicit_predicates_core::ScratchExhausted>(())
    /// ```
    #[must_use]
    pub fn exponent(self) -> i32 {
        self.exp
    }
}

impl Scratch<'_> {
    /// Converts a finite `f64` exactly. The result has at most two words.
    ///
    /// # Errors
    ///
    /// [`ScratchExhausted`] if the words do not fit; the arena is unchanged.
    ///
    /// # Panics
    ///
    /// Panics if `x` is NaN or infinite.
    ///
    /// # Examples
    ///
    /// ```
    /// use implicit_predicates_core::{Scratch, Sign};
    ///
    /// let mut buf = [0.0; 2];
    /// let mut scratch = Scratch::borrowed(&mut buf);
    /// let b = scratch.big_from_f64(2.0)?;
    /// assert_eq!(b.sign(), Sign::Positive);
    /// assert_eq!(b.len(), 1);
    /// # Ok::<(), implicit_predicates_core::ScratchExhausted>(())
    /// ```
    pub fn big_from_f64(&mut self, x: f64) -> Result<Big, ScratchExhausted> {
        assert!(x.is_finite(), "big_from_f64: {x} is not finite");
        let bits = x.to_bits();
        let negative = bits >> 63 == 1;
        let biased = i32::try_from((bits >> 52) & 0x7ff).expect("eleven bits fit in i32");
        let fraction = bits & ((1u64 << 52) - 1);
        // x = mantissa * 2^binary_exp, exactly.
        let (mantissa, binary_exp) = if biased == 0 {
            (fraction, -1074)
        } else {
            (fraction | (1u64 << 52), biased - 1075)
        };
        // Split the exponent into whole words and a shift within a word; the
        // shifted mantissa then spans at most two words.
        let word_exp = binary_exp.div_euclid(64);
        let shift = binary_exp.rem_euclid(64).unsigned_abs();
        let shifted = u128::from(mantissa) << shift;
        let words = [shifted as u64, (shifted >> 64) as u64];
        self.big_from_words(negative, &words, word_exp)
    }

    /// Returns word `i` of the magnitude of `b`, counting from the least
    /// significant.
    ///
    /// # Panics
    ///
    /// Panics if `i >= b.len()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use implicit_predicates_core::Scratch;
    ///
    /// let mut buf = [0.0; 2];
    /// let mut scratch = Scratch::borrowed(&mut buf);
    /// let b = scratch.big_from_f64(3.0)?;
    /// assert_eq!(scratch.big_limb(b, 0), 3);
    /// # Ok::<(), implicit_predicates_core::ScratchExhausted>(())
    /// ```
    #[must_use]
    pub fn big_limb(&self, b: Big, i: usize) -> u64 {
        self.words(b.limbs)[i].to_bits()
    }

    /// Stores `±words · 2^(64·exp)` normalized: words of zero at either end
    /// are dropped, the low ones into the exponent, and zero becomes
    /// [`Big::ZERO`]. Public only for tests.
    ///
    /// # Errors
    ///
    /// [`ScratchExhausted`] if the normalized words do not fit; the arena is
    /// unchanged.
    ///
    /// # Panics
    ///
    /// Panics if the exponent leaves the range of `i32`.
    #[doc(hidden)]
    pub fn big_from_words(
        &mut self,
        negative: bool,
        words: &[u64],
        exp: i32,
    ) -> Result<Big, ScratchExhausted> {
        let low = words.iter().take_while(|&&w| w == 0).count();
        let high = words.len() - words.iter().rev().take_while(|&&w| w == 0).count();
        if low >= high {
            return Ok(Big::ZERO);
        }
        let trimmed = &words[low..high];
        let exp = exp
            .checked_add(i32::try_from(low).expect("a word count fits in i32"))
            .expect("the exponent of a big integer stays within i32");
        let needed = u64::try_from(trimmed.len()).unwrap_or(u64::MAX);
        let limbs = self.write_words([], needed, |[], out| {
            for (slot, &word) in out.iter_mut().zip(trimmed) {
                *slot = f64::from_bits(word);
            }
            trimmed.len()
        })?;
        Ok(Big {
            limbs,
            exp,
            negative,
        })
    }
}
