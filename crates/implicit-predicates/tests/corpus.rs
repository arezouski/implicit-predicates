//! `orient3d` and `insphere` on the calls recorded from a tetrahedral
//! mesher: every stored call, with explicit points and with points on a
//! line, must get the recorded sign, and the chain must agree with the
//! first of its stages that decides, and with every stage that decides
//! alone.
//!
//! The recording program uses the opposite sign convention, so the recorded
//! sign is `REFERENCE_ORIENT3D_FACTOR` times ours for `orient3d`, and
//! `REFERENCE_INSPHERE_FACTOR` times ours for `insphere`. `insphere` has no
//! public function: its chain is `staged_in` of the hidden module. Calls
//! with a kind of point that this crate does not provide yet are counted
//! and reported. Every call is decided: a call that no stage on expansions
//! decides goes to the stage on big integers, and is reported with the
//! reason (see `common::corpus_sweep::Why`); on the samples there must be
//! none, for either predicate. On the generated corpus, the stage on big integers
//! alone must also give the recorded sign of every call.
//!
//! The versioned samples are checked by default. The generated corpus is
//! large and not versioned; its sweep is ignored, and is meant to be run in
//! release mode with `cargo test -p implicit-predicates --release --test
//! corpus -- --ignored --nocapture generated`, which also prints, for the models
//! without and with self-intersections and for each predicate, the weighted
//! fraction of the calls that each stage decides first, by number of
//! implicit points, and the number of calls and the time of the sweeps of
//! the stage on big integers.

// `orient3d` needs the `std` feature.
#![cfg(feature = "std")]

mod common;

use std::collections::BTreeSet;
use std::fs::File;
use std::io::BufReader;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use common::corpus_sweep::{BigAlone, CrossCheck, InSphereTally, PointCounts, Sweep, Tally, Why};
use implicit_predicates_corpus::exact::REFERENCE_INSPHERE_FACTOR;
use implicit_predicates_corpus::parallel::{default_jobs, map_ordered};
use implicit_predicates_corpus::{Corpus, Predicate};

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

/// The fewest `insphere` calls without and with implicit points that the
/// samples check: as many as they hold.
const SAMPLE_INSPHERE_CALLS: (usize, usize) = (1200, 3358);

/// Fails, after the report, if a call got a sign other than the recorded
/// one, the chain disagrees with the stages, or no stage on expansions
/// decided a call whose inputs are inside their documented ranges.
fn assert_no_mismatch(name: &str, tally: &impl Sweep) {
    let mismatches = tally.mismatches();
    assert!(
        mismatches.count == 0,
        "{name}: {} mismatches, the first: {}",
        mismatches.count,
        mismatches.described[0]
    );
}

/// The paths of the versioned samples.
fn samples() -> impl Iterator<Item = PathBuf> {
    SAMPLES
        .into_iter()
        .map(|name| root().join("corpus/samples").join(name))
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
    assert_merging_is_adding::<Tally>(&tally);
    assert_no_mismatch("samples", &tally);
    let mut alone = BigAlone::default();
    for name in SAMPLES {
        alone.add_corpus(&read(&root().join("corpus/samples").join(name)));
    }
    assert_big_alone("samples", &alone, total.0 + total.1);
    assert!(
        tally.big_integers.counts.is_empty(),
        "calls decided on big integers on the samples: {:?}",
        tally.big_integers.examples
    );
    assert!(
        total.0 >= SAMPLE_CALLS.0 && total.1 >= SAMPLE_CALLS.1,
        "the samples check {total:?} calls, fewer than {SAMPLE_CALLS:?}"
    );
}

