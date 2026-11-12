//! Input sets of `orient3d` on points of any type: the recorded calls with
//! implicit points, the explicit sets as [`AnyPoint`], and the exact
//! coordinates of the recorded calls that the filter does not decide.
//!
//! The recorded calls are drawn from the sample corpora, or from the corpus
//! files of the directory named by the environment variable
//! [`CORPUS_DIR`] when it is set, by deterministic weighted resampling, as
//! the `corpus` sets of the parent module: each stored call weighs
//! `total / kept` of its stratum, so a set reflects the mix of calls of the
//! recording runs. The corpus points are mapped to the
//! points of `implicit-predicates` by the same source file the corpus tests
//! of that crate use, so the two mappings cannot drift apart.

use implicit_predicates::stages::orient3d as stages;
use implicit_predicates::{AnyPoint, ExactCoords, Explicit, Point, Scratch};
use implicit_predicates_corpus::exact::REFERENCE_ORIENT3D_FACTOR;
use implicit_predicates_corpus::weighted::weighted_sample;
use implicit_predicates_corpus::{Call, Corpus, Predicate};

use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use super::{InputSet, SET_SIZE, orient3d_sets, read_corpora, sample_corpora};

#[allow(dead_code)]
#[path = "../../../crates/implicit-predicates/tests/common/corpus_points.rs"]
mod corpus_points;

use corpus_points::{Mapped, call_points, implicit_count, points};

/// Four points of `orient3d`, of any type of `implicit-predicates`.
pub type ImplicitInput = [AnyPoint; 4];

/// Seed of the weighted resampling of [`implicit_sets`]: the set of index
/// `k` uses `IMPLICIT_SEED + k`.
pub const IMPLICIT_SEED: u64 = 0x696d_706c;

/// Seed of the weighted resampling of [`exact_sets`]: the set of calls with
/// `k` implicit points uses `EXACT_SEED + k`.
pub const EXACT_SEED: u64 = 0x6578_6163;

/// The environment variable naming a directory of corpus files to draw the
/// recorded calls of this module from, instead of the versioned samples.
pub const CORPUS_DIR: &str = "IMPLICIT_PREDICATES_CORPUS_DIR";

/// The largest number of implicit points of an `orient3d` call.
pub const MAX_IMPLICIT: usize = 4;

/// The name of the set of calls with `k` implicit points, or of the mix of
/// calls with one to four for `k == 0`.
#[must_use]
pub fn implicit_set_name(k: usize) -> &'static str {
    [
        "mix",
        "1_implicit",
        "2_implicit",
        "3_implicit",
        "4_implicit",
    ][k]
}

/// [`SET_SIZE`] recorded `orient3d` calls with `k` implicit points (one to
/// four for `k == 0`) whose points are all provided and for which `extra`
/// holds, drawn by weighted resampling with seed `seed`, each with its
/// recorded sign in the convention of `implicit_predicates_corpus::exact`.
fn draw(k: usize, seed: u64, extra: impl Fn(&ImplicitInput) -> bool) -> Vec<(ImplicitInput, i8)> {
    let (corpora, mapped) = recorded();
    let input = |c: usize, call: &Call| -> Option<ImplicitInput> {
        let count = implicit_count(&corpora[c], call);
        let wanted = if k == 0 { count > 0 } else { count == k };
        if call.predicate != Predicate::Orient3d || !wanted {
            return None;
        }
        call_points(&mapped[c], call).filter(|p| extra(p))
    };
    // `weighted_sample` passes each corpus by reference to the slice.
    let index: HashMap<*const Corpus, usize> = corpora
        .iter()
        .enumerate()
        .map(|(c, corpus)| (std::ptr::from_ref(corpus), c))
        .collect();
    let draws = weighted_sample(corpora, SET_SIZE, seed, |corpus, call| {
        input(index[&std::ptr::from_ref(corpus)], call).is_some()
    });
    assert_eq!(draws.len(), SET_SIZE, "no eligible call in the samples");
    draws
        .into_iter()
        .map(|(c, i)| {
            let call = &corpora[c].calls[i];
            let points = input(c, call).expect("an eligible call");
            (points, REFERENCE_ORIENT3D_FACTOR * call.sign)
        })
        .collect()
}

