//! Benchmarks for the arithmetic core of `indirect-predicates`.
//!
//! The benchmark files live in `benches/`, one per group:
//!
//! - `eft`: `two_product` with Dekker's algorithm and, when the target has a
//!   hardware fused multiply-add, with it;
//! - `expansion`: `sum`, `scale` and `product` on expansions of fixed length,
//!   compared with `geometry-predicates` where it has an equivalent;
//! - `det3`: an exact 3x3 determinant built on the arena, which shows what the
//!   fused multiply-add changes in a realistic workload.
//!
//! This library holds the deterministic input builders they share, so that
//! every benchmark measures the same data on every run. Nothing here is part
//! of the published crates.
//!
//! Run everything with `cargo bench -p indirect-predicates-benches`, and
//! with `RUSTFLAGS="-C target-cpu=native"` to enable the fused
//! multiply-add on a CPU that has it.

/// Number of pairs in the `two_product` benchmark.
pub const PAIR_COUNT: usize = 1024;

/// Number of matrices in each `det3` set.
pub const MATRIX_COUNT: usize = 256;

/// A small deterministic pseudo-random generator (SplitMix64).
///
/// It exists so that the benchmarks do not depend on a random number crate
/// and always see the same inputs for the same seed.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    /// Creates a generator from `seed`.
    #[must_use]
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    /// Returns the next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Returns a uniform value in `[-1, 1)`.
    pub fn unit(&mut self) -> f64 {
        // 53 random bits make an exact multiple of 2^-53 in [0, 1).
        let bits = (self.next_u64() >> 11) as f64;
        2.0 * bits * f64::EPSILON / 2.0 - 1.0
    }

    /// Returns a nonzero value with a random sign, a random 53-bit
    /// significand and a binary exponent in `-exponent_limit..exponent_limit`.
    pub fn scaled(&mut self, exponent_limit: i32) -> f64 {
        let bits = self.next_u64();
        let significand = 1.0 + ((bits >> 11) as f64) * (f64::EPSILON / 2.0);
        let span = u64::from(exponent_limit.unsigned_abs()) * 2;
        let exponent = (self.next_u64() % span) as i32 - exponent_limit;
        let magnitude = significand * 2f64.powi(exponent);
        if bits & 1 == 0 { magnitude } else { -magnitude }
    }
}

/// Returns [`PAIR_COUNT`] pairs of factors for `two_product`, with binary
/// exponents in `-20..20`, well inside the supported domain.
#[must_use]
pub fn product_pairs(seed: u64) -> Vec<(f64, f64)> {
    let mut rng = Rng::new(seed);
    (0..PAIR_COUNT)
        .map(|_| (rng.scaled(20), rng.scaled(20)))
        .collect()
}

/// Returns a valid expansion of exactly `len` components: strictly
/// increasing magnitude, nonoverlapping and without zeros.
///
/// The components occupy consecutive, disjoint bit ranges (slots) spread over
/// the exponents `-480..416`, the range where `scale` and `product` are
/// supported. Each component has a random odd significand and a random sign,
/// as wide as its slot allows (53 bits at most) but of random width, so two
/// expansions built from different seeds interleave when merged.
///
/// # Panics
///
/// Panics if `len` is zero or more than 64.
#[must_use]
pub fn valid_expansion(seed: u64, len: usize) -> Vec<f64> {
    assert!((1..=64).contains(&len), "length must be in 1..=64");
    let mut rng = Rng::new(seed);
    // Slot width in bits; 14 bits for the longest expansion, 53 or more
    // (capped) for the shortest.
    let slot = 896 / len as i32;
    let max_width = (slot - 1).min(53) as u32;
    (0..len)
        .map(|i| {
            let width = 1 + (rng.next_u64() % u64::from(max_width)) as u32;
            // An odd integer below 2^width: the top bit is free, the lowest
            // bit is set so the component is never zero.
            let significand = (rng.next_u64() & ((1u64 << width) - 1)) | 1;
            let sign = if rng.next_u64() & 1 == 0 { 1.0 } else { -1.0 };
            sign * significand as f64 * 2f64.powi(-480 + slot * i as i32)
        })
        .collect()
}

/// Returns a scalar for `scale`, with a random sign and exponent in
/// `-20..20`.
#[must_use]
pub fn scale_factor(seed: u64) -> f64 {
    Rng::new(seed).scaled(20)
}

/// A 3x3 matrix of `f64`, by rows.
pub type Matrix3 = [[f64; 3]; 3];

/// Returns [`MATRIX_COUNT`] matrices with entries uniform in `[-1, 1)`.
#[must_use]
pub fn random_matrices(seed: u64) -> Vec<Matrix3> {
    let mut rng = Rng::new(seed);
    (0..MATRIX_COUNT)
        .map(|_| std::array::from_fn(|_| std::array::from_fn(|_| rng.unit())))
        .collect()
}

/// Returns [`MATRIX_COUNT`] nearly singular matrices: the third row is the
/// sum of the first two, rounded in `f64`, so the determinant is tiny and
/// usually nonzero.
#[must_use]
pub fn near_singular_matrices(seed: u64) -> Vec<Matrix3> {
    random_matrices(seed)
        .into_iter()
        .map(|mut m| {
            m[2] = std::array::from_fn(|j| m[0][j] + m[1][j]);
            m
        })
        .collect()
}
