//! Error-free transformations: floating-point sums and products together with
//! their exact rounding errors.
//!
//! Every function returns a pair `(x, y)` where `x` is the correctly rounded
//! floating-point result of the operation and `y` its rounding error, so that
//! `x + y` equals the real result exactly. This holds under the requirement
//! documented on each function, assuming the default round-to-nearest-even
//! mode. NaN and infinite inputs are not supported.
//!
//! The algorithms are those of Knuth, Dekker and Veltkamp as presented by
//! Shewchuk, "Adaptive Precision Floating-Point Arithmetic and Fast Robust
//! Geometric Predicates" (1997). When the target has a hardware fused
//! multiply-add (see [`HAS_FMA`]), the exact product uses it instead.

// On 32-bit x86 without SSE2, `f64` arithmetic runs on the x87 unit, whose
// excess precision and double rounding break every error-free
// transformation here.
#[cfg(all(target_arch = "x86", not(target_feature = "sse2")))]
compile_error!(
    "implicit-predicates-core needs SSE2 on 32-bit x86: x87 floating-point arithmetic \
     uses excess precision, which breaks the error-free transformations"
);

/// Selects code by whether the target has a hardware fused multiply-add that
/// the crate can use: x86_64 compiled with the `fma` target feature, or
/// aarch64 with the `neon` target feature. This is the single definition of
/// that condition.
///
/// - `fma_target!(has)` is `true` or `false` according to the condition;
/// - `fma_target!(item)` keeps `item` only when the condition holds;
/// - `fma_target!(if { a } else { b })` evaluates `a` when the condition
///   holds and `b` otherwise; only the chosen expression is compiled.
macro_rules! fma_target {
    (@with [$pred:meta] has) => {
        cfg!($pred)
    };
    (@with [$pred:meta] if { $yes:expr } else { $no:expr }) => {{
        #[cfg($pred)]
        let selected = $yes;
        #[cfg(not($pred))]
        let selected = $no;
        selected
    }};
    (@with [$pred:meta] $item:item) => {
        #[cfg($pred)]
        $item
    };
    ($($input:tt)*) => {
        fma_target! {
            @with [any(all(target_arch = "x86_64", target_feature = "fma"), all(target_arch = "aarch64", target_feature = "neon"))]
            $($input)*
        }
    };
}

/// True when `two_product` uses a fused multiply-add on this target.
///
/// That is the case on x86_64 when compiled with the `fma` target feature
/// (for example with `-C target-feature=+fma` or `-C target-cpu=native` on a
/// CPU that has it), and on aarch64 with the `neon` target feature, which
/// every aarch64 target enables by default except the soft-float ones.
///
/// # Examples
///
/// ```
/// use implicit_predicates_core::eft::HAS_FMA;
///
/// if HAS_FMA {
///     // `two_product` and `two_square` use `two_product_fma`.
/// }
/// ```
pub const HAS_FMA: bool = fma_target!(has);

/// `2^-480`, the smallest nonzero magnitude in the product domain. See
/// [`in_product_domain`].
///
/// # Examples
///
/// ```
/// use implicit_predicates_core::eft::PRODUCT_OPERAND_MIN;
///
/// assert_eq!(PRODUCT_OPERAND_MIN, 2f64.powi(-480));
/// ```
pub const PRODUCT_OPERAND_MIN: f64 = f64::from_bits((1023 - 480) << 52);

/// `2^480`, the largest magnitude in the product domain. See
/// [`in_product_domain`].
///
/// # Examples
///
/// ```
/// use implicit_predicates_core::eft::PRODUCT_OPERAND_MAX;
///
/// assert_eq!(PRODUCT_OPERAND_MAX, 2f64.powi(480));
/// ```
pub const PRODUCT_OPERAND_MAX: f64 = f64::from_bits((1023 + 480) << 52);

/// Returns `true` if `x` lies in the *product domain*: it is zero of either
/// sign, or its magnitude is in
/// [`PRODUCT_OPERAND_MIN`]` ..= `[`PRODUCT_OPERAND_MAX`], that is
/// `2^-480 ..= 2^480`.
///
/// Every operand of the exact products ([`two_product`],
/// [`two_product_dekker`], `two_product_fma` and [`two_square`]) must lie in
/// this domain, where neither the product nor its rounding error can
/// overflow or underflow. Outside it the error they return can be inexact,
/// without any other symptom. Debug builds check the domain on every exact
/// product and panic outside it; release builds do not check.
///
/// # Examples
///
/// ```
/// use implicit_predicates_core::eft::{PRODUCT_OPERAND_MAX, PRODUCT_OPERAND_MIN, in_product_domain};
///
/// assert!(in_product_domain(-0.0));
/// assert!(in_product_domain(PRODUCT_OPERAND_MIN) && in_product_domain(-PRODUCT_OPERAND_MAX));
/// assert!(!in_product_domain(PRODUCT_OPERAND_MIN / 2.0));
/// assert!(!in_product_domain(f64::MAX));
/// assert!(!in_product_domain(f64::NAN));
/// ```
#[inline]
#[must_use]
pub fn in_product_domain(x: f64) -> bool {
    x == 0.0 || (PRODUCT_OPERAND_MIN..=PRODUCT_OPERAND_MAX).contains(&x.abs())
}

