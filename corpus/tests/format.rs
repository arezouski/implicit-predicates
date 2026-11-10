//! Tests of the corpus file format.

use indirect_predicates_corpus::{
    Call, Corpus, CorpusError, CounterKey, PointDef, Predicate, Sampling, Stage, Stratum,
};
use std::collections::BTreeMap;

fn call(predicate: Predicate, points: [u32; 5], sign: i8, stage: Stage) -> Call {
    Call {
        predicate,
        points,
        sign,
        stage,
    }
}

/// Builds a corpus whose counters and sampling are consistent with `calls`
/// (every call is kept).
fn corpus_from(points: Vec<PointDef>, calls: Vec<Call>) -> Corpus {
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

fn full_points() -> Vec<PointDef> {
    vec![
        PointDef::Explicit([-0.0, 1.5, 5e-324]),
        PointDef::Explicit([1.0, 0.0, -2.0]),
        PointDef::Explicit([0.0, 1.0, 3.0]),
        PointDef::Explicit([f64::MAX, f64::MIN_POSITIVE, -1.0]),
        PointDef::Lnc {
            p: 0,
            q: 1,
            t: 0.25,
        },
        PointDef::Lpi {
            line: [0, 1],
            plane: [1, 2, 3],
        },
        PointDef::Tpi {
            planes: [[0, 1, 2], [1, 2, 3], [0, 2, 3]],
        },
    ]
}

fn full_corpus() -> Corpus {
    use Predicate::{InSphere, Orient3d};
    let calls = vec![
        call(Orient3d, [0, 1, 2, 3, 0], 1, Stage::Unknown),
        call(Orient3d, [0, 1, 2, 4, 0], -1, Stage::Filtered),
        call(Orient3d, [0, 1, 5, 4, 0], 0, Stage::Interval),
        call(Orient3d, [0, 6, 5, 4, 0], 1, Stage::Bigfloat),
        call(InSphere, [0, 1, 2, 3, 4], -1, Stage::Exact),
        call(InSphere, [0, 1, 2, 5, 6], 0, Stage::Filtered),
        call(InSphere, [0, 1, 2, 3, 6], 1, Stage::Unknown),
    ];
    corpus_from(full_points(), calls)
}

fn to_bytes(c: &Corpus) -> Vec<u8> {
    let mut buf = Vec::new();
    c.write(&mut buf).unwrap();
    buf
}

fn point_bits(p: &PointDef) -> Vec<u64> {
    match p {
        PointDef::Explicit(c) => c.iter().map(|v| v.to_bits()).collect(),
        PointDef::Lnc { t, .. } => vec![t.to_bits()],
        _ => Vec::new(),
    }
}

/// Byte offset of the first point record for a corpus named "m".
const FIRST_POINT: usize = 8 + 4 + 4 + 1 + 32;

#[test]
fn round_trip_preserves_every_field() {
    let c = full_corpus();
    let back = Corpus::read(&to_bytes(&c)[..]).unwrap();
    assert_eq!(back, c);
    for (a, b) in back.points.iter().zip(&c.points) {
        assert_eq!(point_bits(a), point_bits(b));
    }
    let PointDef::Explicit(first) = back.points[0] else {
        panic!("first point is explicit");
    };
    assert!(first[0].is_sign_negative() && first[0] == 0.0);
    assert_eq!(first[2].to_bits(), 1);
    assert_eq!(back.calls[0].points, [0, 1, 2, 3, 0]);
}

#[test]
fn empty_corpus_round_trips() {
    let c = corpus_from(Vec::new(), Vec::new());
    assert_eq!(Corpus::read(&to_bytes(&c)[..]).unwrap(), c);
}

#[test]
fn every_truncation_is_an_error() {
    let bytes = to_bytes(&full_corpus());
    for len in 0..bytes.len() {
        assert!(
            Corpus::read(&bytes[..len]).is_err(),
            "prefix {len} accepted"
        );
    }
}

#[test]
fn trailing_bytes_are_an_error() {
    let mut bytes = to_bytes(&full_corpus());
    bytes.push(0);
    assert!(matches!(
        Corpus::read(&bytes[..]),
        Err(CorpusError::Format(_))
    ));
}

#[test]
fn huge_header_counts_do_not_allocate() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"IPCORPUS");
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&u64::MAX.to_le_bytes());
    bytes.extend_from_slice(&u64::MAX.to_le_bytes());
    bytes.extend_from_slice(&u64::MAX.to_le_bytes());
    bytes.extend_from_slice(&u64::MAX.to_le_bytes());
    assert!(Corpus::read(&bytes[..]).is_err());
    // A huge name length with no data must not allocate either.
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"IPCORPUS");
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&u32::MAX.to_le_bytes());
    assert!(Corpus::read(&bytes[..]).is_err());
}