#[test]
fn insphere_matches_the_recorded_sign_on_the_samples() {
    let mut tally = InSphereTally::default();
    let mut total = (0, 0);
    for path in samples() {
        let (explicit, implicit) = tally.add_corpus(&read(&path));
        assert!(
            explicit > 0,
            "{}: no explicit insphere call checked",
            path.display()
        );
        assert!(
            implicit > 0,
            "{}: no insphere call with implicit points checked",
            path.display()
        );
        total = (total.0 + explicit, total.1 + implicit);
    }
    println!("{}", tally.report("insphere: samples"));
    assert_merging_is_adding::<InSphereTally>(&tally);
    assert_no_mismatch("insphere: samples", &tally);
    let mut alone = BigAlone::default();
    for path in samples() {
        alone.add_insphere_corpus(&read(&path));
    }
    assert_big_alone("insphere: samples", &alone, total.0 + total.1);
    // The stage on big integers, the only exact stage of `insphere`, decides
    // exactly the calls that neither test of the filter does, all for that
    // reason. With a single reason this is close to what the sweep counts by
    // construction; it guards that no call is lost on the way.
    assert_eq!(
        tally
            .big_integers_explicit
            .counts
            .iter()
            .map(|&(why, n, _)| (why, n))
            .collect::<Vec<_>>(),
        [(Why::BigIntegersOnly, tally.explicit_past_filters.0)],
        "{:?}",
        tally.big_integers_explicit.examples
    );
    assert!(tally.explicit_past_filters.0 > 0);
    // `filter_axes` decides some of the explicit calls that `filter` does
    // not, and they are in its column.
    assert!(tally.explicit_past_filter.0 > tally.explicit_past_filters.0);
    assert!(tally.buckets[0].columns[1] > 0.0);
    let implicit = tally.implicit();
    let close = |x: f64, y: f64| (x - y).abs() <= 1e-9 * y.abs();
    let [(why, stored, weight)] = tally.big_integers.counts[..] else {
        panic!("{:?}", tally.big_integers.counts);
    };
    assert_eq!(why, Why::BigIntegersOnly);
    assert!(stored > 0);
    let [filter, axes, big] = implicit.columns[..] else {
        panic!("the columns of insphere are not the two tests of the filter and the big integers");
    };
    assert!(close(weight, big), "{weight} {big}");
    assert!(
        close(filter + axes + big, implicit.weight),
        "{filter} {axes} {big}"
    );
    assert!(
        total.0 >= SAMPLE_INSPHERE_CALLS.0 && total.1 >= SAMPLE_INSPHERE_CALLS.1,
        "the samples check {total:?} insphere calls, fewer than {SAMPLE_INSPHERE_CALLS:?}"
    );
}

/// Without the factor of the reference, every `insphere` call of the samples
/// whose recorded sign is not zero is a mismatch, and no other: the sweep
/// compares the sign of the chain with the recorded one through the factor.
#[test]
fn the_sweep_of_insphere_fails_without_the_factor_of_the_reference() {
    let mut tally = InSphereTally::new(1, CrossCheck::EveryStageAlone);
    let mut not_zero = 0;
    for path in samples() {
        let corpus = read(&path);
        tally.add_corpus(&corpus);
        not_zero += corpus
            .calls
            .iter()
            .filter(|c| c.predicate == Predicate::InSphere && c.sign != 0)
            .count();
    }
    assert!(
        not_zero > 0,
        "no insphere call with a nonzero recorded sign"
    );
    assert_eq!(tally.mismatches.count, not_zero);
}

