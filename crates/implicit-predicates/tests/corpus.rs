//! `orient3d` on the calls recorded from a tetrahedral mesher: every stored
//! `orient3d` call, with explicit points and with points on a line, must
//! get the recorded sign, and the public functions must agree with the
//! first stage of the chain that decides.
//!
//! The recording program uses the opposite sign convention, so the recorded
//! sign is `REFERENCE_ORIENT3D_FACTOR` times ours. Calls with a kind of
//! point that this crate does not provide yet are counted and reported.
//! A call that no stage decides is reported with the documented reason
//! (see `common::corpus_sweep::Why`); on the samples there must be none.
//!
//! The versioned samples are checked by default. The generated corpus is
//! large and not versioned; its sweep is ignored, and is meant to be run in
//! release mode with `cargo test -p implicit-predicates --release --test
//! corpus -- --ignored --nocapture`, which also prints, for the models
//! without and with self-intersections, the weighted fraction of the calls
//! that each stage decides first, by number of implicit points.

// `orient3d` needs the `std` feature.
#![cfg(feature = "std")]

mod common;

use std::collections::BTreeSet;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use common::corpus_sweep::Tally;
use implicit_predicates_corpus::Corpus;

/// The root of the workspace.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The versioned sample corpora, each of which must exist.
const SAMPLES: [&str; 3] = [
    "thingi10k-1716275.ipcorpus",
    "thingi10k-217026.ipcorpus",
    "thingi10k-97167.ipcorpus",
];

/// The fewest calls without and with implicit points that the samples
/// check: as many as they hold, so that the test cannot pass on an empty or
/// truncated sample.
const SAMPLE_CALLS: (usize, usize) = (1200, 3177);

fn read(path: &Path) -> Corpus {
    let file = File::open(path).unwrap_or_else(|e| panic!("cannot open {}: {e}", path.display()));
    Corpus::read(BufReader::new(file))
        .unwrap_or_else(|e| panic!("invalid corpus {}: {e:?}", path.display()))
}

/// Fails, after the report, if a call got a sign other than the recorded
/// one or the public functions disagree with the stages.
fn assert_no_mismatch(name: &str, tally: &Tally) {
    assert!(
        tally.mismatch_count == 0,
        "{name}: {} mismatches, the first: {}",
        tally.mismatch_count,
        tally.mismatches[0]
    );
}

#[test]
fn orient3d_matches_the_recorded_sign_on_the_samples() {
    let mut tally = Tally::default();
    let mut total = (0, 0);
    for name in SAMPLES {
        let (explicit, implicit) =
            tally.add_corpus(&read(&root().join("corpus/samples").join(name)));
        assert!(explicit > 0, "{name}: no explicit orient3d call checked");
        assert!(
            implicit > 0,
            "{name}: no orient3d call with implicit points checked"
        );
        total = (total.0 + explicit, total.1 + implicit);
    }
    println!("{}", tally.report("samples"));
    assert_no_mismatch("samples", &tally);
    assert!(
        tally.undecided.is_empty(),
        "undecided calls on the samples: {:?}",
        tally.examples
    );
    assert!(
        total.0 >= SAMPLE_CALLS.0 && total.1 >= SAMPLE_CALLS.1,
        "the samples check {total:?} calls, fewer than {SAMPLE_CALLS:?}"
    );
}

/// The files listed in `list`, relative to the root of the workspace.
fn listed(list: &str) -> Vec<PathBuf> {
    let path = root().join(list);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| root().join(line))
        .collect()
}

#[test]
#[ignore = "needs the generated corpus; prints a table; run in release mode"]
fn orient3d_matches_the_recorded_sign_on_the_generated_corpus() {
    let dir = root().join("corpus/data/corpus");
    assert!(
        dir.is_dir(),
        "no generated corpus in {}: build it with `python3 corpus/tools/corpus.py all`",
        dir.display()
    );
    let populations = [
        ("models without self-intersections", "corpus/data/nosi.txt"),
        ("models with self-intersections", "corpus/data/si.txt"),
    ];
    // The two lists split the generated corpus.
    let in_dir: BTreeSet<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot list {}: {e}", dir.display()))
        .map(|entry| entry.expect("a directory entry").file_name())
        .filter(|name| Path::new(name).extension().is_some_and(|e| e == "ipcorpus"))
        .collect();
    let mut seen = BTreeSet::new();
    for (_, list) in populations {
        for path in listed(list) {
            let name = path.file_name().expect("a file name").to_owned();
            assert!(seen.insert(name), "{} listed twice", path.display());
        }
    }
    assert_eq!(seen, in_dir, "the lists do not split the generated corpus");

    let mut tallies = Vec::new();
    for (name, list) in populations {
        let mut tally = Tally::default();
        let files = listed(list);
        for path in &files {
            tally.add_corpus(&read(path));
        }
        println!("{} files", files.len());
        println!("{}", tally.report(name));
        tallies.push((name, tally));
    }
    for (name, tally) in &tallies {
        assert_no_mismatch(name, tally);
    }
}
