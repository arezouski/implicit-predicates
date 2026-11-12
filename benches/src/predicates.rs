//! Input sets and contenders for the predicate benchmarks.
//!
//! The sets of [`orient3d_sets`] and [`insphere_sets`] are deterministic and
//! have [`SET_SIZE`] cases each, on explicit points. The `corpus` set is
//! drawn from the calls of the versioned sample corpora whose points are all
//! explicit, weighted by how many calls of the recording run each stored
//! call stands for. The sets of [`implicit_sets`], [`any_explicit_sets`] and
//! [`exact_sets`] hold points of any type of `implicit-predicates`, among
//! them the recorded calls with implicit points.
//!
//! The contenders are `implicit-predicates` (`orient3d` only, so far), the
//! `robust` and `geometry-predicates` crates and an unprotected `f64`
//! determinant ([`naive_orient3d`], [`naive_insphere`]), the cost of the
//! arithmetic a floating-point filter cannot avoid. The
//! benchmarks call every contender through a function pointer, so the time
//! of the naive determinant (about 2 ns for orient3d) is mostly the cost of
//! that call, which is the same for every contender.
//!
//! Every contender is exposed through a function that returns the sign in
//! the convention of the exact computation of `implicit-predicates-corpus`:
//!
//! - `orient3d` is the sign of `det[a - d; b - d; c - d]`;
//! - `insphere` is the sign of the 4x4 determinant whose rows are
//!   `(p - e, |p - e|^2)` for `p` in `a, b, c, d`.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use implicit_predicates_corpus::weighted::weighted_sample;
use implicit_predicates_corpus::{Call, Corpus, PointDef, Predicate};

use crate::Rng;

mod contenders;
mod implicit;

pub use contenders::{
    geometry_insphere_sign, geometry_orient3d_sign, implicit_predicates_orient3d_any_sign,
    implicit_predicates_orient3d_sign, naive_insphere, naive_insphere_sign, naive_orient3d,
    naive_orient3d_sign, robust_insphere_sign, robust_orient3d_sign,
};
pub use implicit::{
    CORPUS_DIR, EXACT_SEED, ExactSet, IMPLICIT_SEED, ImplicitInput, MAX_IMPLICIT,
    any_explicit_sets, exact_sets, implicit_recorded_signs, implicit_set_name, implicit_sets,
};

/// Four points of `orient3d`.
pub type Orient3dInput = [[f64; 3]; 4];

/// Five points of `insphere`.
pub type InSphereInput = [[f64; 3]; 5];

/// A named, ordered collection of benchmark cases.
#[derive(Clone, Debug)]
pub struct InputSet<T> {
    /// Name of the set, used in the benchmark identifiers.
    pub name: &'static str,
    /// The cases, in the order the benchmarks run them.
    pub cases: Vec<T>,
}

/// Number of cases of every set.
pub const SET_SIZE: usize = 1024;

/// Seed of the `random` sets.
pub const RANDOM_SEED: u64 = 0x7261_6e64;

/// Seed of the `near_degenerate` sets.
pub const NEAR_DEGENERATE_SEED: u64 = 0x6e65_6172;

/// Seed of the `degenerate` sets.
pub const DEGENERATE_SEED: u64 = 0x6465_6765;

/// Seed of the weighted resampling of the `corpus` sets.
pub const CORPUS_SEED: u64 = 0x636f_7270;

/// Magnitude bound of the random coordinates.
const RANDOM_SCALE: f64 = 1024.0;

/// Exponent of the grid step of the `near_degenerate` orient3d set.
const GRID_EXPONENT: i32 = -20;

/// Returns the `orient3d` sets: `random`, `near_degenerate`, `degenerate` and
/// `corpus`.
#[must_use]
pub fn orient3d_sets() -> Vec<InputSet<Orient3dInput>> {
    vec![
        InputSet {
            name: "random",
            cases: random_cases(RANDOM_SEED),
        },
        InputSet {
            name: "near_degenerate",
            cases: near_coplanar(NEAR_DEGENERATE_SEED),
        },
        InputSet {
            name: "degenerate",
            cases: coplanar(DEGENERATE_SEED),
        },
        corpus_set(Predicate::Orient3d),
    ]
}

