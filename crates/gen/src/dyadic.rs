//! Exact dyadic rationals: an integer times a power of two.
//!
//! Every quantity of the filter bound is built from integers and powers of
//! two by sums and products, so it is a dyadic rational and is represented
//! here without error.

use std::cmp::Ordering;
use std::ops::{Add, Mul, Sub};

use num_bigint::{BigInt, BigUint, Sign};

/// The exact value `mantissa * 2^exponent`.
///
/// The representation is unique: the mantissa is odd, or it is zero and the
/// exponent is zero. Derived equality is therefore equality of values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Dyadic {
    mantissa: BigInt,
    exponent: i64,
}

impl Dyadic {
    /// The value `mantissa * 2^exponent`.
    pub(crate) fn new(mantissa: impl Into<BigInt>, exponent: i64) -> Self {
        let mantissa = mantissa.into();
        match mantissa.trailing_zeros() {
            None => Dyadic {
                mantissa,
                exponent: 0,
            },
            Some(zeros) => Dyadic {
                mantissa: mantissa >> zeros,
                exponent: exponent + i64::try_from(zeros).expect("a shift fits in i64"),
            },
        }
    }

    /// The integer `n`.
    pub(crate) fn integer(n: impl Into<BigInt>) -> Self {
        Dyadic::new(n, 0)
    }

    /// The power of two `2^exponent`.
    pub(crate) fn pow2(exponent: i64) -> Self {
        Dyadic::new(1, exponent)
    }

    /// The value of `x`, which must be finite.
    #[cfg(test)]
    pub(crate) fn from_f64(x: f64) -> Self {
        assert!(x.is_finite());
        let bits = x.to_bits();
        let field = ((bits >> 52) & 0x7ff) as i64;
        let fraction = bits & ((1 << 52) - 1);
        let (magnitude, exponent) = if field == 0 {
            (fraction, -1074)
        } else {
            (fraction | (1 << 52), field - 1075)
        };
        let magnitude = BigInt::from(magnitude);
        let mantissa = if x.is_sign_negative() {
            -magnitude
        } else {
            magnitude
        };
        Dyadic::new(mantissa, exponent)
    }

    /// The exact value as an integer or as `numerator/denominator` in lowest
    /// terms.
    pub(crate) fn fraction(&self) -> String {
        match u32::try_from(-self.exponent) {
            Ok(shift) if shift > 0 => {
                format!("{}/{}", self.mantissa, BigUint::from(1u8) << shift)
            }
            _ => (&self.mantissa << self.exponent.max(0)).to_string(),
        }
    }

    /// The smallest integer `s` with `value <= 2^s`, for a positive value.
    pub(crate) fn ceil_log2(&self) -> i64 {
        assert!(
            self.mantissa.sign() == Sign::Plus,
            "the value must be positive"
        );
        // The mantissa is odd: it is 1, or strictly between two powers of two.
        let width = i64::try_from(self.mantissa.bits()).expect("a width fits in i64");
        if width == 1 {
            self.exponent
        } else {
            self.exponent + width
        }
    }

    /// The smallest `f64` greater than or equal to the value, or infinity.
    pub(crate) fn round_up(&self) -> f64 {
        self.round(true)
    }

    /// The largest `f64` less than or equal to the value, or minus infinity.
    #[cfg(test)]
    pub(crate) fn round_down(&self) -> f64 {
        self.round(false)
    }

    /// Rounds to `f64` towards plus infinity when `up`, towards minus
    /// infinity otherwise.
    fn round(&self, up: bool) -> f64 {
        let negative = self.mantissa.sign() == Sign::Minus;
        // Whether the magnitude is rounded away from zero.
        let away = up != negative;
        let magnitude = self.mantissa.magnitude();
        if magnitude.bits() == 0 {
            return 0.0;
        }
        let width = i64::try_from(magnitude.bits()).expect("a width fits in i64");
        // The unit in the last place of the result: 53 significant bits,
        // but never below the smallest subnormal.
        let ulp = (width + self.exponent - 53).max(-1074);
        let units = if self.exponent >= ulp {
            magnitude << u64::try_from(self.exponent - ulp).expect("nonnegative")
        } else {
            // The mantissa is odd, so a right shift always discards a one
            // bit: the value lies strictly between two multiples of the ulp.
            let truncated = magnitude >> u64::try_from(ulp - self.exponent).expect("positive");
            if away { truncated + 1u8 } else { truncated }
        };
        // `units` is at most 2^53, so the conversions below are exact.
        let units = u64::try_from(&units).expect("at most 54 bits");
        let result = if ulp + i64::from(64 - units.leading_zeros()) > 1024 {
            if away { f64::INFINITY } else { f64::MAX }
        } else {
            // The product is a representable value, so it is exact.
            units as f64 * pow2_f64(ulp)
        };
        if negative { -result } else { result }
    }
}

/// `2^exponent` as an `f64`, for `exponent` in `-1074..=1023`.
fn pow2_f64(exponent: i64) -> f64 {
    if exponent >= -1022 {
        f64::from_bits(u64::try_from(exponent + 1023).expect("in range") << 52)
    } else {
        f64::from_bits(1 << (exponent + 1074))
    }
}

/// The mantissa of `x` scaled to the exponent `exponent`, which is at most
/// that of `x`.
fn aligned(x: &Dyadic, exponent: i64) -> BigInt {
    &x.mantissa << (x.exponent - exponent)
}

impl Add for &Dyadic {
    type Output = Dyadic;

    fn add(self, other: &Dyadic) -> Dyadic {
        let exponent = self.exponent.min(other.exponent);
        Dyadic::new(aligned(self, exponent) + aligned(other, exponent), exponent)
    }
}

impl Sub for &Dyadic {
    type Output = Dyadic;

    fn sub(self, other: &Dyadic) -> Dyadic {
        let negated = Dyadic {
            mantissa: -&other.mantissa,
            exponent: other.exponent,
        };
        self + &negated
    }
}

impl Mul for &Dyadic {
    type Output = Dyadic;

    fn mul(self, other: &Dyadic) -> Dyadic {
        Dyadic::new(
            &self.mantissa * &other.mantissa,
            self.exponent + other.exponent,
        )
    }
}

impl Ord for Dyadic {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self - other).mantissa.sign() {
            Sign::Minus => Ordering::Less,
            Sign::NoSign => Ordering::Equal,
            Sign::Plus => Ordering::Greater,
        }
    }
}

impl PartialOrd for Dyadic {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests;
