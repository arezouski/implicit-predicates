//! Benchmarks for the arithmetic core of `implicit-predicates`.
//!
//! The benchmark files live in `benches/`, one per group:
//!
//! - `eft`: `two_product` with Dekker's algorithm and, when the target has a
//!   hardware fused multiply-add, with it;
//! - `expansion`: `sum`, `scale` and `product` on expansions of fixed length,
//!   compared with `geometry-predicates` where it has an equivalent;
//! - `det3`: an exact 3x3 determinant built on the arena, over random,
//!   nearly singular and exactly singular matrices, which shows what the
//!   fused multiply-add changes in a realistic workload;
//! - `predicates`: `orient3d` and `insphere` on explicit points, for
//!   `implicit-predicates`, the `robust` and `geometry-predicates` crates and
//!   an unprotected `f64` determinant, over random, nearly degenerate,
//!   exactly degenerate and recorded inputs, then `orient3d` on points of
//!   type `AnyPoint`: the same explicit sets and the recorded calls with
//!   implicit points. Adding a contender is adding a row to the table in
//!   that file;
//! - `orient3d_stages`: each stage of the generated `orient3d` alone, the
//!   filter on the random set and the exact stages on the nearly degenerate
//!   and exactly degenerate sets; the filter of points of any type on the
//!   recorded calls with implicit points, and the exact stages over arena
//!   spans on those of them that the filter does not decide.
//!
//! This library holds the deterministic input builders and the seeds they
//! use, and the determinant routine, shared by the benchmarks and by the
//! tests that check them, so that every benchmark measures the same data on
//! every run and the measured computation is the one that is verified.
//! Nothing here is part of the published crates.
//!
//! Run everything with `cargo bench -p implicit-predicates-benches`, and
//! with `RUSTFLAGS="-C target-cpu=native"` to enable the fused
//! multiply-add on a CPU that has it.

pub mod predicates;

use implicit_predicates_core::{Scratch, Sign, Span, expansion};

/// Expansion lengths of the `sum` and `scale` benchmarks.
pub const LENGTHS: [usize; 6] = [2, 4, 8, 16, 32, 64];

/// Expansion lengths of the `product` benchmark.
pub const PRODUCT_LENGTHS: [usize; 4] = [4, 8, 16, 32];

/// Seed of the first operand of `sum`, and the operand of `scale` and
/// `product`.
pub const FIRST_OPERAND_SEED: u64 = 1;

/// Seed of the second operand of `sum` and `product`.
pub const SECOND_OPERAND_SEED: u64 = 2;

/// Seed of the scalar of `scale`.
pub const SCALE_SEED: u64 = 3;

/// Seed of the `two_product` pairs.
pub const PAIRS_SEED: u64 = 0x7770_7072_6f64;

/// Seed of the `det3` matrix sets.
pub const MATRICES_SEED: u64 = 0x6465_7433;

/// Capacity, in components, of the arena [`det3_sign`] needs.
pub const DET3_CAPACITY: usize = 1024;

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

    /// Returns a uniform multiple of 2^-52 in `[-1, 1)`.
    pub fn unit(&mut self) -> f64 {
        // 53 random bits times 2^-52 are an exact multiple of 2^-52 in
        // [0, 2).
        let bits = (self.next_u64() >> 11) as f64;
        bits * f64::EPSILON - 1.0
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
/// the exponents `-480..416`, inside the product domain of
/// `eft::in_product_domain` where `scale` and `product` are supported. Each component has a random odd significand and a random sign,
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

/// Returns [`MATRIX_COUNT`] generic matrices with entries uniform in
/// `[-1, 1)`; their determinants are not zero.
#[must_use]
pub fn random_matrices(seed: u64) -> Vec<Matrix3> {
    let mut rng = Rng::new(seed);
    (0..MATRIX_COUNT)
        .map(|_| std::array::from_fn(|_| std::array::from_fn(|_| rng.unit())))
        .collect()
}

/// Returns [`MATRIX_COUNT`] exactly singular matrices: the third row is the
/// sum of the first two. The entries are multiples of 2^-52 below 1 in
/// magnitude, so the sum is exact in `f64` and the determinant is exactly
/// zero.
#[must_use]
pub fn singular_matrices(seed: u64) -> Vec<Matrix3> {
    random_matrices(seed)
        .into_iter()
        .map(|mut m| {
            m[2] = std::array::from_fn(|j| m[0][j] + m[1][j]);
            m
        })
        .collect()
}

/// Returns [`MATRIX_COUNT`] nearly singular matrices: those of
/// [`singular_matrices`] with 2^-45 added to one entry of the third row, so
/// that the determinant is tiny but, for these seeds, not zero.
#[must_use]
pub fn near_singular_matrices(seed: u64) -> Vec<Matrix3> {
    singular_matrices(seed)
        .into_iter()
        .map(|mut m| {
            m[2][0] += 2f64.powi(-45);
            m
        })
        .collect()
}

/// Pushes the exact value of `x*y - z*w` on the arena.
fn minor(scratch: &mut Scratch<'_>, [x, y]: [f64; 2], [z, w]: [f64; 2]) -> Span {
    let mut term = [0.0; 2];
    let len = expansion::from_two_product(x, y, &mut term);
    let first = scratch.push(&term[..len]).expect("arena capacity");
    let len = expansion::from_two_product(z, w, &mut term);
    let second = scratch.push(&term[..len]).expect("arena capacity");
    scratch.diff(first, second).expect("arena capacity")
}

/// Returns the exact sign of the determinant of `m`, evaluated on `scratch`
/// (which needs [`DET3_CAPACITY`] free components) and left as it was found.
///
/// Six products, three differences, three scalings and two sums, expanded
/// along the first row.
///
/// # Panics
///
/// Panics if the arena is too small.
pub fn det3_sign(scratch: &mut Scratch<'_>, m: &Matrix3) -> Sign {
    let mark = scratch.mark();
    let [a, b, c] = *m;
    let m0 = minor(scratch, [b[1], c[2]], [b[2], c[1]]);
    let m1 = minor(scratch, [b[0], c[2]], [b[2], c[0]]);
    let m2 = minor(scratch, [b[0], c[1]], [b[1], c[0]]);
    let t0 = scratch.scale(m0, a[0]).expect("arena capacity");
    let t1 = scratch.scale(m1, -a[1]).expect("arena capacity");
    let t2 = scratch.scale(m2, a[2]).expect("arena capacity");
    let partial = scratch.sum(t0, t1).expect("arena capacity");
    let det = scratch.sum(partial, t2).expect("arena capacity");
    let sign = scratch.sign(det);
    scratch.reset(mark);
    sign
}

/// Returns the two operands of a `product` benchmark of length `len`.
#[must_use]
pub fn product_operands(len: usize) -> (Vec<f64>, Vec<f64>) {
    (
        valid_expansion(FIRST_OPERAND_SEED, len),
        valid_expansion(SECOND_OPERAND_SEED, len),
    )
}

/// Capacity, in components, of the arena for one `product` of operands of
/// `len` components each: the operands, the working space and the result.
#[must_use]
pub fn product_capacity(len: usize) -> usize {
    2 * len + 4 * len * len + 4 * len
}