/// Returns the `insphere` sets: `random`, `near_degenerate`, `degenerate` and
/// `corpus`.
#[must_use]
pub fn insphere_sets() -> Vec<InputSet<InSphereInput>> {
    vec![
        InputSet {
            name: "random",
            cases: random_cases(RANDOM_SEED),
        },
        InputSet {
            name: "near_degenerate",
            cases: near_cospherical(NEAR_DEGENERATE_SEED),
        },
        InputSet {
            name: "degenerate",
            cases: cospherical(DEGENERATE_SEED),
        },
        corpus_set(Predicate::InSphere),
    ]
}

/// A point with each coordinate a full 53-bit significand below 2^10 in
/// magnitude.
fn random_point(rng: &mut Rng) -> [f64; 3] {
    std::array::from_fn(|_| rng.unit() * RANDOM_SCALE)
}

/// [`SET_SIZE`] cases of `N` points drawn uniformly from the cube of
/// half-side 2^10.
fn random_cases<const N: usize>(seed: u64) -> Vec<[[f64; 3]; N]> {
    let mut rng = Rng::new(seed);
    (0..SET_SIZE)
        .map(|_| std::array::from_fn(|_| random_point(&mut rng)))
        .collect()
}

/// A uniform integer in `lo..=hi`.
fn int_in(rng: &mut Rng, lo: i64, hi: i64) -> i64 {
    lo + (rng.next_u64() % (hi - lo + 1) as u64) as i64
}

/// A point of the integer grid `-16..=16`.
fn grid_point(rng: &mut Rng) -> [f64; 3] {
    std::array::from_fn(|_| int_in(rng, -16, 16) as f64)
}

/// `a + s (b - a) + t (c - a)`, exact for the small integer grid.
fn combine(a: [f64; 3], b: [f64; 3], c: [f64; 3], s: f64, t: f64) -> [f64; 3] {
    std::array::from_fn(|i| a[i] + s * (b[i] - a[i]) + t * (c[i] - a[i]))
}

/// Exactly coplanar quadruples on an integer grid: `d` is an integer
/// combination of `a`, `b` and `c` with weights summing to one, so all
/// operations are exact. Coordinates stay below 2^7 in magnitude.
fn coplanar(seed: u64) -> Vec<Orient3dInput> {
    let mut rng = Rng::new(seed);
    (0..SET_SIZE)
        .map(|_| {
            let [a, b, c] = std::array::from_fn(|_| grid_point(&mut rng));
            let s = int_in(&mut rng, -3, 3) as f64;
            let t = int_in(&mut rng, -3, 3) as f64;
            [a, b, c, combine(a, b, c, s, t)]
        })
        .collect()
}

/// Moves `x` by `ulps` units in the last place, away from zero when `ulps`
/// is positive and towards it when negative. A zero is moved by 2^-52.
fn nudge(x: f64, ulps: i64) -> f64 {
    if x == 0.0 {
        return ulps as f64 * f64::EPSILON;
    }
    let bits = x.to_bits();
    f64::from_bits(bits.wrapping_add_signed(ulps))
}

/// A nonzero amount of 1 to 3 ulps with a random sign.
fn small_ulps(rng: &mut Rng) -> i64 {
    let magnitude = int_in(rng, 1, 3);
    if rng.next_u64() & 1 == 0 {
        magnitude
    } else {
        -magnitude
    }
}

/// Moves the coordinate of smallest magnitude of the case by a few ulps. Its
/// ulp is the smallest of the case, so the change in the determinant stays
/// well below the rounding error of the computations on the whole case.
fn perturb<const N: usize>(case: &mut [[f64; 3]; N], rng: &mut Rng) {
    let (mut point, mut axis) = (0, 0);
    for i in 0..N {
        for j in 0..3 {
            if case[i][j].abs() < case[point][axis].abs() {
                (point, axis) = (i, j);
            }
        }
    }
    case[point][axis] = nudge(case[point][axis], small_ulps(rng));
}