#[test]
fn forward_parent_is_rejected() {
    let mut c = full_corpus();
    c.points[4] = PointDef::Lnc { p: 0, q: 5, t: 0.5 };
    assert!(matches!(
        Corpus::read(&to_bytes(&c)[..]),
        Err(CorpusError::Invalid(_))
    ));
    assert!(c.validate().is_err());
}

#[test]
fn non_finite_coordinate_is_rejected() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut c = full_corpus();
        c.points[1] = PointDef::Explicit([0.0, bad, 0.0]);
        assert!(Corpus::read(&to_bytes(&c)[..]).is_err());
        let mut c = full_corpus();
        c.points[4] = PointDef::Lnc { p: 0, q: 1, t: bad };
        assert!(Corpus::read(&to_bytes(&c)[..]).is_err());
    }
}

#[test]
fn lnc_with_implicit_parent_is_rejected() {
    let mut c = full_corpus();
    c.points.push(PointDef::Lnc { p: 4, q: 1, t: 0.5 });
    assert!(matches!(
        Corpus::read(&to_bytes(&c)[..]),
        Err(CorpusError::Invalid(_))
    ));
}

#[test]
fn bad_sign_is_rejected() {
    let mut c = full_corpus();
    c.calls[0].sign = 2;
    assert!(Corpus::read(&to_bytes(&c)[..]).is_err());
    let mut c = full_corpus();
    let key = *c.counters.keys().next().unwrap();
    let n = c.counters.remove(&key).unwrap();
    c.counters.insert(CounterKey { sign: -2, ..key }, n);
    assert!(Corpus::read(&to_bytes(&c)[..]).is_err());
}

#[test]
fn unknown_codes_are_rejected() {
    let good = to_bytes(&full_corpus());
    let patch = |offset: usize, value: u8| {
        let mut b = good.clone();
        b[offset] = value;
        Corpus::read(&b[..])
    };
    assert!(Corpus::read(&good[..]).is_ok());
    assert!(patch(8, 2).is_err(), "version");
    assert!(patch(FIRST_POINT, 4).is_err(), "point kind");
    // The last call is an in-sphere call: predicate, sign, stage, 5 indices.
    let last = good.len() - (3 + 4 * 5);
    assert!(patch(last, 2).is_err(), "predicate");
    assert!(patch(last + 2, 5).is_err(), "stage");
}

#[test]
fn sampling_must_match_calls() {
    let mut c = full_corpus();
    let s = c.sampling.values_mut().next().unwrap();
    s.kept += 1;
    s.total += 1;
    assert!(Corpus::read(&to_bytes(&c)[..]).is_err());
    let mut c = full_corpus();
    let s = c.sampling.values_mut().next().unwrap();
    s.total = 0;
    assert!(Corpus::read(&to_bytes(&c)[..]).is_err());
}

#[test]
fn counters_must_match_sampling() {
    let mut c = full_corpus();
    *c.counters.values_mut().next().unwrap() += 1;
    assert!(matches!(
        Corpus::read(&to_bytes(&c)[..]),
        Err(CorpusError::Invalid(_))
    ));
}
