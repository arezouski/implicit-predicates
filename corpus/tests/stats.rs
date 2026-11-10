//! Tests of the corpus statistics.

mod common;

use common::corpus_from;
use indirect_predicates_corpus::stats::Stats;
use indirect_predicates_corpus::{Corpus, CounterKey, Predicate, Sampling, Stage, Stratum};

fn key(implicit: u8, sign: i8, stage: Stage) -> CounterKey {
    CounterKey {
        predicate: Predicate::Orient3d,
        implicit,
        sign,
        stage,
    }
}

/// A corpus with no stored calls whose counters say: 1000 calls with no
/// implicit points (100 of them zero) and 10 calls with one implicit point.
fn counted() -> Corpus {
    let mut c = corpus_from(Vec::new(), Vec::new());
    for (k, n) in [
        (key(0, 1, Stage::Filtered), 800),
        (key(0, -1, Stage::Exact), 100),
        (key(0, 0, Stage::Exact), 100),
        (key(1, 1, Stage::Filtered), 10),
    ] {
        c.counters.insert(k, n);
    }
    for (implicit, zero, total) in [(0, false, 900), (0, true, 100), (1, false, 10)] {
        let s = Stratum {
            predicate: Predicate::Orient3d,
            implicit,
            zero,
        };
        c.sampling.insert(s, Sampling { total, kept: 0 });
    }
    c.validate().unwrap();
    c
}

#[test]
fn fractions_come_from_counters_not_from_kept_calls() {
    let stats = Stats::from_corpus(&counted());
    let f = stats.fraction_by_implicit(Predicate::Orient3d);
    assert_eq!(f[0].0, 0);
    assert_eq!(f[0].1, 1000);
    assert!((f[0].2 - 1000.0 / 1010.0).abs() < 1e-12);
    assert_eq!((f[1].0, f[1].1), (1, 10));
    let z = stats.zero_fraction_by_implicit(Predicate::Orient3d);
    assert_eq!((z[0].0, z[0].1), (0, 100));
    assert!((z[0].2 - 0.1).abs() < 1e-12);
}

#[test]
fn a_corpus_without_calls_reports_zeros() {
    let stats = Stats::from_corpus(&corpus_from(Vec::new(), Vec::new()));
    for p in [Predicate::Orient3d, Predicate::InSphere] {
        for (_, n, frac) in stats.fraction_by_implicit(p) {
            assert_eq!((n, frac), (0, 0.0));
        }
        for (_, n, frac) in stats.zero_fraction_by_implicit(p) {
            assert_eq!((n, frac), (0, 0.0));
        }
    }
    assert!(!stats.to_markdown().is_empty());
}

#[test]
fn a_missing_class_reports_zero() {
    let stats = Stats::from_corpus(&counted());
    let f = stats.fraction_by_implicit(Predicate::Orient3d);
    // Implicit counts 2, 3 and 4 never occur.
    assert_eq!(f.len(), 5);
    for &(_, n, frac) in &f[2..] {
        assert_eq!((n, frac), (0, 0.0));
    }
    let z = stats.zero_fraction_by_implicit(Predicate::Orient3d);
    assert_eq!((z[1].1, z[1].2), (0, 0.0));
    // InSphere has no counters at all.
    assert!(
        stats
            .fraction_by_implicit(Predicate::InSphere)
            .iter()
            .all(|&(_, n, _)| n == 0)
    );
}

#[test]
fn merge_adds_counters() {
    let mut a = Stats::from_corpus(&counted());
    a.merge(&Stats::from_corpus(&counted()));
    let f = a.fraction_by_implicit(Predicate::Orient3d);
    assert_eq!((f[0].1, f[1].1), (2000, 20));
    let z = a.zero_fraction_by_implicit(Predicate::Orient3d);
    assert_eq!(z[0].1, 200);
    assert!((z[0].2 - 0.1).abs() < 1e-12);
    assert!(a.to_markdown().contains("2000"));
}
