//! Tests of the recorded samples versioned in `corpus/samples/`.

use indirect_predicates_corpus::verify::verify;
use indirect_predicates_corpus::{Corpus, PointDef, Predicate};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

/// Upper bound on the total size of the folder, in bytes.
const MAX_TOTAL_BYTES: u64 = 1 << 20;

fn samples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("samples")
}

fn sample_files() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(samples_dir())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "ipcorpus"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no sample files");
    files
}

fn read(path: &Path) -> Corpus {
    let file = File::open(path).unwrap();
    Corpus::read(BufReader::new(file)).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Calls that cite an LPI or TPI point, which `verify` skips.
fn calls_with_lpi_or_tpi(corpus: &Corpus) -> u64 {
    let unverifiable = |&p: &u32| {
        matches!(
            corpus.points[p as usize],
            PointDef::Lpi { .. } | PointDef::Tpi { .. }
        )
    };
    corpus
        .calls
        .iter()
        .filter(|c| c.points().iter().any(unverifiable))
        .count() as u64
}

#[test]
fn every_sample_matches_the_exact_signs() {
    for path in sample_files() {
        let corpus = read(&path);
        let v = verify(&corpus);
        assert!(
            v.mismatches.is_empty(),
            "{}: {:?}",
            path.display(),
            v.mismatches
        );
        assert_eq!(
            v.skipped,
            calls_with_lpi_or_tpi(&corpus),
            "{}",
            path.display()
        );
        assert_eq!(
            v.checked + v.skipped,
            corpus.calls.len() as u64,
            "{}",
            path.display()
        );
    }
}

#[test]
fn the_samples_cover_both_predicates_implicit_points_and_zero_signs() {
    let corpora: Vec<Corpus> = sample_files().iter().map(|p| read(p)).collect();
    let calls = || {
        corpora
            .iter()
            .flat_map(|c| c.calls.iter().map(move |call| (c, call)))
    };
    for predicate in [Predicate::Orient3d, Predicate::InSphere] {
        assert!(
            calls().any(|(_, c)| c.predicate == predicate),
            "{predicate:?}"
        );
        assert!(
            calls().any(|(_, c)| c.predicate == predicate && c.sign == 0),
            "{predicate:?} with sign zero"
        );
        assert!(
            calls().any(|(corpus, c)| c.predicate == predicate
                && c.points()
                    .iter()
                    .any(|&p| !matches!(corpus.points[p as usize], PointDef::Explicit(_)))),
            "{predicate:?} with an implicit point"
        );
    }
}

#[test]
fn the_samples_folder_is_small() {
    let total: u64 = std::fs::read_dir(samples_dir())
        .unwrap()
        .map(|e| e.unwrap().metadata().unwrap().len())
        .sum();
    assert!(total < MAX_TOTAL_BYTES, "{total} bytes");
}