/// The corpora the recorded calls are drawn from, read once, with their
/// points mapped.
fn recorded() -> &'static (Vec<Corpus>, Vec<Mapped>) {
    static RECORDED: OnceLock<(Vec<Corpus>, Vec<Mapped>)> = OnceLock::new();
    RECORDED.get_or_init(|| {
        let corpora = match std::env::var_os(CORPUS_DIR) {
            Some(dir) => read_corpora(Path::new(&dir)),
            None => sample_corpora(),
        };
        let mapped = corpora.iter().map(points).collect();
        (corpora, mapped)
    })
}

/// The sets of recorded `orient3d` calls with implicit points: `mix`, the
/// calls with one to four implicit points, then one set for each number of
/// implicit points, in the order of [`implicit_set_name`].
#[must_use]
pub fn implicit_sets() -> Vec<InputSet<ImplicitInput>> {
    (0..=MAX_IMPLICIT)
        .map(|k| InputSet {
            name: implicit_set_name(k),
            cases: draw(k, IMPLICIT_SEED + k as u64, |_| true)
                .into_iter()
                .map(|(points, _)| points)
                .collect(),
        })
        .collect()
}

/// The recorded sign of every case of [`implicit_sets`], set by set, in the
/// convention of `implicit_predicates_corpus::exact`: what the tests compare
/// the sets with, so that a wrong mapping of the corpus points is caught.
#[must_use]
pub fn implicit_recorded_signs() -> Vec<Vec<i8>> {
    (0..=MAX_IMPLICIT)
        .map(|k| {
            draw(k, IMPLICIT_SEED + k as u64, |_| true)
                .into_iter()
                .map(|(_, sign)| sign)
                .collect()
        })
        .collect()
}

/// The `orient3d` sets of the parent module, `random`, `near_degenerate`,
/// `degenerate` and `corpus`, with every point an [`AnyPoint::Explicit`].
#[must_use]
pub fn any_explicit_sets() -> Vec<InputSet<ImplicitInput>> {
    orient3d_sets()
        .into_iter()
        .map(|set| InputSet {
            name: set.name,
            cases: set
                .cases
                .iter()
                .map(|case| {
                    case.map(|p| AnyPoint::Explicit(Explicit::new(p).expect("finite inputs")))
                })
                .collect(),
        })
        .collect()
}

/// Whether the filter of `orient3d` does not decide the call.
fn undecided_by_the_filter(p: &ImplicitInput) -> bool {
    let [a, b, c, d] = p;
    stages::filter_points(a, b, c, d).is_none()
}

/// The recorded calls with `k` implicit points that the filter does not
/// decide, as points and as their exact coordinates.
pub struct ExactSet {
    /// Name of the set, as [`implicit_set_name`] gives it.
    pub name: &'static str,
    /// The points of each call.
    pub points: Vec<ImplicitInput>,
    /// The exact coordinates of the points of each call, in `scratch`.
    pub coords: Vec<[ExactCoords; 4]>,
    /// The arena that holds every coordinate of `coords`, and has room for
    /// the stages to work after them.
    pub scratch: Scratch<'static>,
}

/// For `k` from one to four, the [`SET_SIZE`] recorded calls with `k`
/// implicit points that the filter does not decide, drawn by weighted
/// resampling with seed `EXACT_SEED + k`, and the exact coordinates of their
/// points.
///
/// # Panics
///
/// Panics if a point cannot compute its exact coordinates; no sample point
/// is out of the range of its exact arithmetic.
#[must_use]
pub fn exact_sets() -> Vec<ExactSet> {
    (1..=MAX_IMPLICIT)
        .map(|k| {
            let points: Vec<_> = draw(k, EXACT_SEED + k as u64, undecided_by_the_filter)
                .into_iter()
                .map(|(points, _)| points)
                .collect();
            let mut scratch = Scratch::growable(1 << 16);
            let coords = points
                .iter()
                .map(|p| {
                    p.each_ref()
                        .map(|q| q.exact(&mut scratch).expect("exact coordinates"))
                })
                .collect();
            ExactSet {
                name: implicit_set_name(k),
                points,
                coords,
                scratch,
            }
        })
        .collect()
}
