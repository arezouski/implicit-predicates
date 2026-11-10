//! Exact verification of the signs recorded in a corpus.

use crate::exact::{
    REFERENCE_INSPHERE_FACTOR, REFERENCE_ORIENT3D_FACTOR, coordinates, insphere, orient3d,
};
use crate::{Corpus, Predicate};

/// A call whose recorded sign disagrees with the exact one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mismatch {
    /// Index of the call in the corpus.
    pub call: usize,
    /// The recorded sign.
    pub recorded: i8,
    /// The exact sign, in the convention of [`crate::exact`].
    pub exact: i8,
}

/// Result of verifying a corpus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verification {
    /// Calls compared against the exact sign.
    pub checked: u64,
    /// Calls skipped because they use an LPI or TPI point.
    pub skipped: u64,
    /// The calls that disagree.
    pub mismatches: Vec<Mismatch>,
}

/// Recomputes the sign of every call with exact arithmetic and compares it
/// with the recorded one: a call matches when `recorded == factor * exact`,
/// with the factor of its predicate ([`REFERENCE_ORIENT3D_FACTOR`] or
/// [`REFERENCE_INSPHERE_FACTOR`]).
///
/// Calls that involve an LPI or TPI point are skipped. The corpus must be
/// valid (see [`Corpus::validate`]).
pub fn verify(corpus: &Corpus) -> Verification {
    verify_with(corpus, REFERENCE_ORIENT3D_FACTOR, REFERENCE_INSPHERE_FACTOR)
}

/// Like [`verify`], with the given factors of orient3d and insphere instead
/// of the reference ones.
pub fn verify_with(corpus: &Corpus, orient3d_factor: i8, insphere_factor: i8) -> Verification {
    let mut result = Verification {
        checked: 0,
        skipped: 0,
        mismatches: Vec::new(),
    };
    for (i, call) in corpus.calls.iter().enumerate() {
        let coords: Option<Vec<_>> = call
            .points()
            .iter()
            .map(|&p| coordinates(&corpus.points, p))
            .collect();
        let Some(c) = coords else {
            result.skipped += 1;
            continue;
        };
        let (exact, factor) = match call.predicate {
            Predicate::Orient3d => (orient3d(&c[0], &c[1], &c[2], &c[3]), orient3d_factor),
            Predicate::InSphere => (insphere(&c[0], &c[1], &c[2], &c[3], &c[4]), insphere_factor),
        };
        result.checked += 1;
        if call.sign != factor * exact {
            result.mismatches.push(Mismatch {
                call: i,
                recorded: call.sign,
                exact,
            });
        }
    }
    result
}
