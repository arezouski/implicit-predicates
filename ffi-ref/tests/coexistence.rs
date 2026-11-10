//! Both reference libraries loaded in one process, and the lifetime of the
//! points they build.

mod common;

use common::{generated, sample_corpora};
use indirect_predicates_corpus::Predicate;
use indirect_predicates_ffi_ref::{current, legacy};
use std::hint::black_box;
use std::sync::Mutex;

/// Serializes the tests of this file, so that the memory test measures
/// only its own allocations.
static SERIAL: Mutex<()> = Mutex::new(());

/// Runs every call of the sample corpora through both versions, on this
/// thread, asserting that they agree; returns the number of calls.
fn sample_calls_agree() -> usize {
    let mut compared = 0;
    for corpus in sample_corpora() {
        let (old, old_ids) = legacy::from_corpus(&corpus);
        let (new, new_ids) = current::from_corpus(&corpus);
        for call in &corpus.calls {
            let o: Vec<_> = call
                .points()
                .iter()
                .map(|&i| old_ids[i as usize].unwrap())
                .collect();
            let n: Vec<_> = call
                .points()
                .iter()
                .map(|&i| new_ids[i as usize].unwrap())
                .collect();
            let (a, b) = match call.predicate {
                Predicate::Orient3d => (
                    old.orient3d(o[0], o[1], o[2], o[3]),
                    new.orient3d(n[0], n[1], n[2], n[3]),
                ),
                Predicate::InSphere => (
                    old.insphere(o[0], o[1], o[2], o[3], o[4]),
                    new.insphere(n[0], n[1], n[2], n[3], n[4]),
                ),
            };
            assert_eq!(a, b, "{}: {call:?}", corpus.model);
            compared += 1;
        }
    }
    compared
}

#[test]
fn both_versions_answer_in_one_process() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let mut compared = sample_calls_agree();
    for p in generated(100) {
        let mut old = legacy::Points::new();
        let mut new = current::Points::new();
        let [a, b, c, d, e] = p.map(|xyz| (old.explicit(xyz), new.explicit(xyz)));
        assert_eq!(
            old.orient3d(a.0, b.0, c.0, d.0),
            new.orient3d(a.1, b.1, c.1, d.1),
            "{p:?}"
        );
        assert_eq!(
            old.insphere(a.0, b.0, c.0, d.0, e.0),
            new.insphere(a.1, b.1, c.1, d.1, e.1),
            "{p:?}"
        );
        compared += 2;
    }
    assert!(compared > 9000, "only {compared} calls compared");
}

#[test]
fn rounding_is_to_nearest_after_the_reference_calls() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    assert!(sample_calls_agree() > 8000);
    // 2^-60 is far below half an ulp of 1: rounding to nearest drops it,
    // rounding upward, downward or toward zero does not in one of these.
    let tiny = black_box(2f64.powi(-60));
    assert_eq!(black_box(1.0) + tiny, 1.0);
    assert_eq!(black_box(1.0) - tiny, 1.0);
    assert_eq!(black_box(-1.0) + tiny, -1.0);
    assert_eq!(black_box(-1.0) - tiny, -1.0);
}

/// Resident set size of this process, in bytes (Linux only).
fn resident_bytes() -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    let line = status.lines().find(|l| l.starts_with("VmRSS:")).unwrap();
    let kib: u64 = line.split_whitespace().nth(1).unwrap().parse().unwrap();
    kib * 1024
}

/// Points created per round; half are explicit and half LNC.
const ROUND: usize = 10_000;
/// Rounds per version: one million points each.
const ROUNDS: usize = 100;
/// Allowed growth of the resident set over all rounds. Leaking the points of
/// one version would take tens of megabytes (an LNC point alone holds four
/// intervals), far above this margin.
const MARGIN: u64 = 8 << 20;

macro_rules! churn {
    ($module:ident) => {{
        let mut points = $module::Points::new();
        let mut previous = points.explicit([0.0, 0.0, 0.0]);
        for i in 1..ROUND / 2 {
            let x = i as f64;
            let p = points.explicit([x, x * 0.5, 1.0 - x]);
            let q = points.lnc(previous, p, 0.25);
            // q lies on the line through previous and p.
            assert_eq!(points.orient3d(previous, p, q, q), 0);
            previous = p;
        }
        drop(points);
    }};
}

#[test]
fn dropping_points_releases_them() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    // A first round of each brings the allocators and pools to their steady
    // state.
    churn!(legacy);
    churn!(current);
    let before = resident_bytes();
    for _ in 0..ROUNDS {
        churn!(legacy);
        churn!(current);
    }
    let growth = resident_bytes().saturating_sub(before);
    assert!(growth < MARGIN, "resident set grew by {growth} bytes");
}