/// The two ways of cross-checking the `insphere` calls give the same table
/// and the same number of mismatches on the samples, also without the
/// factor of the reference, where every call with a nonzero recorded sign
/// is one. Only the descriptions of the mismatches differ, and, in the
/// examples of the calls decided on big integers, what the stages gave
/// alone, which `ChainOnly` does not run: the examples, with their decision,
/// must be the same.
#[test]
fn both_cross_checks_of_insphere_give_the_same_table_on_the_samples() {
    for factor in [REFERENCE_INSPHERE_FACTOR, 1] {
        let [every, chain] =
            [CrossCheck::EveryStageAlone, CrossCheck::ChainOnly].map(|cross_check| {
                let mut tally = InSphereTally::new(factor, cross_check);
                for path in samples() {
                    tally.add_corpus(&read(&path));
                }
                tally
            });
        let table = |t: &InSphereTally| {
            let described = &t.mismatches.described;
            t.report("samples")
                .lines()
                .filter(|line| !described.iter().any(|m| line.trim() == m))
                .map(without_stages_alone)
                .collect::<Vec<_>>()
                .join("\n")
        };
        assert_eq!(table(&every), table(&chain));
        assert_eq!(every.counts(), chain.counts());
        assert_eq!(every.sums(), chain.sums());
        assert_eq!(every.mismatches.count, chain.mismatches.count);
        assert_eq!(
            every.mismatches.count > 0,
            factor != REFERENCE_INSPHERE_FACTOR
        );
    }
}

/// `line` without what the stages gave alone, `alone: ...` up to the
/// points, in a described call.
fn without_stages_alone(line: &str) -> String {
    match (line.find(" alone: "), line.find(", points ")) {
        (Some(start), Some(end)) if start < end => format!("{}{}", &line[..start], &line[end..]),
        _ => line.to_string(),
    }
}

/// Fails unless one tally per sample, merged in order, counts what
/// `expected` does: the tally of the samples added one after another.
fn assert_merging_is_adding<T: Sweep>(expected: &T) {
    let mut merged = T::default();
    for path in samples() {
        let mut of_file = T::default();
        of_file.add_corpus(&read(&path));
        merged.merge(&of_file);
    }
    assert_eq!(merged.counts(), expected.counts());
    // The weights are sums of `f64` in another order: equal up to rounding.
    let (a, b) = (merged.sums(), expected.sums());
    assert_eq!(a.len(), b.len());
    for (x, y) in a.into_iter().zip(b) {
        assert!((x - y).abs() <= 1e-9 * y.abs(), "{x} is not {y}");
    }
}

/// Fails if the stage on big integers alone gave a sign other than the
/// recorded one, skipped a call, or checked fewer than `least` calls.
fn assert_big_alone(name: &str, alone: &BigAlone, least: usize) {
    assert!(
        alone.mismatches.count == 0,
        "{name}: {} mismatches of the stage on big integers, the first: {}",
        alone.mismatches.count,
        alone.mismatches.described[0]
    );
    assert_eq!(alone.unsupported, 0, "{name}: calls skipped");
    assert!(
        alone.calls >= least,
        "{name}: {} calls checked, fewer than {least}",
        alone.calls
    );
}

/// The files of the generated corpus, in order.
fn generated_files() -> Vec<PathBuf> {
    let dir = root().join("corpus/data/corpus");
    assert!(
        dir.is_dir(),
        "no generated corpus in {}: build it with `python3 corpus/tools/corpus.py all`",
        dir.display()
    );
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot list {}: {e}", dir.display()))
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| path.extension().is_some_and(|e| e == "ipcorpus"))
        .collect();
    files.sort();
    files
}

/// `add` of a [`BigAlone`] on every file of the generated corpus, on
/// several threads; prints the number of files and calls, the mismatches
/// and the time under `name`.
fn big_alone_on_the_generated_corpus(name: &str, add: fn(&mut BigAlone, &Corpus)) -> BigAlone {
    let files = generated_files();
    let start = std::time::Instant::now();
    let mut alone = BigAlone::default();
    map_ordered(
        &files,
        default_jobs(),
        |path| {
            let mut of_file = BigAlone::default();
            add(&mut of_file, &read(path));
            of_file
        },
        |of_file| {
            alone.merge(&of_file);
            ControlFlow::<()>::Continue(())
        },
    );
    let seconds = start.elapsed().as_secs_f64();
    println!(
        "{name}: {} files, {} calls, {} mismatches, {seconds:.1} s",
        files.len(),
        alone.calls,
        alone.mismatches.count
    );
    alone
}

