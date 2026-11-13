//! Exact addition and subtraction of big integers.

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
    /// [`ScratchExhausted`] if that space is not free; the arena is
    /// unchanged.
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
        let offset = |x: Big| {
            usize::try_from(i64::from(x.exp) - i64::from(base)).expect("an offset is not negative")
        };
        let (off_a, off_b) = (offset(a), offset(b));
        let top = (off_a + a.len()).max(off_b + b.len());
        let frame = top + 1;
        let needed = u64::try_from(frame).unwrap_or(u64::MAX);

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
        let (negative, first, second) = match order {
            Ordering::Less => (b.negative, b, a),
            _ => (a.negative, a, b),
        };
        let (off_first, off_second) = if order == Ordering::Less {
            (off_b, off_a)
        } else {
            (off_a, off_b)
        };

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

    /// Returns the exact difference `a - b`; see [`big_add`](Self::big_add).
    ///
    /// # Errors
    ///
    /// [`ScratchExhausted`] if the space is not free; the arena is
    /// unchanged.
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