/// `2^27 + 1`: multiplying by it splits an `f64` into two halves of at most
/// 26 significant bits each (Veltkamp's splitting).
const SPLITTER: f64 = 134_217_729.0;

/// Computes `a + b` and its rounding error (Knuth's two-sum).
///
/// Requirement: `a + b` does not overflow.
///
/// # Examples
///
/// ```
/// use implicit_predicates_core::eft::two_sum;
///
/// let (x, y) = two_sum(1.0, 1e-20);
/// assert_eq!(x, 1.0);
/// assert_eq!(y, 1e-20);
/// ```
#[inline]
#[must_use]
pub fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let x = a + b;
    let b_virtual = x - a;
    let a_virtual = x - b_virtual;
    let b_round = b - b_virtual;
    let a_round = a - a_virtual;
    (x, a_round + b_round)
}

/// Computes `a + b` and its rounding error, faster than [`two_sum`] when the
/// magnitudes are known to be ordered (Dekker's fast-two-sum).
///
/// Requirement: `|a| >= |b|` or `a == 0`, and `a + b` does not overflow.
///
/// # Examples
///
/// ```
/// use implicit_predicates_core::eft::fast_two_sum;
///
/// let (x, y) = fast_two_sum(1.0, 1e-20);
/// assert_eq!(x, 1.0);
/// assert_eq!(y, 1e-20);
/// ```
#[inline]
#[must_use]
pub fn fast_two_sum(a: f64, b: f64) -> (f64, f64) {
    let x = a + b;
    let b_virtual = x - a;
    (x, b - b_virtual)
}

/// Computes `a - b` and its rounding error.
///
/// Requirement: `a - b` does not overflow.
///
/// # Examples
///
/// ```
/// use implicit_predicates_core::eft::two_diff;
///
/// let (x, y) = two_diff(1.0, 1e-20);
/// assert_eq!(x, 1.0);
/// assert_eq!(y, -1e-20);
/// ```
#[inline]
#[must_use]
pub fn two_diff(a: f64, b: f64) -> (f64, f64) {
    // Negation is exact, and `a + (-b)` rounds exactly like `a - b`.
    two_sum(a, -b)
}

/// Computes `a * b` and its rounding error with Dekker's algorithm, using
/// only additions and multiplications.
///
/// Requirement: `a` and `b` lie in the product domain (see
/// [`in_product_domain`]); debug builds panic if they do not.
///
/// # Examples
///
/// ```
/// use implicit_predicates_core::eft::two_product_dekker;
///
/// let a = 1.0 + f64::EPSILON;
/// let (x, y) = two_product_dekker(a, a);
/// assert_eq!(x, 1.0 + 2.0 * f64::EPSILON);
/// assert_eq!(y, f64::EPSILON * f64::EPSILON);
/// ```
#[inline]
#[must_use]
pub fn two_product_dekker(a: f64, b: f64) -> (f64, f64) {
    debug_check_product_domain(a, b);
    let x = a * b;
    let (a_hi, a_lo) = split(a);
    let (b_hi, b_lo) = split(b);
    let err1 = x - a_hi * b_hi;
    let err2 = err1 - a_lo * b_hi;
    let err3 = err2 - a_hi * b_lo;
    (x, a_lo * b_lo - err3)
}