/// The stage on big integers alone, without the stages before it, on every
/// `orient3d` call of the generated corpus, in both populations: the only
/// way to run it on real calls in number, since the chain leaves it very
/// few.
#[test]
#[ignore = "needs the generated corpus; run in release mode"]
fn the_big_integers_alone_match_the_recorded_sign_on_the_generated_corpus() {
    let alone = big_alone_on_the_generated_corpus("big integers alone", BigAlone::add_corpus);
    // Every call of the two populations of the sweep of the chain.
    let least = ORIENT3D_NOSI.0 + ORIENT3D_NOSI.1 + ORIENT3D_SI.0 + ORIENT3D_SI.1;
    assert_big_alone("generated corpus", &alone, least);
}

/// [`the_big_integers_alone_match_the_recorded_sign_on_the_generated_corpus`]
/// for `insphere`.
#[test]
#[ignore = "needs the generated corpus; run in release mode"]
fn the_big_integers_alone_match_the_recorded_sign_of_insphere_on_the_generated_corpus() {
    let alone = big_alone_on_the_generated_corpus(
        "insphere: big integers alone",
        BigAlone::add_insphere_corpus,
    );
    let least = INSPHERE_NOSI.0 + INSPHERE_NOSI.1 + INSPHERE_SI.0 + INSPHERE_SI.1;
    assert_big_alone("insphere: generated corpus", &alone, least);
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

/// The `orient3d` calls without and with implicit points of the models
/// without self-intersections of the generated corpus: the fewest that a
/// sweep of them must check, so that it cannot pass if calls stop being
/// checked.
const ORIENT3D_NOSI: (usize, usize) = (3_163_737, 7_861_160);

/// [`ORIENT3D_NOSI`] for the models with self-intersections.
const ORIENT3D_SI: (usize, usize) = (2_738_941, 6_560_056);

/// [`ORIENT3D_NOSI`] for `insphere`.
const INSPHERE_NOSI: (usize, usize) = (2_646_414, 9_367_609);

/// [`ORIENT3D_SI`] for `insphere`.
const INSPHERE_SI: (usize, usize) = (2_048_538, 7_936_005);

/// The sweep of the generated corpus, both populations. `insphere` is
/// swept with [`CrossCheck::EveryStageAlone`]: every stage alone on every
/// call, including the stage on big integers, which the chain leaves few
/// calls; the full corpus, of which this is part, is swept with less.
#[test]
#[ignore = "needs the generated corpus; prints the tables; run in release mode"]
fn orient3d_and_insphere_match_the_recorded_sign_on_the_generated_corpus() {
    let dir = root().join("corpus/data/corpus");
    let in_dir: BTreeSet<_> = generated_files()
        .iter()
        .map(|path| path.file_name().expect("a file name").to_owned())
        .collect();
    // Each population with the fewest calls of each predicate that its
    // sweep must check.
    let populations = [
        (
            "models without self-intersections",
            "corpus/data/nosi.txt",
            ORIENT3D_NOSI,
            INSPHERE_NOSI,
        ),
        (
            "models with self-intersections",
            "corpus/data/si.txt",
            ORIENT3D_SI,
            INSPHERE_SI,
        ),
    ];
    // The two lists split the generated corpus.
    let mut seen = BTreeSet::new();
    for (_, list, ..) in populations {
        for path in listed(list) {
            let name = path.file_name().expect("a file name").to_owned();
            assert!(seen.insert(name), "{} listed twice", path.display());
        }
    }
    assert_eq!(
        seen,
        in_dir,
        "the lists do not split the generated corpus in {}",
        dir.display()
    );

    let mut swept = Vec::new();
    for (name, list, least, least_insphere) in populations {
        let files = listed(list);
        let start = std::time::Instant::now();
        let both = sweep(&files, CrossCheck::EveryStageAlone);
        println!("{} files", files.len());
        println!("{}", both.orient3d.report(name));
        println!("{}", both.insphere.report(&format!("insphere: {name}")));
        println!(
            "sweep of both predicates: {:.1} s",
            start.elapsed().as_secs_f64()
        );
        swept.push((name, both, least, least_insphere));
    }
    for (name, both, least, least_insphere) in &swept {
        assert_no_mismatch(name, &both.orient3d);
        assert_complete(name, &orient3d_completeness(both), *least);
        let insphere = format!("insphere: {name}");
        assert_no_mismatch(&insphere, &both.insphere);
        assert_complete(&insphere, &insphere_completeness(both), *least_insphere);
    }
}

/// What a sweep of files found, for each predicate, with the calls checked
/// without and with implicit points.
struct Swept {
    orient3d: Tally,
    orient3d_checked: (usize, usize),
    insphere: InSphereTally,
    insphere_checked: (usize, usize),
}

/// The tallies of the files: each file read once and swept for both
/// predicates, one file per thread, merged in the order of the files.
fn sweep(files: &[PathBuf], cross_check: CrossCheck) -> Swept {
    let mut swept = Swept {
        orient3d: Tally::default(),
        orient3d_checked: (0, 0),
        insphere: InSphereTally::new(REFERENCE_INSPHERE_FACTOR, cross_check),
        insphere_checked: (0, 0),
    };
    let add = |a: (usize, usize), b: (usize, usize)| (a.0 + b.0, a.1 + b.1);
    map_ordered(
        files,
        default_jobs(),
        |path| {
            let corpus = read(path);
            let mut orient3d = Tally::default();
            let orient3d_checked = orient3d.add_corpus(&corpus);
            let mut insphere = InSphereTally::new(REFERENCE_INSPHERE_FACTOR, cross_check);
            let insphere_checked = insphere.add_corpus(&corpus);
            Swept {
                orient3d,
                orient3d_checked,
                insphere,
                insphere_checked,
            }
        },
        |of_file| {
            swept.orient3d.merge(&of_file.orient3d);
            swept.orient3d_checked = add(swept.orient3d_checked, of_file.orient3d_checked);
            swept.insphere.merge(&of_file.insphere);
            swept.insphere_checked = add(swept.insphere_checked, of_file.insphere_checked);
            ControlFlow::<()>::Continue(())
        },
    );
    swept
}

/// The sweep of every model without self-intersections of the dataset, a
/// corpus of its own next to the generated sample. `insphere` is swept with
/// [`CrossCheck::ChainOnly`]: the chain against the recorded sign, with the
/// stage that decides first and its reason, but no stage alone, which would
/// make the sweep many times longer. Every stage alone, the stage on big
/// integers included, is checked on every call of the generated sample,
/// which is part of this corpus.
#[test]
#[ignore = "needs the full corpus; prints the tables; run in release mode"]
fn orient3d_and_insphere_match_the_recorded_sign_on_the_full_corpus() {
    let dir = root().join("corpus/data/full/corpus");
    assert!(
        dir.is_dir(),
        "no full corpus in {}: see corpus/tools/README.md",
        dir.display()
    );
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot list {}: {e}", dir.display()))
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| path.extension().is_some_and(|e| e == "ipcorpus"))
        .collect();
    files.sort();
    let name = "every model without self-intersections";
    let start = std::time::Instant::now();
    let both = sweep(&files, CrossCheck::ChainOnly);
    println!("{} files", files.len());
    println!("{}", both.orient3d.report(name));
    let insphere = format!("insphere: {name}");
    println!("{}", both.insphere.report(&insphere));
    println!(
        "sweep of both predicates: {:.1} s",
        start.elapsed().as_secs_f64()
    );
    assert_no_mismatch(name, &both.orient3d);
    assert_no_mismatch(&insphere, &both.insphere);
    // The sample of the generated corpus is part of this one.
    assert!(files.len() >= 117, "only {} files", files.len());
    assert_complete(name, &orient3d_completeness(&both), ORIENT3D_NOSI);
    assert_complete(&insphere, &insphere_completeness(&both), INSPHERE_NOSI);
}

