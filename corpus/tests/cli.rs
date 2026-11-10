//! Tests of the `ipcorpus` command-line tool.

mod common;

use common::{call, corpus_from, sampled};
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

fn write(corpus: &Corpus, file: &Temp) {
    corpus
        .write(std::fs::File::create(&file.0).unwrap())
        .unwrap();
}

fn verify_file(extra: &[&str], file: &Temp) -> std::process::Output {
    let mut args: Vec<&std::path::Path> = vec!["verify".as_ref()];
    args.extend(extra.iter().map(std::path::Path::new));
    args.push(file.0.as_path());
    run(&args)
}

/// A corpus with one call on an LPI point, which `verify` cannot check.
fn with_lpi() -> Corpus {
    let mut points: Vec<PointDef> = (0..5)
        .map(|i| PointDef::Explicit([f64::from(i), f64::from(i * i), 1.0]))
        .collect();
    points.push(PointDef::Lpi {
        line: [0, 1],
        plane: [2, 3, 4],
    });
    let calls = vec![call(Predicate::Orient3d, [5, 1, 2, 3, 0], 1, Stage::Exact)];
    corpus_from(points, calls)
}

#[test]
fn verify_succeeds_on_a_correct_file_and_fails_on_an_altered_one() {
    let good = sampled();
    let f = Temp::new("good");
    write(&good, &f);
    let out = verify_file(&[], &f);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );

    let mut bad = good.clone();
    bad.calls[0].sign = -bad.calls[0].sign;
    let g = Temp::new("bad");
    write(&bad, &g);
    let out = verify_file(&[], &g);
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn verify_fails_on_skipped_calls_unless_they_are_allowed() {
    let f = Temp::new("lpi");
    write(&with_lpi(), &f);
    let out = verify_file(&[], &f);
    assert_eq!(out.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&out.stdout).contains("skipped 1"));
    let out = verify_file(&["--allow-skipped"], &f);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn stats_prints_a_table() {
    let f = Temp::new("stats");
    write(&sampled(), &f);
    let out = run(&["stats".as_ref(), f.0.as_path()]);
    assert!(out.status.success());
    assert!(String::from_utf8(out.stdout).unwrap().contains("orient3d"));
}

#[test]
fn extract_produces_a_file_that_validates_and_verifies() {
    let input = Temp::new("in");
    let output = Temp::new("out");
    let full = sampled();
    write(&full, &input);
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
    // The calls fall into four strata: one call each is kept, and only the
    // points they cite. The totals are those of the full recording.
    assert_eq!(small.calls.len(), 4);
    assert_eq!(small.points.len(), full.points.len() - 2);
    assert_eq!(small.counters, full.counters);
    for (s, sampling) in &small.sampling {
        assert_eq!(sampling.total, full.sampling[s].total);
    }
    let out = verify_file(&[], &output);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
}

#[test]
fn stats_per_file_prints_one_row_per_file() {
    let f = Temp::new("per-file-a");
    let g = Temp::new("per-file-b");
    write(&sampled(), &f);
    write(&with_lpi(), &g);
    let out = run(&[
        "stats".as_ref(),
        "--per-file".as_ref(),
        f.0.as_path(),
        g.0.as_path(),
    ]);
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 3, "{text}");
    assert!(lines[0].starts_with("file,model,orient3d_0,orient3d_0_zero,"));
    let first: Vec<&str> = lines[1].split(',').collect();
    assert_eq!(first[0], f.0.to_str().unwrap());
    // The sampled corpus: 1003 explicit orient3d calls, none of them zero.
    assert_eq!(&first[1..4], ["m", "1003", "0"]);
    assert_eq!(first.len(), lines[0].split(',').count());
}
