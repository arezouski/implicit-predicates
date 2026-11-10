//! Inputs shared by the integration tests: the versioned sample corpora,
//! generated point sets, and exact signs.

#![allow(dead_code)] // Each test file uses a different part of this module.

use indirect_predicates_corpus::exact;
use indirect_predicates_corpus::{Call, Corpus, Predicate};
use indirect_predicates_oracle::Dyadic;
use indirect_predicates_test_support::{leaf, sample};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// Five points: orient3d uses the first four, insphere all five.
pub type Five = [[f64; 3]; 5];

/// Every corpus of `corpus/samples/`, in file name order.
pub fn sample_corpora() -> Vec<Corpus> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../corpus/samples");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|x| x == "ipcorpus"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no sample corpus in {}", dir.display());
    files
        .iter()
        .map(|path| {
            let file = File::open(path).unwrap();
            Corpus::read(BufReader::new(file)).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        })
        .collect()
}

/// Exact sign of a recorded call, in the convention of `corpus::exact`.
///
/// # Panics
///
/// Panics if the call uses an LPI or TPI point.
pub fn exact_call_sign(corpus: &Corpus, call: &Call) -> i8 {
    let p: Vec<_> = call
        .points()
        .iter()
        .map(|&i| exact::coordinates(&corpus.points, i).expect("explicit or LNC point"))
        .collect();
    match call.predicate {
        Predicate::Orient3d => exact::orient3d(&p[0], &p[1], &p[2], &p[3]),
        Predicate::InSphere => exact::insphere(&p[0], &p[1], &p[2], &p[3], &p[4]),
    }
}

fn exact_point(p: &[f64; 3]) -> [Dyadic; 3] {
    p.map(Dyadic::from_f64)
}

/// Exact orient3d sign of the first four points.
pub fn exact_orient3d(p: &Five) -> i8 {
    let [a, b, c, d, _] = p.each_ref().map(exact_point);
    exact::orient3d(&a, &b, &c, &d)
}

/// Exact insphere sign of the five points.
pub fn exact_insphere(p: &Five) -> i8 {
    let [a, b, c, d, e] = p.each_ref().map(exact_point);
    exact::insphere(&a, &b, &c, &d, &e)
}

/// Splits a flat list of coordinates into point sets of five points.
fn fives(coordinates: Vec<f64>) -> Vec<Five> {
    coordinates
        .chunks_exact(15)
        .map(|c| std::array::from_fn(|i| [c[3 * i], c[3 * i + 1], c[3 * i + 2]]))
        .collect()
}

/// Coordinates with magnitudes in `2^-40 ..= 2^41` and full significands.
/// The reference libraries do not guard against overflow or underflow, so
/// coordinates stay far from both: every intermediate product of insphere
/// stays inside the normal range.
fn general(count: usize) -> Vec<Five> {
    fives(sample(leaf(-40, 40, 53), 15 * count))
}

/// Integers below `2^41` with at most 20 significant bits: sums of a few of
/// them are exact.
fn integers(count: usize) -> Vec<Five> {
    fives(sample(leaf(19, 40, 20), 15 * count))
}

/// The six permutations of three coordinates.
const PERMUTATIONS: [[usize; 3]; 6] = [
    [0, 1, 2],
    [0, 2, 1],
    [1, 0, 2],
    [1, 2, 0],
    [2, 0, 1],
    [2, 1, 0],
];

/// `count` point sets of each of five families: general position; the
/// fourth point equal to `b + c - a` (exactly coplanar); the first four
/// points sharing one coordinate (coplanar on an axis-parallel plane); five
/// signed permutations of one offset around one center (exactly
/// cospherical, with integers so that the translation is exact); and the
/// fourth point equal to the first.
pub fn generated(count: usize) -> Vec<Five> {
    let mut sets = general(count);
    sets.extend(integers(count).into_iter().map(|mut p| {
        p[3] = std::array::from_fn(|k| p[1][k] + p[2][k] - p[0][k]);
        p
    }));
    sets.extend(general(count).into_iter().enumerate().map(|(i, mut p)| {
        let axis = i % 3;
        for j in 1..4 {
            p[j][axis] = p[0][axis];
        }
        p
    }));
    sets.extend(integers(count).into_iter().enumerate().map(|(i, p)| {
        let (center, offset) = (p[0], p[1]);
        std::array::from_fn(|j| {
            let choice = 5 * i + j;
            let order = PERMUTATIONS[choice % 6];
            std::array::from_fn(|k| {
                let negate = (choice / 6) >> k & 1 == 1;
                let o = offset[order[k]];
                center[k] + if negate { -o } else { o }
            })
        })
    }));
    sets.extend(general(count).into_iter().map(|mut p| {
        p[3] = p[0];
        p
    }));
    sets
}