/// What [`assert_complete`] checks of a sweep of one predicate.
struct Completeness {
    /// The stored calls checked, by number of implicit points.
    stored: Vec<usize>,
    /// The stored calls skipped, by number of implicit points.
    unsupported: Vec<usize>,
    points: PointCounts,
    /// The calls checked without and with implicit points.
    checked: (usize, usize),
}

fn orient3d_completeness(swept: &Swept) -> Completeness {
    let buckets = &swept.orient3d.buckets;
    Completeness {
        stored: buckets.iter().map(|b| b.stored).collect(),
        unsupported: buckets.iter().map(|b| b.unsupported).collect(),
        points: swept.orient3d.points,
        checked: swept.orient3d_checked,
    }
}

fn insphere_completeness(swept: &Swept) -> Completeness {
    let buckets = &swept.insphere.buckets;
    Completeness {
        stored: buckets.iter().map(|b| b.stored).collect(),
        unsupported: buckets.iter().map(|b| b.unsupported).collect(),
        points: swept.insphere.points,
        checked: swept.insphere_checked,
    }
}

/// Fails if a sweep of the generated corpus skipped a call or a point, or
/// checked fewer calls than `least`, without and with implicit points.
///
/// The generated corpus has no point of a kind this crate does not provide,
/// no point with an input that is not finite, and no point on a line with a
/// parameter outside `[0, 1]`. If a new corpus has any, support them, or
/// state here how many are allowed and why.
fn assert_complete(name: &str, swept: &Completeness, least: (usize, usize)) {
    let Completeness {
        stored,
        unsupported,
        points,
        checked,
    } = swept;
    assert!(
        unsupported.iter().all(|&n| n == 0) && points.unsupported == 0,
        "{name}: calls with points of an unsupported kind, by implicit count: {unsupported:?}"
    );
    assert_eq!(points.non_finite, 0, "{name}: points not finite");
    assert_eq!(
        points.unchecked, 0,
        "{name}: points on a line with t outside [0, 1]"
    );
    assert!(
        stored.iter().all(|&n| n > 0),
        "{name}: no call checked for some number of implicit points: {stored:?}"
    );
    assert!(
        checked.0 >= least.0 && checked.1 >= least.1,
        "{name}: {checked:?} calls checked, fewer than {least:?}"
    );
}

