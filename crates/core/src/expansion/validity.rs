//! Checking the expansion invariants, for the debug assertions on data that
//! enters the crate from outside.

/// Returns `true` if `e` is a valid expansion (see the
/// [module documentation](super)): its components are finite and nonzero,
/// sorted by strictly increasing magnitude and nonoverlapping.
///
/// Neighbours suffice: if each component lies strictly above the bits of
/// the one before, it lies above all the earlier ones.
pub(crate) fn is_valid(e: &[f64]) -> bool {
    let mut previous_highest = None;
    for &component in e {
        let Some((lowest, highest)) = set_bit_range(component) else {
            return false;
        };
        if previous_highest.is_some_and(|below| lowest <= below) {
            return false;
        }
        previous_highest = Some(highest);
    }
    true
}

/// The exponents of the lowest and of the highest set bit of `x`, read from
/// its bit pattern, or `None` if `x` is zero, infinite or NaN.
fn set_bit_range(x: f64) -> Option<(i32, i32)> {
    let bits = x.to_bits();
    // The masked exponent field has 11 bits, so it fits in an `i32`.
    let biased_exponent = ((bits >> 52) & 0x7FF) as i32;
    let fraction = bits & ((1 << 52) - 1);
    let significand = match biased_exponent {
        0x7FF => return None,
        0 => fraction,
        _ => fraction | (1 << 52),
    };
    if significand == 0 {
        return None;
    }
    // The weight of bit 0 of the significand: subnormals share the scale of
    // the smallest normal exponent.
    let unit = biased_exponent.max(1) - 1075;
    // Both counts are at most 64, so they fit in an `i32`.
    let lowest = unit + significand.trailing_zeros() as i32;
    let highest = unit + 63 - significand.leading_zeros() as i32;
    Some((lowest, highest))
}

#[cfg(test)]
mod tests {
    use super::is_valid;

    #[test]
    fn accepts_valid_expansions_including_adjacent_and_subnormal_components() {
        let valid: [&[f64]; 8] = [
            &[],
            &[-0.75],
            &[1.0, 2.0],
            &[3.0, 4.0],
            &[1.5, 2.0, -4.0, 8.0],
            &[5e-324, 1e-323 * 2.0, 1.0],
            &[f64::MIN_POSITIVE / 2.0, f64::MIN_POSITIVE],
            &[1.0, f64::MAX],
        ];
        for e in valid {
            assert!(is_valid(e), "{e:?} is valid");
        }
    }

    #[test]
    fn rejects_each_kind_of_invalid_expansion() {
        let invalid: [&[f64]; 10] = [
            &[0.0],
            &[-0.0, 1.0],
            &[f64::NAN],
            &[1.0, f64::INFINITY],
            &[1.0, 1.0],
            &[2.0, 1.0],
            &[3.0, 5.0],
            &[1.5, 2.5],
            &[3.0, 6.0],
            &[f64::MIN_POSITIVE, f64::MIN_POSITIVE * 1.5],
        ];
        for e in invalid {
            assert!(!is_valid(e), "{e:?} is not valid");
        }
    }
}
