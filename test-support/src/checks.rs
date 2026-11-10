//! Structural checks on expansions, read from the bit patterns of their
//! components without the arithmetic under test.

/// Panics unless `e` is a valid expansion in the sense of
/// `indirect_predicates_core::expansion`: every component is finite and
/// nonzero, and the components are sorted by increasing magnitude and
/// nonoverlapping, that is, the highest set bit of each lies strictly below
/// the lowest set bit of the next.
///
/// Checking neighbours suffices: nonoverlap between neighbours implies it
/// between any two components.
pub fn assert_valid_expansion(e: &[f64]) {
    for (i, &component) in e.iter().enumerate() {
        assert!(
            component.is_finite() && component != 0.0,
            "component {i} of {e:?} is zero or not finite"
        );
    }
    for (i, pair) in e.windows(2).enumerate() {
        assert!(
            highest_bit(pair[0]) < lowest_bit(pair[1]),
            "components {i} and {} of {e:?} overlap or are out of order",
            i + 1
        );
    }
}

/// Returns `true` if two neighbouring components of `e` are *adjacent*: the
/// lowest set bit of one lies just above the highest set bit of the other,
/// as in `[3.0, 4.0]`. The expansion invariant allows this.
pub fn has_adjacent_components(e: &[f64]) -> bool {
    e.windows(2)
        .any(|pair| lowest_bit(pair[1]) == highest_bit(pair[0]) + 1)
}

/// The exponent of the highest set bit of a finite nonzero `x`.
pub(crate) fn highest_bit(x: f64) -> i32 {
    let (significand, scale) = integer_significand(x);
    let leading_zeros = i32::try_from(significand.leading_zeros())
        .expect("a u64 has at most 64 leading zeros, which fits in i32");
    scale + 63 - leading_zeros
}

/// The exponent of the lowest set bit of a finite nonzero `x`.
pub(crate) fn lowest_bit(x: f64) -> i32 {
    let (significand, scale) = integer_significand(x);
    let trailing_zeros = i32::try_from(significand.trailing_zeros())
        .expect("a u64 has at most 64 trailing zeros, which fits in i32");
    scale + trailing_zeros
}

/// `(s, q)` with `|x| = s * 2^q` and `s` an integer below `2^53`: the
/// significand field of `x`, with its implicit bit when `x` is normal, and
/// the weight of its lowest bit.
pub(crate) fn integer_significand(x: f64) -> (u64, i32) {
    let bits = x.to_bits();
    let biased =
        i32::try_from((bits >> 52) & 0x7FF).expect("an 11-bit biased exponent fits in i32");
    let fraction = bits & ((1 << 52) - 1);
    if biased == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1 << 52), biased - 1075)
    }
}