#[test]
fn a_failure_of_the_exact_coordinates_is_accepted_only_outside_their_range() {
    use common::corpus_sweep::{Why, why_exact_failed};
    use implicit_predicates::{AnyPoint, Explicit, Lnc};

    let (p, q) = ([1.0, 2.0, 3.0], [4.0, 6.0, 8.0]);
    // A parameter below the product domain, and a parent that makes a
    // difference overflow: outside the range.
    let tiny = 2f64.powi(-600);
    for point in [
        Lnc::new_unchecked(p, q, tiny),
        Lnc::new_unchecked([f64::MAX, 0.0, 0.0], [-f64::MAX, 0.0, 0.0], 0.5),
    ] {
        assert_eq!(why_exact_failed(&point.into()), Why::ExactOutOfRange);
    }
    let not_finite: AnyPoint = Explicit::new_unchecked([f64::NAN, 0.0, 0.0]).into();
    assert_eq!(why_exact_failed(&not_finite), Why::ExactOutOfRange);
    // Inside the range, a failure has no reason: the sweep counts it as a
    // mismatch.
    let inside: AnyPoint = Lnc::new_unchecked(p, q, 0.25).into();
    assert_eq!(why_exact_failed(&inside), Why::Unexplained);
    let explicit: AnyPoint = Explicit::new_unchecked(p).into();
    assert_eq!(why_exact_failed(&explicit), Why::Unexplained);
}
