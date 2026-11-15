//! A long comparison of `implicit_predicates::insphere` on explicit points
//! with `robust::insphere` and `geometry_predicates::insphere`, which are
//! exact while their arithmetic neither overflows nor underflows, on random,
//! nearly cospherical and grid inputs.
//!
//! The run is ignored by default; run it in release mode with
//! `cargo test -p implicit-predicates-benches --release --test
//! insphere_against_robust -- --ignored --nocapture`, which also shows the
//! counts it prints.

use implicit_predicates_benches::Rng;
use implicit_predicates_benches::predicates::{
    InSphereInput, geometry_insphere_sign, robust_insphere_sign,
};
use implicit_predicates_corpus::exact;
use implicit_predicates_oracle::Dyadic;

/// Number of cases of the long run, split evenly between the families (rounded up).
const CASES: usize = 10_000_000;

/// Number of cases of each family.
const PER_FAMILY: usize = CASES.div_ceil(3);

/// Cases per thread task; each task has its own generator.
const CHUNK: usize = 50_000;

/// Seed of the long run.
const SEED: u64 = 0x696e_7370;

/// The binary exponents of the scale factors. Every coordinate built below
/// is zero or below 2^12 in magnitude, and the degree of insphere is 5, so
/// with these scales the products of the reference libraries neither overflow
/// nor underflow.
const SCALE_EXPONENTS: (i64, i64) = (-40, 40);

/// Least number of zero signs the grid family must produce.
const MIN_GRID_ZEROS: usize = 300_000;

/// A uniform integer in `lo..=hi`.
fn int_in(rng: &mut Rng, lo: i64, hi: i64) -> i64 {
    lo + (rng.next_u64() % (hi - lo + 1) as u64) as i64
}

/// A random power of two in the range of [`SCALE_EXPONENTS`].
fn scale(rng: &mut Rng) -> f64 {
    let (lo, hi) = SCALE_EXPONENTS;
    2f64.powi(int_in(rng, lo, hi) as i32)
}

/// A point with integer coordinates in `-limit..=limit`.
fn int_point(rng: &mut Rng, limit: i64) -> [f64; 3] {
    std::array::from_fn(|_| int_in(rng, -limit, limit) as f64)
}

/// Points with full 53-bit significands below 1 in magnitude.
fn random(rng: &mut Rng) -> InSphereInput {
    std::array::from_fn(|_| std::array::from_fn(|_| rng.unit()))
}

/// Five points that are exactly cospherical: a center plus five signed
/// permutations of one integer offset.
fn cospherical(rng: &mut Rng) -> InSphereInput {
    const PERMUTATIONS: [[usize; 3]; 6] = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    let center = int_point(rng, 1 << 8);
    let offset = int_point(rng, 1 << 8);
    let first = rng.next_u64() as usize;
    std::array::from_fn(|j| {
        let choice = first.wrapping_add(5 * j);
        let order = PERMUTATIONS[choice % 6];
        let signs = rng.next_u64();
        std::array::from_fn(|k| {
            let o = offset[order[k]];
            center[k] + if signs >> k & 1 == 1 { -o } else { o }
        })
    })
}

/// A cospherical set with one coordinate moved by one to three ulps.
fn near_cospherical(rng: &mut Rng) -> InSphereInput {
    let mut p = cospherical(rng);
    let (point, axis) = (int_in(rng, 0, 4) as usize, int_in(rng, 0, 2) as usize);
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

/// Points of the small integer grid `-2..=2`: many repeated, coplanar and
/// cospherical configurations.
fn grid(rng: &mut Rng) -> InSphereInput {
    std::array::from_fn(|_| int_point(rng, 2))
}

/// The families, in the order of their counts.
const FAMILIES: [&str; 3] = ["random", "near cospherical", "grid"];

/// One case of a family, every coordinate multiplied by a random power of
/// two, which is exact and keeps cosphericity.
fn case(rng: &mut Rng, family: usize) -> InSphereInput {
    let points = match family {
        0 => random(rng),
        1 => near_cospherical(rng),
        _ => grid(rng),
    };
    let s = scale(rng);
    points.map(|p| p.map(|x| x * s))
}

fn exact_sign(p: &InSphereInput) -> i8 {
    let [a, b, c, d, e] = p.each_ref().map(|q| q.map(Dyadic::from_f64));
    exact::insphere(&a, &b, &c, &d, &e)
}

/// Compares `count` cases of one family, from the generator `seed`, and
/// returns the number of zero signs.
fn run_chunk(family: usize, seed: u64, count: usize) -> usize {
    let mut rng = Rng::new(seed);
    let mut zero = 0;
    for index in 0..count {
        let p = case(&mut rng, family);
        let [a, b, c, d, e] = &p;
        let ours = i8::from(implicit_predicates::insphere(a, b, c, d, e));
        let robust = robust_insphere_sign(&p);
        let geometry = geometry_insphere_sign(&p);
        assert!(
            ours == robust && ours == geometry,
            "{} case {index} of chunk {seed:x}: ours {ours}, robust {robust}, \
             geometry-predicates {geometry}, exact {}, bits {:x?}",
            FAMILIES[family],
            exact_sign(&p),
            p.map(|q| q.map(f64::to_bits))
        );
        zero += usize::from(ours == 0);
    }
    zero
}

/// Compares every case of one family on all the cores, and returns the
/// number of zero signs.
fn run_family(family: usize) -> usize {
    let chunks: Vec<(u64, usize)> = (0..PER_FAMILY.div_ceil(CHUNK))
        .map(|i| {
            let seed = SEED ^ ((family as u64) << 32) ^ i as u64;
            (seed, CHUNK.min(PER_FAMILY - i * CHUNK))
        })
        .collect();
    let threads = std::thread::available_parallelism().map_or(1, usize::from);
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    let mut zero = 0;
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(&(seed, count)) = chunks.get(i) else {
                            return zero;
                        };
                        zero += run_chunk(family, seed, count);
                    }
                })
            })
            .collect();
        workers.into_iter().map(|w| w.join().unwrap()).sum()
    })
}

#[test]
#[ignore = "ten million cases; run in release mode"]
fn insphere_matches_robust_and_geometry_predicates_on_a_long_run() {
    let mut zeros = [0; 3];
    for family in 0..3 {
        zeros[family] = run_family(family);
        println!(
            "{}: {PER_FAMILY} cases, {} with sign zero, no mismatch",
            FAMILIES[family], zeros[family]
        );
    }
    assert!(
        zeros[2] >= MIN_GRID_ZEROS,
        "only {} zero signs in the grid family",
        zeros[2]
    );
    assert!(zeros[1] > 0, "the near cospherical family never hit zero");
}