/// Quadruples that are exactly coplanar before one coordinate of one point is
/// moved by one to three ulps. The points are multiples of 2^-20 below 2^10
/// in magnitude and `d = a + b - c`, so the construction is exact; the
/// perturbation is far below what the static filter of the baselines can
/// resolve.
fn near_coplanar(seed: u64) -> Vec<Orient3dInput> {
    let mut rng = Rng::new(seed);
    let step = 2f64.powi(GRID_EXPONENT);
    (0..SET_SIZE)
        .map(|_| {
            let [a, b, c]: [[f64; 3]; 3] = std::array::from_fn(|_| {
                std::array::from_fn(|_| int_in(&mut rng, -(1 << 29), 1 << 29) as f64 * step)
            });
            let d = combine(a, b, c, 1.0, -1.0);
            let mut case = [a, b, c, d];
            perturb(&mut case, &mut rng);
            case
        })
        .collect()
}

/// Five distinct corners of a box with integer sides and an integer corner,
/// which are exactly cospherical. Coordinates stay below 2^5 in magnitude.
fn cospherical(seed: u64) -> Vec<InSphereInput> {
    let mut rng = Rng::new(seed);
    (0..SET_SIZE)
        .map(|_| {
            let origin = grid_point(&mut rng);
            let sides: [f64; 3] = std::array::from_fn(|_| int_in(&mut rng, 1, 8) as f64);
            // A partial Fisher-Yates shuffle of the eight corner masks.
            let mut masks: [usize; 8] = std::array::from_fn(|i| i);
            for i in 0..5 {
                let j = i + int_in(&mut rng, 0, 7 - i as i64) as usize;
                masks.swap(i, j);
            }
            std::array::from_fn(|i| {
                std::array::from_fn(|axis| {
                    origin[axis] + ((masks[i] >> axis) & 1) as f64 * sides[axis]
                })
            })
        })
        .collect()
}

/// Exactly cospherical quintuples, as in [`cospherical`], with one coordinate
/// of one point moved by one to three ulps.
fn near_cospherical(seed: u64) -> Vec<InSphereInput> {
    let mut rng = Rng::new(seed ^ 0x5eed);
    cospherical(seed)
        .into_iter()
        .map(|mut case| {
            perturb(&mut case, &mut rng);
            case
        })
        .collect()
}

/// The sample corpora of `corpus/samples/`, in file name order.
fn sample_corpora() -> Vec<Corpus> {
    read_corpora(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../corpus/samples"))
}

/// The corpus files of `dir`, in file name order.
///
/// # Panics
///
/// Panics if `dir` cannot be read, has no corpus file, or has an invalid
/// one.
fn read_corpora(dir: &Path) -> Vec<Corpus> {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| path.extension().is_some_and(|e| e == "ipcorpus"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no corpus file in {}", dir.display());
    files
        .iter()
        .map(|path| {
            let reader = BufReader::new(
                File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())),
            );
            Corpus::read(reader).unwrap_or_else(|e| panic!("{}: {e:?}", path.display()))
        })
        .collect()
}

/// The coordinates of the points of a call, if they are all explicit.
fn explicit_points(corpus: &Corpus, call: &Call) -> Option<Vec<[f64; 3]>> {
    call.points()
        .iter()
        .map(|&i| match corpus.points[i as usize] {
            PointDef::Explicit(c) => Some(c),
            _ => None,
        })
        .collect()
}

/// The `corpus` set: [`SET_SIZE`] calls of `predicate` on explicit points,
/// drawn from the sample corpora by deterministic weighted resampling with
/// seed [`CORPUS_SEED`] ([`weighted_sample`]): each stored call weighs
/// `total / kept` of its stratum, so the set reflects the mix of explicit
/// calls of the recording runs rather than the stratified sample stored in
/// the files.
fn corpus_set<const N: usize>(predicate: Predicate) -> InputSet<[[f64; 3]; N]> {
    let corpora = sample_corpora();
    let draws = weighted_sample(&corpora, SET_SIZE, CORPUS_SEED, |corpus, call| {
        call.predicate == predicate && explicit_points(corpus, call).is_some()
    });
    assert_eq!(
        draws.len(),
        SET_SIZE,
        "no explicit {predicate:?} call in the samples"
    );
    let cases = draws
        .into_iter()
        .map(|(c, i)| {
            let points = explicit_points(&corpora[c], &corpora[c].calls[i]).expect("explicit");
            std::array::from_fn(|k| points[k])
        })
        .collect();
    InputSet {
        name: "corpus",
        cases,
    }
}
