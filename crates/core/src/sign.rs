//! The sign of a quantity: the result type of every predicate.

use core::cmp::Ordering;
use core::ops::{Mul, Neg};

/// The sign of a real quantity.
///
/// The discriminants are `-1`, `0` and `1`, so `sign as i8` (or
/// `i8::from(sign)`) gives the usual integer sign, and signs are ordered as
/// those integers: `Negative < Zero < Positive`.
///
/// # Examples
///
/// ```
/// use indirect_predicates_core::Sign;
///
/// assert_eq!(Sign::Negative as i8, -1);
/// assert_eq!(Sign::Zero as i8, 0);
/// assert_eq!(Sign::Positive as i8, 1);
/// assert!(Sign::Negative < Sign::Zero && Sign::Zero < Sign::Positive);
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
#[repr(i8)]
pub enum Sign {
    /// Strictly less than zero.
    Negative = -1,
    /// Exactly zero.
    Zero = 0,
    /// Strictly greater than zero.
    Positive = 1,
}

impl Sign {
    /// Returns the sign of `x`. Both `0.0` and `-0.0` give [`Sign::Zero`].
    ///
    /// NaN is not a valid input: debug builds panic on it, and release builds
    /// return [`Sign::Zero`], which is meaningless.
    ///
    /// # Examples
    ///
    /// ```
    /// use indirect_predicates_core::Sign;
    ///
    /// assert_eq!(Sign::of(2.5), Sign::Positive);
    /// assert_eq!(Sign::of(-2.5), Sign::Negative);
    /// assert_eq!(Sign::of(-0.0), Sign::Zero);
    /// ```
    #[must_use]
    pub fn of(x: f64) -> Sign {
        debug_assert!(!x.is_nan(), "Sign::of called with NaN");
        if x > 0.0 {
            Sign::Positive
        } else if x < 0.0 {
            Sign::Negative
        } else {
            Sign::Zero
        }
    }

    /// Returns `true` if the sign is [`Sign::Zero`].
    ///
    /// # Examples
    ///
    /// ```
    /// use indirect_predicates_core::Sign;
    ///
    /// assert!(Sign::Zero.is_zero());
    /// assert!(!Sign::Positive.is_zero());
    /// ```
    #[must_use]
    pub fn is_zero(self) -> bool {
        self == Sign::Zero
    }

    /// Returns `true` if the sign is [`Sign::Positive`].
    ///
    /// # Examples
    ///
    /// ```
    /// use indirect_predicates_core::Sign;
    ///
    /// assert!(Sign::Positive.is_positive());
    /// assert!(!Sign::Zero.is_positive());
    /// ```
    #[must_use]
    pub fn is_positive(self) -> bool {
        self == Sign::Positive
    }

    /// Returns `true` if the sign is [`Sign::Negative`].
    ///
    /// # Examples
    ///
    /// ```
    /// use indirect_predicates_core::Sign;
    ///
    /// assert!(Sign::Negative.is_negative());
    /// assert!(!Sign::Zero.is_negative());
    /// ```
    #[must_use]
    pub fn is_negative(self) -> bool {
        self == Sign::Negative
    }
}

impl Neg for Sign {
    type Output = Sign;

    /// Flips the sign; [`Sign::Zero`] stays zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use indirect_predicates_core::Sign;
    ///
    /// assert_eq!(-Sign::Positive, Sign::Negative);
    /// assert_eq!(-Sign::Zero, Sign::Zero);
    /// ```
    fn neg(self) -> Sign {
        match self {
            Sign::Negative => Sign::Positive,
            Sign::Zero => Sign::Zero,
            Sign::Positive => Sign::Negative,
        }
    }
}

impl Mul for Sign {
    type Output = Sign;

    /// The sign of a product: zero if either factor is zero, positive if the
    /// signs agree, negative otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use indirect_predicates_core::Sign;
    ///
    /// assert_eq!(Sign::Negative * Sign::Negative, Sign::Positive);
    /// assert_eq!(Sign::Negative * Sign::Positive, Sign::Negative);
    /// assert_eq!(Sign::Zero * Sign::Positive, Sign::Zero);
    /// ```
    fn mul(self, rhs: Sign) -> Sign {
        match (self, rhs) {
            (Sign::Zero, _) | (_, Sign::Zero) => Sign::Zero,
            (a, b) if a == b => Sign::Positive,
            _ => Sign::Negative,
        }
    }
}

impl From<Sign> for i8 {
    /// The integer sign: `-1`, `0` or `1`.
    ///
    /// # Examples
    ///
    /// ```
    /// use indirect_predicates_core::Sign;
    ///
    /// assert_eq!(i8::from(Sign::Negative), -1);
    /// assert_eq!(i8::from(Sign::Positive), 1);
    /// ```
    fn from(sign: Sign) -> i8 {
        sign as i8
    }
}

impl From<Ordering> for Sign {
    /// The sign of `a - b` when `a.cmp(&b)` is `ordering`: `Less` is
    /// negative, `Equal` zero and `Greater` positive.
    ///
    /// # Examples
    ///
    /// ```
    /// use indirect_predicates_core::Sign;
    ///
    /// assert_eq!(Sign::from(3.cmp(&5)), Sign::Negative);
    /// assert_eq!(Sign::from(5.cmp(&5)), Sign::Zero);
    /// ```
    fn from(ordering: Ordering) -> Sign {
        match ordering {
            Ordering::Less => Sign::Negative,
            Ordering::Equal => Sign::Zero,
            Ordering::Greater => Sign::Positive,
        }
    }
}
