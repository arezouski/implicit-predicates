//! Tests of the weighted resampling of stored calls.

mod common;

use common::{UNSTORED, call, corpus_from, sampled};
use implicit_predicates_corpus::weighted::{weighted_sample, weights};
use implicit_predicates_corpus::{Corpus, PointDef, Predicate, Stage, Stratum};

/// A corpus of `stored` explicit orient3d calls with sign `sign`, recorded
/// from a run that made `total` such calls.
fn stratum_of(sign: i8, stored: usize, total: u64) -> Corpus {
    let points = (0..4)
        .map(|i| PointDef::Explicit([f64::from(i), 0.0, 0.0]))
        .collect();
    let calls = (0..stored)
        .map(|_| call(Predicate::Orient3d, [0, 1, 2, 3, 0], sign, Stage::Exact))
        .collect();
    let mut c = corpus_from(points, calls);
    let stratum = Stratum {
        predicate: Predicate::Orient3d,
        implicit: 0,
        zero: sign == 0,
    };
    *c.counters.values_mut().next().unwrap() = total;
    c.sampling.get_mut(&stratum).unwrap().total = total;
    c.validate().unwrap();
    c
}

#[test]
fn a_stored_call_weighs_the_calls_it_stands_for() {
    let c = sampled();
    let w = weights(&c);
    let heavy = (3 + UNSTORED) as f64 / 3.0;
    assert_eq!(w, vec![heavy, 1.0, heavy, 1.0, 1.0, heavy, 1.0]);
}

#[test]
fn a_heavily_sampled_stratum_is_drawn_in_proportion_to_its_total() {
    // Ten stored calls each: the first stands for 100,000 calls, the second
    // for ten.
    let corpora = [stratum_of(1, 10, 100_000), stratum_of(-1, 10, 10)];
    let draws = weighted_sample(&corpora, 100_000, 7, |_, _| true);
    assert_eq!(draws.len(), 100_000);
    let light = draws.iter().filter(|&&(c, _)| c == 1).count();
    // Expected 10 of 100,000; an unweighted draw would give about half.
    assert!(light < 40, "{light}");
    assert!(draws.iter().all(|&(c, i)| c < 2 && i < 10));
}

#[test]
fn strata_of_equal_weight_are_drawn_evenly() {
    let corpora = [stratum_of(1, 10, 1000), stratum_of(-1, 20, 2000)];
    let draws = weighted_sample(&corpora, 30_000, 1, |_, _| true);
    let second = draws.iter().filter(|&&(c, _)| c == 1).count() as f64;
    assert!((second / 30_000.0 - 2.0 / 3.0).abs() < 0.02, "{second}");
}

#[test]
fn only_eligible_calls_are_drawn() {
    let c = [sampled()];
    let draws = weighted_sample(&c, 1000, 3, |_, call| call.predicate == Predicate::InSphere);
    assert!(draws.iter().all(|&d| d == (0, 6)));
    assert!(weighted_sample(&c, 10, 3, |_, _| false).is_empty());
}

#[test]
fn the_draw_is_fixed_by_the_seed() {
    let c = [sampled()];
    let a = weighted_sample(&c, 500, 11, |_, _| true);
    assert_eq!(a, weighted_sample(&c, 500, 11, |_, _| true));
    assert_ne!(a, weighted_sample(&c, 500, 12, |_, _| true));
}
