//! The grids of points, the order of the `f64`s and the pseudo-random
//! sequence of the search for edge cases ([`super::edges`]).

use super::edges::Case;

/// The grid integers are in `-GRID..=GRID`; with the affine combination,
/// every coordinate is below `2^8` in magnitude, and so is every
/// difference of two of them, before scaling.
pub(super) const GRID: i64 = 16;

/// `count` points on the grid of integers in `-GRID..=GRID` times
/// `2^scale`, the last an integer affine combination of the first three
/// when there are at least four.
pub(super) fn grid_points(random: &mut SplitMix64, count: usize, scale: i32) -> Case {
    let mut integer = || random.below(2 * GRID as u64 + 1) as i64 - GRID;
    let mut p: Vec<[i64; 3]> = (0..count)
        .map(|_| [integer(), integer(), integer()])
        .collect();
    if count >= 4 {
        let (s, t) = (random.below(5) as i64 - 2, random.below(5) as i64 - 2);
        let (a, b, c) = (p[0], p[1], p[2]);
        p[count - 1] = std::array::from_fn(|i| a[i] + s * (b[i] - a[i]) + t * (c[i] - a[i]));
    }
    // The integers are below 2^8, so the products are exact.
    let unit = pow2(scale);
    p.iter()
        .map(|point| point.map(|n| n as f64 * unit))
        .collect()
}

/// The largest key of a finite `f64`: that of `f64::MAX`.
pub(super) const FINITE_KEYS: i128 = 0x7fef_ffff_ffff_ffff;

/// The position of `x` in the order of the finite `f64`s, with both zeros
/// at 0: consecutive keys are consecutive values.
pub(super) fn key(x: f64) -> i128 {
    let magnitude = i128::from(x.to_bits() & !(1 << 63));
    if x.is_sign_negative() {
        -magnitude
    } else {
        magnitude
    }
}

/// The `f64` of key `key`, which must be at most [`FINITE_KEYS`] in
/// magnitude.
pub(super) fn from_key(key: i128) -> f64 {
    let magnitude = u64::try_from(key.unsigned_abs()).expect("a finite key");
    let sign = if key < 0 { 1 << 63 } else { 0 };
    f64::from_bits(sign | magnitude)
}

/// The exponent of a power of two.
pub(super) fn exponent(x: f64) -> i32 {
    i32::try_from(x.to_bits() >> 52).expect("11 bits") - 1023
}

/// `2^exponent`, for a normal power of two.
pub(super) fn pow2(exponent: i32) -> f64 {
    f64::from_bits(u64::try_from(exponent + 1023).expect("a normal power of two") << 52)
}

/// The SplitMix64 sequence of pseudo-random numbers.
pub(super) struct SplitMix64(pub(super) u64);

impl SplitMix64 {
    pub(super) fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A number in `0..n`, nearly uniform for small `n`.
    pub(super) fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}
