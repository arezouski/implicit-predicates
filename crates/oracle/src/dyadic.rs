//! Exact dyadic rational numbers, `mantissa * 2^exponent`.

use core::cmp::Ordering;
use core::ops::{Add, Mul, Neg, Sub};

use num_bigint::{BigInt, Sign};

/// An exact dyadic rational: an arbitrary-precision integer mantissa times a
/// power of two.
///
/// Every finite `f64` converts exactly, and addition, subtraction and
/// multiplication never round or overflow. Equality and ordering compare
/// values, not representations.
///
/// # Examples
///
/// ```
/// use indirect_predicates_oracle::Dyadic;
///
/// let tenth = Dyadic::from_f64(0.1);
/// let three_tenths = &(&tenth + &tenth) + &tenth;
/// // 0.1 + 0.1 + 0.1 is not 0.3 in binary, and the oracle sees the difference.
/// assert_eq!(three_tenths.sign(), 1);
/// assert!(three_tenths > Dyadic::from_f64(0.3));
/// ```
#[derive(Clone, Debug)]
pub struct Dyadic {
    mant: BigInt,
    exp: i64,
}

impl Dyadic {
    /// Converts a finite `f64` exactly. Negative zero converts to zero.
    ///
    /// # Panics
    ///
    /// Panics if `x` is NaN or infinite.
    ///
    /// # Examples
    ///
    /// ```
    /// use indirect_predicates_oracle::Dyadic;
    ///
    /// let two = &Dyadic::from_f64(1.0) + &Dyadic::from_f64(1.0);
    /// assert_eq!(two, Dyadic::from_f64(2.0));
    /// ```
    pub fn from_f64(x: f64) -> Dyadic {
        assert!(x.is_finite(), "Dyadic::from_f64: non-finite input");
        let bits = x.to_bits();
        let biased_exp = ((bits >> 52) & 0x7ff) as i64;
        let fraction = bits & ((1u64 << 52) - 1);
        let (magnitude, exp) = if biased_exp == 0 {
            // Subnormal (or zero): no implicit bit.
            (fraction, -1074)
        } else {
            (fraction | (1u64 << 52), biased_exp - 1075)
        };
        let sign = if bits >> 63 == 1 {
            Sign::Minus
        } else {
            Sign::Plus
        };
        Dyadic {
            mant: BigInt::from_biguint(sign, magnitude.into()),
            exp,
        }
    }

    /// Returns the exact sum of all the given values.
    ///
    /// # Panics
    ///
    /// Panics if any value is NaN or infinite.
    ///
    /// # Examples
    ///
    /// ```
    /// use indirect_predicates_oracle::Dyadic;
    ///
    /// assert_eq!(Dyadic::sum_of(&[1.0, 2.0, 4.0]), Dyadic::from_f64(7.0));
    /// assert!(Dyadic::sum_of(&[]).is_zero());
    /// ```
    pub fn sum_of(xs: &[f64]) -> Dyadic {
        xs.iter()
            .fold(Dyadic::from_f64(0.0), |acc, &x| &acc + &Dyadic::from_f64(x))
    }

    /// Returns `-1`, `0` or `1` according to the sign of the value.
    ///
    /// # Examples
    ///
    /// ```
    /// use indirect_predicates_oracle::Dyadic;
    ///
    /// assert_eq!(Dyadic::from_f64(-3.5).sign(), -1);
    /// assert_eq!(Dyadic::from_f64(0.0).sign(), 0);
    /// assert_eq!(Dyadic::from_f64(3.5).sign(), 1);
    /// ```
    pub fn sign(&self) -> i8 {
        match self.mant.sign() {
            Sign::Minus => -1,
            Sign::NoSign => 0,
            Sign::Plus => 1,
        }
    }

    /// Returns `true` if the value is exactly zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use indirect_predicates_oracle::Dyadic;
    ///
    /// assert!(Dyadic::from_f64(-0.0).is_zero());
    /// assert!(!Dyadic::from_f64(1e-300).is_zero());
    /// ```
    pub fn is_zero(&self) -> bool {
        self.sign() == 0
    }

    /// Mantissa scaled so that it is expressed with exponent `exp`, which
    /// must not exceed `self.exp`.
    fn mantissa_at(&self, exp: i64) -> BigInt {
        let shift = usize::try_from(self.exp - exp).expect("exponent difference fits in usize");
        &self.mant << shift
    }
}

impl Add for &Dyadic {
    type Output = Dyadic;

    fn add(self, rhs: &Dyadic) -> Dyadic {
        let exp = self.exp.min(rhs.exp);
        Dyadic {
            mant: self.mantissa_at(exp) + rhs.mantissa_at(exp),
            exp,
        }
    }
}

impl Sub for &Dyadic {
    type Output = Dyadic;

    fn sub(self, rhs: &Dyadic) -> Dyadic {
        let exp = self.exp.min(rhs.exp);
        Dyadic {
            mant: self.mantissa_at(exp) - rhs.mantissa_at(exp),
            exp,
        }
    }
}

impl Mul for &Dyadic {
    type Output = Dyadic;

    fn mul(self, rhs: &Dyadic) -> Dyadic {
        Dyadic {
            mant: &self.mant * &rhs.mant,
            exp: self.exp + rhs.exp,
        }
    }
}

impl Neg for &Dyadic {
    type Output = Dyadic;

    fn neg(self) -> Dyadic {
        Dyadic {
            mant: -&self.mant,
            exp: self.exp,
        }
    }
}

/// Implements an operator for owned operands by delegating to the
/// reference implementation.
macro_rules! forward_owned_binary_op {
    ($trait:ident, $method:ident) => {
        impl $trait for Dyadic {
            type Output = Dyadic;

            fn $method(self, rhs: Dyadic) -> Dyadic {
                $trait::$method(&self, &rhs)
            }
        }
    };
}

forward_owned_binary_op!(Add, add);
forward_owned_binary_op!(Sub, sub);
forward_owned_binary_op!(Mul, mul);

impl Neg for Dyadic {
    type Output = Dyadic;

    fn neg(self) -> Dyadic {
        -&self
    }
}

impl PartialEq for Dyadic {
    fn eq(&self, other: &Dyadic) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Dyadic {}

impl PartialOrd for Dyadic {
    fn partial_cmp(&self, other: &Dyadic) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Dyadic {
    fn cmp(&self, other: &Dyadic) -> Ordering {
        (self - other).sign().cmp(&0)
    }
}
