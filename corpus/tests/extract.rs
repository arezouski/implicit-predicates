//! Tests of the extraction of a small corpus out of a large one.

mod common;

use common::{UNSTORED, sampled};
use implicit_predicates_corpus::exact::coordinates;
use implicit_predicates_corpus::verify::verify;
use implicit_predicates_corpus::{Corpus, PointDef, Predicate, Stratum};

fn stratum(predicate: Predicate, implicit: u8, zero: bool) -> Stratum {
    Stratum {
        predicate,
        implicit,
        zero,
    }
}

/// The exact coordinates of the points of every call.
fn call_coordinates(c: &Corpus) -> Vec<Vec<[implicit_predicates_oracle::Dyadic; 3]>> {
    c.calls
        .iter()
        .map(|call| {
            call.points()
                .iter()
                .map(|&i| coordinates(&c.points, i).unwrap())
                .collect()
        })
        .collect()
}

#[test]
fn extract_keeps_the_first_calls_of_each_stratum_in_order() {
    let full = sampled();
    let small = full.extract(1);
    // One call per stratum: the first of each, in their original order.
    let expected: Vec<_> = [0, 1, 3, 6].iter().map(|&i| full.calls[i]).collect();
    assert_eq!(small.calls.len(), expected.len());
    for (kept, original) in small.calls.iter().zip(&expected) {
        assert_eq!(
            (kept.predicate, kept.sign, kept.stage),
            (original.predicate, original.sign, original.stage)
        );
    }
    let all = full.extract(usize::MAX);
    assert_eq!(all.calls, full.calls[..].to_vec());
}

#[test]
fn extract_keeps_only_the_cited_points_and_remaps_the_calls() {
    let full = sampled();
    let small = full.extract(1);
    // No kept call cites the LNC point 6 or the explicit point 7; every
    // other point is cited, or is a parent of the cited LNC point 5.
    assert_eq!(small.points.len(), full.points.len() - 2);
    assert!(small.points.iter().all(|p| !matches!(
        p,
        PointDef::Explicit([2.0, 3.0, 5.0]) | PointDef::Lnc { t: 0.25, .. }
    )));
    let originals: Vec<_> = [0, 1, 3, 6]
        .iter()
        .map(|&i| call_coordinates(&full)[i].clone())
        .collect();
    assert_eq!(call_coordinates(&small), originals);
}

#[test]
fn extract_keeps_the_counters_and_totals_of_the_recording_run() {
    let full = sampled();
    let small = full.extract(1);
    assert_eq!(small.counters, full.counters);
    assert_eq!(
        small.sampling.keys().collect::<Vec<_>>(),
        full.sampling.keys().collect::<Vec<_>>()
    );
    for (s, sampling) in &small.sampling {
        assert_eq!(sampling.total, full.sampling[s].total, "{s:?}");
        assert_eq!(sampling.kept, full.sampling[s].kept.min(1), "{s:?}");
    }
    let explicit = stratum(Predicate::Orient3d, 0, false);
    assert_eq!(small.sampling[&explicit].total, 3 + UNSTORED);
    assert_eq!(small.sampling[&explicit].kept, 1);
}

#[test]
fn the_extracted_corpus_validates_and_verifies() {
    let full = sampled();
    for n in [0, 1, 2, usize::MAX] {
        let small = full.extract(n);
        small.validate().unwrap();
        let mut bytes = Vec::new();
        small.write(&mut bytes).unwrap();
        assert_eq!(Corpus::read(&bytes[..]).unwrap(), small);
        let v = verify(&small);
        assert!(v.mismatches.is_empty(), "{n}: {:?}", v.mismatches);
        assert_eq!((v.checked, v.skipped), (small.calls.len() as u64, 0));
    }
}
