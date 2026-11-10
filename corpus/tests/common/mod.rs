//! Helpers shared by the integration tests.
#![allow(dead_code)]

use indirect_predicates_corpus::{
    Call, Corpus, CounterKey, PointDef, Predicate, Sampling, Stage, Stratum,
};
use std::collections::BTreeMap;

/// Builds a call.
pub fn call(predicate: Predicate, points: [u32; 5], sign: i8, stage: Stage) -> Call {
    Call {
        predicate,
        points,
        sign,
        stage,
    }
}

/// Builds a corpus whose counters and sampling are consistent with `calls`
/// (every call is kept).
pub fn corpus_from(points: Vec<PointDef>, calls: Vec<Call>) -> Corpus {
    let mut counters = BTreeMap::new();
    let mut sampling: BTreeMap<Stratum, Sampling> = BTreeMap::new();
    for c in &calls {
        let implicit = c
            .points()
            .iter()
            .filter(|&&i| !matches!(points[i as usize], PointDef::Explicit(_)))
            .count() as u8;
        let key = CounterKey {
            predicate: c.predicate,
            implicit,
            sign: c.sign,
            stage: c.stage,
        };
        *counters.entry(key).or_insert(0) += 1;
        let stratum = Stratum {
            predicate: c.predicate,
            implicit,
            zero: c.sign == 0,
        };
        let s = sampling
            .entry(stratum)
            .or_insert(Sampling { total: 0, kept: 0 });
        s.total += 1;
        s.kept += 1;
    }
    Corpus {
        model: "m".to_string(),
        points,
        counters,
        sampling,
        calls,
    }
}
