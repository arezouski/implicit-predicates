//! The signs of both reference libraries, in our convention, against exact
//! signs.

mod common;

use common::{Five, exact_call_sign, exact_insphere, exact_orient3d, generated, sample_corpora};
use implicit_predicates_ffi_ref::{current, legacy};

/// Mismatches of one module on every call of the sample corpora, as
/// printable lines, and the number of calls compared.
macro_rules! sample_mismatches {
    ($module:ident) => {{
        let mut mismatches = Vec::new();
        let mut compared = 0;
        for corpus in sample_corpora() {
            let (points, ids) = $module::from_corpus(&corpus);
            for (index, call) in corpus.calls.iter().enumerate() {
                let p: Vec<_> = call
                    .points()
                    .iter()
                    .map(|&i| ids[i as usize].unwrap())
                    .collect();
                let got = match call.predicate {
                    implicit_predicates_corpus::Predicate::Orient3d => {
                        points.orient3d(p[0], p[1], p[2], p[3])
                    }
                    implicit_predicates_corpus::Predicate::InSphere => {
                        points.insphere(p[0], p[1], p[2], p[3], p[4])
                    }
                };
                let exact = exact_call_sign(&corpus, call);
                compared += 1;
                if got != exact {
                    let defs: Vec<_> = call
                        .points()
                        .iter()
                        .map(|&i| &corpus.points[i as usize])
                        .collect();
                    mismatches.push(format!(
                        "{} call {index} {:?}: got {got}, exact {exact}, points {defs:?}",
                        corpus.model, call.predicate
                    ));
                }
            }
        }
        (mismatches, compared)
    }};
}

/// Signs of one module on a point set: (orient3d, insphere).
macro_rules! signs_of {
    ($module:ident, $p:expr) => {{
        let mut points = $module::Points::new();
        let [a, b, c, d, e] = $p.map(|xyz| points.explicit(xyz));
        (points.orient3d(a, b, c, d), points.insphere(a, b, c, d, e))
    }};
}

fn assert_no_mismatch(mismatches: &[String], compared: usize) {
    assert!(compared > 8000, "only {compared} calls compared");
    assert!(
        mismatches.is_empty(),
        "{} of {compared} calls differ from the exact sign:\n{}",
        mismatches.len(),
        mismatches[..mismatches.len().min(10)].join("\n")
    );
}

#[test]
fn legacy_matches_exact_on_the_sample() {
    let (mismatches, compared) = sample_mismatches!(legacy);
    assert_no_mismatch(&mismatches, compared);
}

#[test]
fn current_matches_exact_on_the_sample() {
    let (mismatches, compared) = sample_mismatches!(current);
    assert_no_mismatch(&mismatches, compared);
}

#[test]
fn explicit_calls_match_exact_on_generated_inputs() {
    let sets = generated(400);
    assert_eq!(sets.len(), 2000);
    let mut zeros = (0, 0);
    for p in &sets {
        let exact = (exact_orient3d(p), exact_insphere(p));
        zeros.0 += usize::from(exact.0 == 0);
        zeros.1 += usize::from(exact.1 == 0);
        assert_eq!(signs_of!(legacy, *p), exact, "legacy on {p:?}");
        assert_eq!(signs_of!(current, *p), exact, "current on {p:?}");
    }
    // The coplanar and duplicate families make orient3d zero, the
    // cospherical family makes insphere zero.
    assert!(zeros.0 >= 1200, "only {} zero orient3d signs", zeros.0);
    assert!(zeros.1 >= 400, "only {} zero insphere signs", zeros.1);
}

/// For each point set `[a, b, c, d, e]`, the LNC points `(a, e, 0)` and
/// `(e, a, 1)` give the same signs as `a` itself.
macro_rules! assert_lnc_endpoints {
    ($module:ident, $p:expr) => {{
        let p: Five = $p;
        let mut points = $module::Points::new();
        let [a, b, c, d, e] = p.map(|xyz| points.explicit(xyz));
        let expected = (points.orient3d(a, b, c, d), points.insphere(a, b, c, d, e));
        assert_eq!(expected, (exact_orient3d(&p), exact_insphere(&p)), "{p:?}");
        for lnc in [points.lnc(a, e, 0.0), points.lnc(e, a, 1.0)] {
            let got = (
                points.orient3d(lnc, b, c, d),
                points.insphere(lnc, b, c, d, e),
            );
            assert_eq!(got, expected, "{} on {p:?}", stringify!($module));
        }
    }};
}

#[test]
fn lnc_endpoints_match_their_parents() {
    for p in generated(40) {
        assert_lnc_endpoints!(legacy, p);
        assert_lnc_endpoints!(current, p);
    }
}
