//! Exact addition, subtraction and multiplication of big integers.

use core::cmp::Ordering;

use super::Big;
use super::limbs::{self, Placed};
use crate::scratch::{Scratch, ScratchExhausted};

impl Scratch<'_> {
    /// Returns the exact sum `a + b`, written above the top of the arena.
    ///
    /// The result is in its unique representation. Adding zero returns the
    /// other operand without writing, and a sum that cancels is
    /// [`Big::ZERO`] with nothing left in the arena. Space for the longest
    /// possible result is reserved (and counted by
    /// [`high_water`](Self::high_water)) while the operation runs, but only
    /// the real result stays.
    ///
    /// # Errors
    ///
    /// [`ScratchExhausted`] if that space is not free, or if the exponents
    /// are so far apart that the result would be wider than `u32::MAX`
    /// words; the arena is unchanged.
    ///
    /// # Panics
    ///
    /// Panics if an exponent leaves the range of `i32`.
    ///
    /// # Examples
    ///
    /// ```
    /// use implicit_predicates_core::{Scratch, Sign};
    ///
    /// let mut buf = [0.0; 8];
    /// let mut scratch = Scratch::borrowed(&mut buf);
    /// let a = scratch.big_from_f64(3.0)?;
    /// let b = scratch.big_from_f64(-3.0)?;
    /// assert_eq!(scratch.big_add(a, b)?.sign(), Sign::Zero);
    /// # Ok::<(), implicit_predicates_core::ScratchExhausted>(())
    /// ```
    pub fn big_add(&mut self, a: Big, b: Big) -> Result<Big, ScratchExhausted> {
        if a.is_zero() {
            return Ok(b);
        }
        if b.is_zero() {
            return Ok(a);
        }
        // The frame starts at the lower exponent; each operand sits at its
        // distance from it. One extra word takes a final carry.
        let base = a.exp.min(b.exp);
        // Everything here is in u64 with checked operations: on a 32-bit
        // target a distance between exponents can exceed `usize`. A frame
        // wider than any arena can be is exhausted space, not a wrong sum.
        let offset = |x: Big| {
            u64::try_from(i64::from(x.exp) - i64::from(base)).expect("an offset is not negative")
        };
        let (off_a, off_b) = (offset(a), offset(b));
        let end = |off: u64, x: Big| off.checked_add(x.len() as u64).ok_or(ScratchExhausted);
        let top = end(off_a, a)?.max(end(off_b, b)?);
        let needed = top.checked_add(1).ok_or(ScratchExhausted)?;
        if needed > u64::from(u32::MAX) {
            return Err(ScratchExhausted);
        }
        // Every value below fits in `u32`, hence in `usize`.
        let to_index = |n: u64| usize::try_from(n).expect("a frame position fits in usize");
        let (off_a, off_b) = (to_index(off_a), to_index(off_b));
        let top = to_index(top);
        let frame = top + 1;

        let same_sign = a.negative == b.negative;
        // For opposite signs the larger magnitude is the minuend; the
        // operands are compared before reserving, from the words as stored.
        let order = if same_sign {
            Ordering::Greater
        } else {
            let pa = Placed {
                words: self.words(a.limbs),
                offset: off_a,
            };
            let pb = Placed {
                words: self.words(b.limbs),
                offset: off_b,
            };
            limbs::compare(pa, pb, top)
        };
        if order == Ordering::Equal {
            return Ok(Big::ZERO);
        }
        // The larger magnitude is the minuend and gives the sign.
        let ((first, off_first), (second, off_second)) = if order == Ordering::Less {
            ((b, off_b), (a, off_a))
        } else {
            ((a, off_a), (b, off_b))
        };
        let negative = first.negative;

        let mut dropped = 0;
        let limbs = self.write_words([first.limbs, second.limbs], needed, |[wf, ws], out| {
            let pf = Placed {
                words: wf,
                offset: off_first,
            };
            let ps = Placed {
                words: ws,
                offset: off_second,
            };
            let out = &mut out[..frame];
            if same_sign {
                limbs::add(out, pf, ps);
            } else {
                limbs::sub(out, pf, ps);
            }
            // Drop the zero words at both ends so the result is unique and
            // nothing but the result stays reserved.
            let (len, low) = limbs::trim_and_shift(out, frame);
            dropped = low;
            // Unequal magnitudes cannot cancel; a zero here would not be
            // the unique zero.
            debug_assert!(len > 0, "a sum of unequal magnitudes came out zero");
            len
        })?;
        let exp = i32::try_from(i64::from(base) + i64::try_from(dropped).unwrap_or(i64::MAX))
            .expect("the exponent of a big integer stays within i32");
        Ok(Big {
            limbs,
            exp,
            negative,
        })
    }

    /// Returns the exact product `a * b`, written above the top of the
    /// arena.
    ///
    /// The result is in its unique representation: a product with a zero
    /// factor is [`Big::ZERO`] and writes nothing, and the zero words that
    /// the product may have at either end are dropped, so only the real
    /// result stays. Space for `a.len() + b.len()` words is reserved (and
    /// counted by [`high_water`](Self::high_water)) while the operation
    /// runs.
    ///
    /// # Errors
    ///
    /// [`ScratchExhausted`] if that space is not free, or if the exponent
    /// of the product leaves the range of `i32`, which cannot happen for
    /// operands whose exponents come from fewer than `u32::MAX` words; the
    /// arena is unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use implicit_predicates_core::{Scratch, Sign};
    ///
    /// let mut buf = [0.0; 16];
    /// let mut scratch = Scratch::borrowed(&mut buf);
    /// let a = scratch.big_from_f64(-3.0)?;
    /// let b = scratch.big_from_f64(5.0)?;
    /// let p = scratch.big_mul(a, b)?;
    /// assert_eq!(p.sign(), Sign::Negative);
    /// assert_eq!(scratch.big_limb(p, 0), 15);
    /// # Ok::<(), implicit_predicates_core::ScratchExhausted>(())
    /// ```
    pub fn big_mul(&mut self, a: Big, b: Big) -> Result<Big, ScratchExhausted> {
        if a.is_zero() || b.is_zero() {
            return Ok(Big::ZERO);
        }
        // The lowest word of each factor is nonzero, so the product has
        // exactly `tz_a + tz_b` trailing zero bits, fewer than 128: it
        // loses zero or one low word, known before anything is written.
        let low_zeros = |x: Big, s: &Self| s.words(x.limbs)[0].to_bits().trailing_zeros();
        let lost = u64::from((low_zeros(a, self) + low_zeros(b, self)) / 64);
        // Lengths and exponents in 64 bits, checked: see `big_add`.
        let needed = (a.len() as u64)
            .checked_add(b.len() as u64)
            .ok_or(ScratchExhausted)?;
        if needed > u64::from(u32::MAX) {
            return Err(ScratchExhausted);
        }
        let exp = i32::try_from(i64::from(a.exp) + i64::from(b.exp) + lost as i64)
            .map_err(|_| ScratchExhausted)?;
        // Fits in `u32`, hence in `usize`.
        let frame = usize::try_from(needed).expect("a frame length fits in usize");
        let limbs = self.write_words([a.limbs, b.limbs], needed, |[wa, wb], out| {
            let out = &mut out[..frame];
            limbs::mul(out, wa, wb);
            let (len, low) = limbs::trim_and_shift(out, frame);
            debug_assert!(len > 0, "a product of nonzero factors came out zero");
            debug_assert_eq!(low as u64, lost, "the product lost the predicted words");
            len
        })?;
        Ok(Big {
            limbs,
            exp,
            negative: a.negative != b.negative,
        })
    }

    /// Returns the exact difference `a - b`; see [`big_add`](Self::big_add).
    ///
    /// # Errors
    ///
    /// [`ScratchExhausted`] if the space is not free or the exponents are
    /// too far apart; the arena is unchanged.
    ///
    /// # Panics
    ///
    /// Panics if an exponent leaves the range of `i32`.
    ///
    /// # Examples
    ///
    /// ```
    /// use implicit_predicates_core::{Scratch, Sign};
    ///
    /// let mut buf = [0.0; 8];
    /// let mut scratch = Scratch::borrowed(&mut buf);
    /// let a = scratch.big_from_f64(1.0)?;
    /// let b = scratch.big_from_f64(3.0)?;
    /// assert_eq!(scratch.big_sub(a, b)?.sign(), Sign::Negative);
    /// # Ok::<(), implicit_predicates_core::ScratchExhausted>(())
    /// ```
    pub fn big_sub(&mut self, a: Big, b: Big) -> Result<Big, ScratchExhausted> {
        self.big_add(a, b.neg())
    }
}
