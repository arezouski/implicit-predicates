//! The public `insphere` of `implicit-predicates` against both reference
//! libraries on the `insphere` calls of the sample corpora, and against the
//! exact sign. The reference signs come back in our convention from
//! `implicit_predicates_ffi_ref`, so nothing here converts them.

mod common;

use common::{exact_call_sign, sample_corpora};
use implicit_predicates::{AnyPoint, Explicit, Lnc, insphere};
use implicit_predicates_corpus::{Corpus, PointDef, Predicate};
use implicit_predicates_ffi_ref::{current, legacy};

/// Most implicit points of a call that `current` is asked about: with 3 or
/// more the current reference library crashes the process in `insphere`, a
/// defect of that library, so those calls are compared against `legacy` and
/// the exact sign only.
const CURRENT_MAX_IMPLICIT: usize = 2;

/// Counts of one run over the sample corpora.
#[derive(Debug, Default)]
struct Counts {
    /// `insphere` calls compared with `legacy` and the exact sign.
    legacy: usize,
    /// `insphere` calls compared with `current`.
    current: usize,
    /// Calls with a sign of zero among those of `legacy`.
    zero: usize,
    /// Calls with 3 or more implicit points, left out of `current`.
    skipped_for_current: usize,
}

/// The sign a reference module gives to the `insphere` call, in our
/// convention.
macro_rules! reference_sign {
    ($points:expr, $ids:expr, $call:expr) => {{
        let id = |i: usize| $ids[$call.points()[i] as usize].unwrap();
        $points.insphere(id(0), id(1), id(2), id(3), id(4))
    }};
}

/// Our points for the table of a corpus; `None` for LPI and TPI points.
fn our_points(corpus: &Corpus) -> Vec<Option<AnyPoint>> {
    corpus
        .points
        .iter()
        .map(|def| match *def {
            PointDef::Explicit(xyz) => Some(Explicit::new(xyz).unwrap().into()),
            PointDef::Lnc { p, q, t } => {
                let parent = |i: u32| match corpus.points[i as usize] {
                    PointDef::Explicit(xyz) => xyz,
                    _ => panic!("LNC parents are explicit"),
                };
                // `Lnc::new(a, b, t)` is `b + t (a - b)`, the corpus point is
                // `p + t (q - p)`: the parents go in the other order.
                Some(Lnc::new(parent(q), parent(p), t).unwrap().into())
            }
            PointDef::Lpi { .. } | PointDef::Tpi { .. } => None,
        })
        .collect()
}

#[test]
fn public_insphere_matches_both_references_on_the_sample() {
    let mut counts = Counts::default();
    for corpus in sample_corpora() {
        let ours = our_points(&corpus);
        let (old, old_ids) = legacy::from_corpus(&corpus);
        let (new, new_ids) = current::from_corpus(&corpus);
        for (index, call) in corpus.calls.iter().enumerate() {
            if call.predicate != Predicate::InSphere {
                continue;
            }
            let at = |i: usize| call.points()[i] as usize;
            let implicit = call
                .points()
                .iter()
                .filter(|&&i| matches!(corpus.points[i as usize], PointDef::Lnc { .. }))
                .count();
            let p: [&AnyPoint; 5] = std::array::from_fn(|i| ours[at(i)].as_ref().unwrap());
            let got = i8::from(insphere(p[0], p[1], p[2], p[3], p[4]));
            let label = || format!("{} call {index}: {:?}", corpus.model, call.points());

            assert_eq!(got, exact_call_sign(&corpus, call), "exact, {}", label());
            let legacy_sign = reference_sign!(old, old_ids, call);
            assert_eq!(got, legacy_sign, "legacy, {}", label());
            counts.legacy += 1;
            counts.zero += usize::from(got == 0);

            if implicit <= CURRENT_MAX_IMPLICIT {
                let current_sign = reference_sign!(new, new_ids, call);
                assert_eq!(got, current_sign, "current, {}", label());
                counts.current += 1;
            } else {
                counts.skipped_for_current += 1;
            }
        }
    }
    println!("{counts:?}");
    assert!(counts.legacy > 1000, "{counts:?}");
    assert!(counts.current > 1000, "{counts:?}");
    assert!(counts.zero > 0, "{counts:?}");
}
