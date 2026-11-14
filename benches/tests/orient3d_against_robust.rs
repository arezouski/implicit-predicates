//! A long comparison of `implicit_predicates::orient3d` with
//! `robust::orient3d`, which is exact while its arithmetic neither
//! overflows nor underflows, on inputs whose coordinates span many binades.
//!
//! The run is ignored by default; run it in release mode with
//! `cargo test -p implicit-predicates-benches --release -- --ignored
//! --nocapture`, which also shows the counts it prints.

use implicit_predicates::stages::orient3d::TRANSLATED_RANGE;
use implicit_predicates_benches::Rng;
use implicit_predicates_benches::predicates::{Orient3dInput, robust_orient3d_sign};
use implicit_predicates_corpus::exact;
use implicit_predicates_oracle::Dyadic;

/// Number of cases of the long run.
const CASES: usize = 10_000_000;

/// Seed of the long run.
const SEED: u64 = 0x6c6f_6e67;

/// The binary exponents of the scale factors. Every coordinate built below
/// is zero or below 2^12 in magnitude and at least 2^-52 times its scale,
/// so with these scales every coordinate stays well inside the range where
/// `orient3d` decides, and the products of `robust` neither overflow nor
/// underflow.
const SCALE_EXPONENTS: (i32, i32) = (-130, 220);

/// A uniform integer in `lo..=hi`.
fn int_in(rng: &mut Rng, lo: i64, hi: i64) -> i64 {
    lo + (rng.next_u64() % (hi - lo + 1) as u64) as i64
}

/// A random power of two in the range of [`SCALE_EXPONENTS`].
fn scale(rng: &mut Rng) -> f64 {
    let (lo, hi) = SCALE_EXPONENTS;
    2f64.powi(int_in(rng, i64::from(lo), i64::from(hi)) as i32)
}

/// A point with integer coordinates in `-limit..=limit`.
fn int_point(rng: &mut Rng, limit: i64) -> [f64; 3] {
    std::array::from_fn(|_| int_in(rng, -limit, limit) as f64)
}

/// Points with full 53-bit significands below 1 in magnitude.
fn random(rng: &mut Rng) -> Orient3dInput {
    std::array::from_fn(|_| std::array::from_fn(|_| rng.unit()))
}

/// An exactly coplanar quadruple: `d` is an integer affine combination of
/// `a`, `b` and `c`, with integer coordinates below 2^12, so every
/// operation is exact.
fn coplanar(rng: &mut Rng) -> Orient3dInput {
    let [a, b, c] = std::array::from_fn(|_| int_point(rng, 1 << 8));
    let (s, t) = (int_in(rng, -3, 3) as f64, int_in(rng, -3, 3) as f64);
    let d = std::array::from_fn(|i| a[i] + s * (b[i] - a[i]) + t * (c[i] - a[i]));
    [a, b, c, d]
}

/// A coplanar quadruple with one coordinate moved by one to three ulps.
fn near_coplanar(rng: &mut Rng) -> Orient3dInput {
    let mut p = coplanar(rng);
    let (point, axis) = (int_in(rng, 0, 3) as usize, int_in(rng, 0, 2) as usize);
    let ulps = int_in(rng, 1, 3);
    let x = p[point][axis];
    p[point][axis] = if x == 0.0 {
        ulps as f64 * f64::EPSILON
    } else if rng.next_u64() & 1 == 0 {
        f64::from_bits(x.to_bits() + ulps as u64)
    } else {
        f64::from_bits(x.to_bits() - ulps as u64)
    };
    p
}

/// Points of the small integer grid `-4..=4`: many repeated, collinear and
/// coplanar configurations.
fn grid(rng: &mut Rng) -> Orient3dInput {
    std::array::from_fn(|_| int_point(rng, 4))
}

/// One case: a family chosen in turn, every coordinate multiplied by a
/// random power of two, which is exact and keeps coplanarity.
fn case(rng: &mut Rng, index: usize) -> Orient3dInput {
    let points = match index % 4 {
        0 => random(rng),
        1 => near_coplanar(rng),
        2 => coplanar(rng),
        _ => grid(rng),
    };
    let s = scale(rng);
    points.map(|p| p.map(|x| x * s))
}

fn in_range(x: f64) -> bool {
    let (min, max) = TRANSLATED_RANGE;
    x == 0.0 || (min..=max).contains(&x.abs())
}

fn exact_sign(p: &Orient3dInput) -> i8 {
    let [a, b, c, d] = p.map(|q| q.map(Dyadic::from_f64));
    exact::orient3d(&a, &b, &c, &d)
}

#[test]
#[ignore = "ten million cases; run in release mode"]
fn orient3d_matches_robust_on_a_long_run() {
    let mut rng = Rng::new(SEED);
    let mut zero = 0usize;
    for index in 0..CASES {
        let p = case(&mut rng, index);
        assert!(p.iter().flatten().all(|&x| in_range(x)), "{p:?}");
        let [a, b, c, d] = &p;
        let ours = i8::from(implicit_predicates::orient3d(a, b, c, d));
        let theirs = robust_orient3d_sign(&p);
        assert_eq!(
            ours,
            theirs,
            "case {index}: exact sign {}, bits {:x?}",
            exact_sign(&p),
            p.map(|q| q.map(f64::to_bits))
        );
        zero += usize::from(ours == 0);
    }
    println!("{CASES} cases, {zero} with sign zero, no mismatch");
}
