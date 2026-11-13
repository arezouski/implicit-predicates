//! Arithmetic on magnitudes made of 64-bit words stored as `f64` bit
//! patterns, least significant word first.
//!
//! Every function works in a common frame: an operand is a slice of words
//! placed at an offset, and the words of the frame outside it are zero. The
//! frame is as long as the output slice. Nothing here treats a word as a
//! number: they are read with `to_bits` and written with `from_bits`.

use core::cmp::Ordering;

/// An operand of the frame: `words` placed `offset` words above its bottom.
#[derive(Clone, Copy)]
pub(super) struct Placed<'a> {
    pub(super) words: &'a [f64],
    pub(super) offset: usize,
}

impl Placed<'_> {
    /// The word at position `i` of the frame; zero outside the operand.
    fn at(self, i: usize) -> u64 {
        match i.checked_sub(self.offset).and_then(|j| self.words.get(j)) {
            Some(w) => w.to_bits(),
            None => 0,
        }
    }
}

/// Compares two magnitudes placed in a frame of `frame` words.
pub(super) fn compare(a: Placed<'_>, b: Placed<'_>, frame: usize) -> Ordering {
    for i in (0..frame).rev() {
        match a.at(i).cmp(&b.at(i)) {
            Ordering::Equal => {}
            other => return other,
        }
    }
    Ordering::Equal
}

/// Writes `a + b` into `out`, which is the whole frame. The frame must be
/// long enough to hold the sum: the final carry is dropped otherwise.
pub(super) fn add(out: &mut [f64], a: Placed<'_>, b: Placed<'_>) {
    let mut carry = false;
    for (i, slot) in out.iter_mut().enumerate() {
        let (s, c1) = a.at(i).overflowing_add(b.at(i));
        let (s, c2) = s.overflowing_add(u64::from(carry));
        carry = c1 || c2;
        *slot = f64::from_bits(s);
    }
    debug_assert!(!carry, "the frame is too short for the sum");
}

/// Writes `big - small` into `out`, which is the whole frame. Requires
/// `big >= small`.
pub(super) fn sub(out: &mut [f64], big: Placed<'_>, small: Placed<'_>) {
    let mut borrow = false;
    for (i, slot) in out.iter_mut().enumerate() {
        let (d, b1) = big.at(i).overflowing_sub(small.at(i));
        let (d, b2) = d.overflowing_sub(u64::from(borrow));
        borrow = b1 || b2;
        *slot = f64::from_bits(d);
    }
    debug_assert!(!borrow, "the subtrahend is the larger magnitude");
}

/// Drops the zero words at both ends of `out[..len]`, moving the rest down
/// to the start. Returns the new length and how many low words were
/// dropped.
pub(super) fn trim_and_shift(out: &mut [f64], len: usize) -> (usize, usize) {
    let words = &out[..len];
    let low = words.iter().take_while(|w| w.to_bits() == 0).count();
    let high = len - words.iter().rev().take_while(|w| w.to_bits() == 0).count();
    if low >= high {
        return (0, 0);
    }
    out.copy_within(low..high, 0);
    (high - low, low)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w<const N: usize>(words: [u64; N]) -> [f64; N] {
        words.map(f64::from_bits)
    }

    fn bits<const N: usize>(words: [f64; N]) -> [u64; N] {
        words.map(f64::to_bits)
    }

    #[test]
    fn carries_and_borrows_cross_words() {
        let a = w([u64::MAX, u64::MAX]);
        let one = w([1]);
        let pa = Placed {
            words: &a,
            offset: 0,
        };
        let p1 = Placed {
            words: &one,
            offset: 0,
        };
        let mut out = [0.0; 3];
        add(&mut out, pa, p1);
        assert_eq!(bits(out), [0, 0, 1]);
        let mut back = [0.0; 3];
        let sum = w([0, 0, 1]);
        sub(
            &mut back,
            Placed {
                words: &sum,
                offset: 0,
            },
            p1,
        );
        assert_eq!(bits(back), [u64::MAX, u64::MAX, 0]);
    }

    #[test]
    fn trimming_moves_the_words_down() {
        let mut v = w([0, 0, 7, 0, 9, 0]);
        assert_eq!(trim_and_shift(&mut v, 6), (3, 2));
        assert_eq!(bits([v[0], v[1], v[2]]), [7, 0, 9]);
        assert_eq!(trim_and_shift(&mut w([0, 0]), 2), (0, 0));
    }
}