fma_target! {
    /// Computes `a * b` and its rounding error with one multiplication and one
    /// fused multiply-add.
    ///
    /// Only available when [`HAS_FMA`] is true. It returns a pair equal in
    /// value to that of [`two_product_dekker`]; the two may differ in the
    /// sign of a zero error (Dekker's algorithm can give `-0.0` where this
    /// one gives `+0.0`).
    ///
    /// Requirement: `a` and `b` lie in the product domain (see
    /// [`in_product_domain`]); debug builds panic if they do not.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(any(all(target_arch = "x86_64", target_feature = "fma"), all(target_arch = "aarch64", target_feature = "neon")))]
    /// # {
    /// use implicit_predicates_core::eft::two_product_fma;
    ///
    /// let a = 1.0 + f64::EPSILON;
    /// let (x, y) = two_product_fma(a, a);
    /// assert_eq!(x, 1.0 + 2.0 * f64::EPSILON);
    /// assert_eq!(y, f64::EPSILON * f64::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[must_use]
    // The `core::arch` intrinsics carry `#[target_feature]`, so calling them
    // from a function without that attribute needs `unsafe`.
    #[allow(unsafe_code)]
    pub fn two_product_fma(a: f64, b: f64) -> (f64, f64) {
        debug_check_product_domain(a, b);
        let x = a * b;
        // `a * b - x` is representable in the domain, so the fused operation
        // computes it without rounding.
        // SAFETY: this function only exists when the target has the features
        // the intrinsics need, enabled at compile time: `fma` on x86_64,
        // which implies the `sse2` the other intrinsics need (through
        // `avx`), and `neon` on aarch64.
        let y = unsafe {
            #[cfg(target_arch = "x86_64")]
            let y = {
                use core::arch::x86_64::{_mm_cvtsd_f64, _mm_fmsub_sd, _mm_set_sd};
                _mm_cvtsd_f64(_mm_fmsub_sd(_mm_set_sd(a), _mm_set_sd(b), _mm_set_sd(x)))
            };
            #[cfg(target_arch = "aarch64")]
            let y = {
                use core::arch::aarch64::{vdup_n_f64, vfma_f64, vget_lane_f64};
                // `vfma_f64(c, a, b)` is `c + a * b` with a single rounding.
                vget_lane_f64::<0>(vfma_f64(vdup_n_f64(-x), vdup_n_f64(a), vdup_n_f64(b)))
            };
            y
        };
        (x, y)
    }
}

/// Computes `a * b` and its rounding error with the crate's selected
/// implementation: `two_product_fma` when [`HAS_FMA`] is true, and
/// [`two_product_dekker`] otherwise. Both give equal values.
///
/// Requirement: `a` and `b` lie in the product domain (see
/// [`in_product_domain`]); debug builds panic if they do not.
///
/// # Examples
///
/// ```
/// use implicit_predicates_core::eft::two_product;
///
/// let a = 1.0 + f64::EPSILON;
/// let (x, y) = two_product(a, 1.0 - f64::EPSILON);
/// assert_eq!(x, 1.0);
/// assert_eq!(y, -f64::EPSILON * f64::EPSILON);
/// ```
#[inline]
#[must_use]
pub fn two_product(a: f64, b: f64) -> (f64, f64) {
    fma_target!(if { two_product_fma(a, b) } else { two_product_dekker(a, b) })
}

/// Computes `a * a` and its rounding error: with `two_product_fma` when
/// [`HAS_FMA`] is true, and otherwise with fewer operations than
/// [`two_product_dekker`].
///
/// Requirement: `a` lies in the product domain (see [`in_product_domain`]);
/// debug builds panic if it does not.
///
/// # Examples
///
/// ```
/// use implicit_predicates_core::eft::two_square;
///
/// let a = 1.0 + f64::EPSILON;
/// let (x, y) = two_square(a);
/// assert_eq!(x, 1.0 + 2.0 * f64::EPSILON);
/// assert_eq!(y, f64::EPSILON * f64::EPSILON);
/// ```
#[inline]
#[must_use]
pub fn two_square(a: f64) -> (f64, f64) {
    debug_check_product_domain(a, a);
    fma_target!(if { two_product_fma(a, a) } else {
        {
            let x = a * a;
            let (hi, lo) = split(a);
            let err1 = x - hi * hi;
            // `hi + hi` is exact, and `hi * lo` counts twice in `a * a`.
            let err3 = err1 - (hi + hi) * lo;
            (x, lo * lo - err3)
        }
    })
}

/// Panics in debug builds unless both operands of an exact product lie in
/// the product domain.
#[inline]
#[track_caller]
fn debug_check_product_domain(a: f64, b: f64) {
    debug_assert!(
        in_product_domain(a) && in_product_domain(b),
        "an operand of the exact product {a:e} * {b:e} is outside the product domain \
         (see `eft::in_product_domain`)"
    );
}

/// Veltkamp's splitting: returns `(hi, lo)` with `hi + lo == a` exactly and
/// each half fitting in 26 significant bits, so that products of halves are
/// exact. Requires `|a| * SPLITTER` not to overflow.
#[inline]
fn split(a: f64) -> (f64, f64) {
    let c = SPLITTER * a;
    let a_big = c - a;
    let hi = c - a_big;
    (hi, a - hi)
}
