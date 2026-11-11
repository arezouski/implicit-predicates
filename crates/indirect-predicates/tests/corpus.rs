//! `orient3d` on the calls recorded from a tetrahedral mesher: every stored
//! `orient3d` call whose points are all explicit must get the recorded sign.
//!
//! The recording program uses the opposite sign convention, so the recorded
//! sign is `REFERENCE_ORIENT3D_FACTOR` times ours. A call with a coordinate
//! outside the range where `orient3d` is documented to decide may get
//! `None`; such calls are counted and reported, and any other `None` is a
//! failure.
//!
//! The versioned samples are checked by default. The generated corpus is
//! large and not versioned; its tests are ignored, and are meant to be run
//! in release mode with `cargo test -p indirect-predicates --release --
//! --ignored --nocapture`, which also shows the counts they print.

mod common;

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use common::{CHAIN, Points, all_in_range, first_deciding};
use indirect_predicates::Sign;
use indirect_predicates::stages::orient3d as stages;
use indirect_predicates_corpus::exact::REFERENCE_ORIENT3D_FACTOR;
use indirect_predicates_corpus::weighted::weights;
use indirect_predicates_corpus::{Call, Corpus, PointDef, Predicate};

/// The directory of the versioned sample corpora.
fn samples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/samples")
}

/// The directory of the generated corpus, which must exist.
fn generated_dir() -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/data/corpus");
    assert!(
        dir.is_dir(),
        "no generated corpus in {}: build it with `python3 corpus/tools/corpus.py all`",
        dir.display()
    );
    dir
}

/// The `.ipcorpus` files of `dir`, in file name order.
fn corpus_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot list {}: {e}", dir.display()))
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| path.extension().is_some_and(|e| e == "ipcorpus"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no corpus file in {}", dir.display());
    files
}

fn read(path: &Path) -> Corpus {
    let file = File::open(path).unwrap_or_else(|e| panic!("cannot open {}: {e}", path.display()));
    Corpus::read(BufReader::new(file))
        .unwrap_or_else(|e| panic!("invalid corpus {}: {e:?}", path.display()))
}

/// The points of an `orient3d` call, if they are all explicit.
fn explicit_orient3d(corpus: &Corpus, call: &Call) -> Option<Points> {
    if call.predicate != Predicate::Orient3d {
        return None;
    }
    let mut points = [[0.0; 3]; 4];
    for (point, &index) in points.iter_mut().zip(call.points()) {
        match corpus.points[index as usize] {
            PointDef::Explicit(c) => *point = c,
            _ => return None,
        }
    }
    Some(points)
}

/// The coordinates of a call as bit patterns, for failure messages.
fn bits(p: &Points) -> [[u64; 3]; 4] {
    p.map(|q| q.map(f64::to_bits))
}

/// What a sweep over recorded calls found.
#[derive(Debug, Default)]
struct Sweep {
    files: usize,
    checked: usize,
    zero: usize,
    outside_range: usize,
    undecided: usize,
}

/// Checks every explicit `orient3d` call of the files in `dir` against its
/// recorded sign.
fn sweep(dir: &Path) -> Sweep {
    let mut found = Sweep::default();
    for path in corpus_files(dir) {
        let corpus = read(&path);
        found.files += 1;
        for (index, call) in corpus.calls.iter().enumerate() {
            let Some(p) = explicit_orient3d(&corpus, call) else {
                continue;
            };
            found.checked += 1;
            let in_range = all_in_range(&p, stages::UNTRANSLATED_RANGE);
            found.outside_range += usize::from(!in_range);
            let Some(sign) = common::orient3d(&p) else {
                assert!(
                    !in_range,
                    "{} call {index}: undecided inside the range, bits {:x?}",
                    corpus.model,
                    bits(&p)
                );
                found.undecided += 1;
                continue;
            };
            found.zero += usize::from(sign == Sign::Zero);
            assert_eq!(
                REFERENCE_ORIENT3D_FACTOR * i8::from(sign),
                call.sign,
                "{} call {index}: recorded sign differs, bits {:x?}",
                corpus.model,
                bits(&p)
            );
        }
    }
    found
}

#[test]
fn orient3d_matches_the_recorded_sign_on_the_samples() {
    let found = sweep(&samples_dir());
    println!("samples: {found:?}");
    assert!(
        found.checked > 0,
        "no explicit orient3d call in the samples"
    );
}

#[test]
fn the_first_stage_that_decides_gives_the_result_of_the_chain_on_the_samples() {
    for path in corpus_files(&samples_dir()) {
        let corpus = read(&path);
        for (index, call) in corpus.calls.iter().enumerate() {
            let Some(p) = explicit_orient3d(&corpus, call) else {
                continue;
            };
            assert_eq!(
                first_deciding(&p).map(|(_, sign)| sign),
                common::orient3d(&p),
                "{} call {index}: bits {:x?}",
                corpus.model,
                bits(&p)
            );
        }
    }
}

#[test]
#[ignore = "needs the generated corpus; run in release mode"]
fn orient3d_matches_the_recorded_sign_on_the_generated_corpus() {
    let found = sweep(&generated_dir());
    println!("generated corpus: {found:?}");
    assert!(found.checked > 0, "no explicit orient3d call in the corpus");
}

/// The weight of the explicit `orient3d` calls of the files in `dir` that
/// each stage of `CHAIN` decides first, and last of those no stage decides; each
/// stored call weighs as many calls of the recording run as it stands for.
fn stage_weights(dir: &Path) -> [f64; CHAIN.len() + 1] {
    let mut decided = [0.0; CHAIN.len() + 1];
    for path in corpus_files(dir) {
        let corpus = read(&path);
        for (call, weight) in corpus.calls.iter().zip(weights(&corpus)) {
            let Some(p) = explicit_orient3d(&corpus, call) else {
                continue;
            };
            let stage = first_deciding(&p).map_or(CHAIN.len(), |(i, _)| i);
            decided[stage] += weight;
        }
    }
    decided
}

fn print_stage_fractions(name: &str, decided: &[f64]) {
    let total: f64 = decided.iter().sum();
    println!("{name}: {total:.0} weighted explicit orient3d calls");
    let names = CHAIN.iter().map(|(name, _)| *name).chain(["none"]);
    for (stage, weight) in names.zip(decided) {
        println!("  {stage:<24} {:>12.6} %", 100.0 * weight / total);
    }
}

#[test]
#[ignore = "prints a table; needs the generated corpus; run in release mode"]
fn orient3d_stage_fractions_on_recorded_calls() {
    let samples = stage_weights(&samples_dir());
    print_stage_fractions("samples", &samples);
    let generated = stage_weights(&generated_dir());
    print_stage_fractions("generated corpus", &generated);
}
