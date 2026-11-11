//! Helpers shared by the integration tests.
#![allow(dead_code)]

use implicit_predicates_corpus::exact::{
    REFERENCE_INSPHERE_FACTOR, REFERENCE_ORIENT3D_FACTOR, coordinates, insphere, orient3d,
};
use implicit_predicates_corpus::{
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

/// Like [`corpus_from`], with the sign of every call replaced by the sign the
/// reference mesher records: the exact sign times the reference factor.
pub fn recorded(points: Vec<PointDef>, mut calls: Vec<Call>) -> Corpus {
    for c in &mut calls {
        let x: Vec<_> = c
            .points()
            .iter()
            .map(|&i| coordinates(&points, i).expect("explicit or LNC point"))
            .collect();
        c.sign = match c.predicate {
            Predicate::Orient3d => REFERENCE_ORIENT3D_FACTOR * orient3d(&x[0], &x[1], &x[2], &x[3]),
            Predicate::InSphere => {
                REFERENCE_INSPHERE_FACTOR * insphere(&x[0], &x[1], &x[2], &x[3], &x[4])
            }
        };
    }
    corpus_from(points, calls)
}

/// Calls of the explicit non-zero orient3d stratum that [`sampled`] adds to
/// the counters without storing them.
pub const UNSTORED: u64 = 1000;

/// A recorded corpus whose explicit non-zero orient3d stratum was sampled:
/// it stores 3 of its `3 + UNSTORED` calls. It also has orient3d calls with
/// one implicit point (zero and non-zero signs) and one insphere call; point
/// 6 is an LNC point and point 7 an explicit point that no call uses.
pub fn sampled() -> Corpus {
    let points = vec![
        PointDef::Explicit([1.0, 0.0, 0.0]),
        PointDef::Explicit([0.0, 1.0, 0.0]),
        PointDef::Explicit([0.0, 0.0, 1.0]),
        PointDef::Explicit([0.0, 0.0, 0.0]),
        PointDef::Explicit([9.0, 7.0, 5.0]),
        PointDef::Lnc { p: 0, q: 1, t: 0.5 },
        PointDef::Lnc {
            p: 2,
            q: 4,
            t: 0.25,
        },
        PointDef::Explicit([2.0, 3.0, 5.0]),
    ];
    let o = Predicate::Orient3d;
    let calls = vec![
        call(o, [0, 1, 2, 3, 0], 0, Stage::Filtered),
        call(o, [5, 1, 2, 3, 0], 0, Stage::Interval),
        call(o, [0, 1, 2, 4, 0], 0, Stage::Filtered),
        call(o, [0, 1, 5, 2, 0], 0, Stage::Exact),
        call(o, [5, 2, 1, 3, 0], 0, Stage::Interval),
        call(o, [1, 0, 2, 3, 0], 0, Stage::Filtered),
        call(Predicate::InSphere, [0, 1, 2, 3, 4], 0, Stage::Filtered),
    ];
    let mut c = recorded(points, calls);
    let key = CounterKey {
        predicate: o,
        implicit: 0,
        sign: c.calls[0].sign,
        stage: Stage::Filtered,
    };
    *c.counters.get_mut(&key).unwrap() += UNSTORED;
    let stratum = Stratum {
        predicate: o,
        implicit: 0,
        zero: false,
    };
    c.sampling.get_mut(&stratum).unwrap().total += UNSTORED;
    c.validate().unwrap();
    c
}
