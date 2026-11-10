//! Input sets and contenders for the explicit-point predicate benchmarks.
//!
//! The sets of [`orient3d_sets`] and [`insphere_sets`] are deterministic:
//! each has [`SET_SIZE`] cases, except `corpus`, which holds the calls of the
//! versioned sample corpora whose points are all explicit. The contenders
//! are the `robust` and `geometry-predicates` crates and an unprotected `f64`
//! determinant ([`naive_orient3d`], [`naive_insphere`]). Every contender is
//! exposed through a function that returns the sign in the convention of the
//! exact computation of `indirect-predicates-corpus`:
//!
//! - `orient3d` is the sign of `det[a - d; b - d; c - d]`;
//! - `insphere` is the sign of the 4x4 determinant whose rows are
//!   `(p - e, |p - e|^2)` for `p` in `a, b, c, d`.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use indirect_predicates_corpus::{Corpus, PointDef, Predicate};

use crate::Rng;

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

/// Number of cases of every generated set, and the most the `corpus` set
/// holds.
pub const SET_SIZE: usize = 1024;

/// Seed of the `random` sets.
pub const RANDOM_SEED: u64 = 0x7261_6e64;

/// Seed of the `near_degenerate` sets.
pub const NEAR_DEGENERATE_SEED: u64 = 0x6e65_6172;

/// Seed of the `degenerate` sets.
pub const DEGENERATE_SEED: u64 = 0x6465_6765;

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

/// The explicit-only calls of one predicate in the sample corpora, in file
/// order.
fn corpus_cases<const N: usize>(predicate: Predicate) -> Vec<[[f64; 3]; N]> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../corpus/samples");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("the sample corpora directory")
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| path.extension().is_some_and(|e| e == "ipcorpus"))
        .collect();
    files.sort();
    let mut cases = Vec::new();
    for path in files {
        let reader = BufReader::new(File::open(&path).expect("a sample corpus"));
        let corpus = Corpus::read(reader).expect("a valid sample corpus");
        for call in corpus.calls.iter().filter(|c| c.predicate == predicate) {
            let explicit: Option<Vec<[f64; 3]>> = call
                .points()
                .iter()
                .map(|&i| match corpus.points[i as usize] {
                    PointDef::Explicit(c) => Some(c),
                    _ => None,
                })
                .collect();
            if let Some(points) = explicit {
                cases.push(std::array::from_fn(|i| points[i]));
            }
        }
    }
    cases
}

/// The `corpus` set: [`SET_SIZE`] explicit-only calls spread evenly over the
/// sample corpora, or all of them, with their count in the name, if there are
/// fewer.
fn corpus_set<const N: usize>(predicate: Predicate) -> InputSet<[[f64; 3]; N]> {
    let all = corpus_cases::<N>(predicate);
    if all.len() <= SET_SIZE {
        let name = if all.len() == SET_SIZE {
            "corpus"
        } else {
            Box::leak(format!("corpus_{}", all.len()).into_boxed_str())
        };
        return InputSet { name, cases: all };
    }
    let cases = (0..SET_SIZE)
        .map(|i| all[i * all.len() / SET_SIZE])
        .collect();
    InputSet {
        name: "corpus",
        cases,
    }
}

/// The sign, 1, 0 or -1, of a value.
fn sign(x: f64) -> i8 {
    if x > 0.0 {
        1
    } else if x < 0.0 {
        -1
    } else {
        0
    }
}

/// Multiplier that maps the sign `robust` returns for `orient3d` to the
/// convention above.
const ROBUST_ORIENT3D_FACTOR: i8 = 1;

/// Multiplier for `robust::insphere`.
const ROBUST_INSPHERE_FACTOR: i8 = 1;

/// Multiplier for `geometry_predicates::orient3d`.
const GEOMETRY_ORIENT3D_FACTOR: i8 = 1;

/// Multiplier for `geometry_predicates::insphere`.
const GEOMETRY_INSPHERE_FACTOR: i8 = 1;

fn coord(p: &[f64; 3]) -> robust::Coord3D<f64> {
    robust::Coord3D {
        x: p[0],
        y: p[1],
        z: p[2],
    }
}

/// `orient3d` of the `robust` crate.
#[must_use]
pub fn robust_orient3d_sign(p: &Orient3dInput) -> i8 {
    let [a, b, c, d] = p;
    ROBUST_ORIENT3D_FACTOR * sign(robust::orient3d(coord(a), coord(b), coord(c), coord(d)))
}

/// `insphere` of the `robust` crate.
#[must_use]
pub fn robust_insphere_sign(p: &InSphereInput) -> i8 {
    let [a, b, c, d, e] = p;
    ROBUST_INSPHERE_FACTOR
        * sign(robust::insphere(
            coord(a),
            coord(b),
            coord(c),
            coord(d),
            coord(e),
        ))
}

/// `orient3d` of the `geometry-predicates` crate.
#[must_use]
pub fn geometry_orient3d_sign(p: &Orient3dInput) -> i8 {
    let [a, b, c, d] = *p;
    GEOMETRY_ORIENT3D_FACTOR * sign(geometry_predicates::orient3d(a, b, c, d))
}

/// `insphere` of the `geometry-predicates` crate.
#[must_use]
pub fn geometry_insphere_sign(p: &InSphereInput) -> i8 {
    let [a, b, c, d, e] = *p;
    GEOMETRY_INSPHERE_FACTOR * sign(geometry_predicates::insphere(a, b, c, d, e))
}

fn sub(a: &[f64; 3], b: &[f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}

/// 3x3 determinant by cofactors along the first row, in plain `f64`.
fn det3(r: [&[f64; 3]; 3]) -> f64 {
    r[0][0] * (r[1][1] * r[2][2] - r[1][2] * r[2][1])
        - r[0][1] * (r[1][0] * r[2][2] - r[1][2] * r[2][0])
        + r[0][2] * (r[1][0] * r[2][1] - r[1][1] * r[2][0])
}

/// `det[a - d; b - d; c - d]` in plain `f64`, with no error bound: its sign is
/// wrong on nearly degenerate inputs.
#[must_use]
pub fn naive_orient3d(p: &Orient3dInput) -> f64 {
    let [a, b, c, d] = p;
    det3([&sub(a, d), &sub(b, d), &sub(c, d)])
}

/// The 4x4 determinant with rows `(p - e, |p - e|^2)` in plain `f64`, with no
/// error bound: its sign is wrong on nearly degenerate inputs.
#[must_use]
pub fn naive_insphere(p: &InSphereInput) -> f64 {
    let [a, b, c, d, e] = p;
    let rows = [sub(a, e), sub(b, e), sub(c, e), sub(d, e)];
    let lifted = rows.map(|r| r[0] * r[0] + r[1] * r[1] + r[2] * r[2]);
    // Expansion along the last column; the entry of row i has sign
    // (-1)^(i + 3).
    -lifted[0] * det3([&rows[1], &rows[2], &rows[3]])
        + lifted[1] * det3([&rows[0], &rows[2], &rows[3]])
        - lifted[2] * det3([&rows[0], &rows[1], &rows[3]])
        + lifted[3] * det3([&rows[0], &rows[1], &rows[2]])
}

/// Sign of [`naive_orient3d`].
#[must_use]
pub fn naive_orient3d_sign(p: &Orient3dInput) -> i8 {
    sign(naive_orient3d(p))
}

/// Sign of [`naive_insphere`].
#[must_use]
pub fn naive_insphere_sign(p: &InSphereInput) -> i8 {
    sign(naive_insphere(p))
}
