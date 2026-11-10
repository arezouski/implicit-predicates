//! Tests of the `ipcorpus` command-line tool.

mod common;

use common::{call, corpus_from};
use indirect_predicates_corpus::exact::REFERENCE_ORIENT3D_FACTOR;
use indirect_predicates_corpus::{Corpus, PointDef, Predicate, Stage};
use std::path::PathBuf;
use std::process::Command;

/// A temporary file removed on drop.
struct Temp(PathBuf);

impl Temp {
    fn new(name: &str) -> Temp {
        let unique = format!("ipcorpus-{}-{name}", std::process::id());
        Temp(std::env::temp_dir().join(unique))
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn run(args: &[&std::path::Path]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ipcorpus"))
        .args(args)
        .output()
        .unwrap()
}

fn sample(sign: i8) -> Corpus {
    let points = vec![
        PointDef::Explicit([1.0, 0.0, 0.0]),
        PointDef::Explicit([0.0, 1.0, 0.0]),
        PointDef::Explicit([0.0, 0.0, 1.0]),
        PointDef::Explicit([0.0, 0.0, 0.0]),
        PointDef::Explicit([9.0, 9.0, 9.0]),
        PointDef::Lnc { p: 0, q: 1, t: 0.5 },
    ];
    let calls = vec![
        call(Predicate::Orient3d, [0, 1, 2, 3, 0], sign, Stage::Exact),
        call(Predicate::Orient3d, [0, 1, 2, 4, 0], sign, Stage::Exact),
        call(Predicate::Orient3d, [5, 1, 2, 3, 0], sign, Stage::Exact),
    ];
    corpus_from(points, calls)
}

fn write(corpus: &Corpus, file: &Temp) {
    corpus
        .write(std::fs::File::create(&file.0).unwrap())
        .unwrap();
}

#[test]
fn verify_succeeds_on_a_correct_file_and_fails_on_an_altered_one() {
    // The exact sign of the first call is +1 for the unit tetrahedron, so the
    // recorded one is the orient3d factor; the other two calls are not part
    // of this check, so use a file of one call.
    let mut good = sample(REFERENCE_ORIENT3D_FACTOR);
    good.calls.truncate(1);
    let good = corpus_from(good.points, good.calls);
    let f = Temp::new("good");
    write(&good, &f);
    let out = run(&["verify".as_ref(), f.0.as_path()]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let mut bad = good.clone();
    bad.calls[0].sign = -REFERENCE_ORIENT3D_FACTOR;
    let bad = corpus_from(bad.points, bad.calls);
    let g = Temp::new("bad");
    write(&bad, &g);
    let out = run(&["verify".as_ref(), g.0.as_path()]);
    assert!(!out.status.success());
}

#[test]
fn stats_prints_a_table() {
    let f = Temp::new("stats");
    write(&sample(1), &f);
    let out = run(&["stats".as_ref(), f.0.as_path()]);
    assert!(out.status.success());
    assert!(String::from_utf8(out.stdout).unwrap().contains("orient3d"));
}

#[test]
fn extract_produces_a_file_the_reader_accepts() {
    let input = Temp::new("in");
    let output = Temp::new("out");
    write(&sample(1), &input);
    let out = run(&[
        "extract".as_ref(),
        "--max-calls".as_ref(),
        "1".as_ref(),
        input.0.as_path(),
        output.0.as_path(),
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let small = Corpus::read(std::fs::File::open(&output.0).unwrap()).unwrap();
    // The three calls fall into two strata (0 and 1 implicit points): one
    // call each is kept, and only the points they cite.
    assert_eq!(small.calls.len(), 2);
    assert_eq!(small.points.len(), 6 - 1);
    assert!(small.sampling.values().all(|s| s.total == s.kept));
}
