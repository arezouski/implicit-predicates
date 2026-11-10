//! The product of two expansions in an arena.

use super::{Scratch, ScratchExhausted, Span};
use crate::expansion;

impl Scratch<'_> {
    /// Writes the expansion of `a * b` and returns its span.
    ///
    /// The product is the sum, over the components `c` of the shorter
    /// operand, of the longer operand scaled by `c` ([`expansion::scale`]),
    /// accumulated with [`expansion::sum`]. It is exact, and the result is a
    /// valid expansion of at most `2 * a.len() * b.len()` components.
    ///
    /// Requirement, as for [`scale`](Self::scale): every component of `a`
    /// and of `b` is zero or has magnitude in `2^-480 ..= 2^480`, so that
    /// every product of a component of `a` by one of `b` lies in the domain
    /// of [`eft::two_product`](crate::eft::two_product). The components of
    /// the result can lie outside that range, so a product of products needs
    /// the caller to bound the magnitudes involved.
    ///
    /// Needs `4 * a.len() * b.len() + 2 * max(a.len(), b.len())` free
    /// components while it works, or none if either operand is empty. The
    /// result ends up just above the previous top, like that of every other
    /// operation, and the working space beyond it is released.
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
    /// let tiny = 2f64.powi(-60);
    /// let mut buf = [0.0; 32];
    /// let mut scratch = Scratch::borrowed(&mut buf);
    /// let a = scratch.push(&[tiny, 1.0])?; // 1 + 2^-60
    /// let b = scratch.push(&[-tiny, 1.0])?; // 1 - 2^-60
    /// let p = scratch.product(a, b)?;
    /// assert_eq!(scratch.get(p), &[-tiny * tiny, 1.0]); // 1 - 2^-120
    /// assert_eq!(scratch.used(), 4 + p.len());
    /// assert_eq!(scratch.high_water(), 4 + 4 * 2 * 2 + 2 * 2);
    /// # Ok::<(), indirect_predicates_core::ScratchExhausted>(())
    /// ```
    pub fn product(&mut self, a: Span, b: Span) -> Result<Span, ScratchExhausted> {
        let needed = working_space(a.len, b.len).ok_or(ScratchExhausted)?;
        self.write_result([a, b], needed, |[a, b], work| {
            let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
            write_product(short, long, work)
        })
    }
}

/// The components [`Scratch::product`] needs above the top for operands of
/// `la` and `lb` components, or `None` if that does not fit in a `u64`.
fn working_space(la: u32, lb: u32) -> Option<u64> {
    if la == 0 || lb == 0 {
        return Some(0);
    }
    let longer = u64::from(la.max(lb));
    (u64::from(la) * u64::from(lb))
        .checked_mul(4)?
        .checked_add(2 * longer)
}

/// Writes the expansion of `short * long` at the start of `work` and
/// returns its length. `work` holds exactly [`working_space`] components,
/// split into three zones: two of `2 * short.len() * long.len()` that take
/// turns holding the running sum, and one of `2 * long.len()` for each
/// partial product.
fn write_product(short: &[f64], long: &[f64], work: &mut [f64]) -> usize {
    if short.is_empty() {
        return 0;
    }
    let zone = 2 * short.len() * long.len();
    let (sums, partial) = work.split_at_mut(2 * zone);
    let mut in_first_zone = true;
    let mut len = 0;
    for &c in short {
        let partial_len = expansion::scale(long, c, partial);
        let (first, second) = sums.split_at_mut(zone);
        let (from, to) = if in_first_zone {
            (first, second)
        } else {
            (second, first)
        };
        len = expansion::sum(&from[..len], &partial[..partial_len], to);
        in_first_zone = !in_first_zone;
    }
    if !in_first_zone {
        sums.copy_within(zone..zone + len, 0);
    }
    len
}

#[cfg(test)]
mod tests {
    use super::working_space;

    #[test]
    fn working_space_is_exact_and_never_overflows() {
        assert_eq!(working_space(0, 7), Some(0));
        assert_eq!(working_space(7, 0), Some(0));
        assert_eq!(working_space(2, 3), Some(4 * 6 + 2 * 3));
        assert_eq!(working_space(u32::MAX, 1), Some(6 * u64::from(u32::MAX)));
        assert_eq!(working_space(u32::MAX, u32::MAX), None);
    }
}
