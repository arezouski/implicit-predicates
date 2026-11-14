//! Input sets of `orient3d` and `insphere` on points of any type: the
//! recorded calls with implicit points, the explicit sets as [`AnyPoint`],
//! and the recorded calls that the filter does not decide, with their exact
//! coordinates for `orient3d`. For `insphere`, those are the calls that the
//! first test of its filter does not decide, in one group for each number
//! of implicit points: the second test, `filter_axes`, decides some of them,
//! and `insphere` has no stage on expansions, so the others go to the stage
//! on big integers.
//!
//! The recorded calls are drawn from the sample corpora, or from the corpus
//! files of the directory named by the environment variable
//! [`CORPUS_DIR`] when it is set, by deterministic weighted resampling, as
//! the `corpus` sets of the parent module: each stored call weighs
//! `total / kept` of its stratum, so a set reflects the mix of calls of the
//! recording runs. The corpus points are mapped to the
//! points of `implicit-predicates` by the same source file the corpus tests
//! of that crate use, so the two mappings cannot drift apart.

use implicit_predicates::stages::insphere;
use implicit_predicates::stages::orient3d as stages;
use implicit_predicates::{AnyPoint, ExactCoords, Explicit, Point, Scratch, Sign};
use implicit_predicates_corpus::exact::{REFERENCE_INSPHERE_FACTOR, REFERENCE_ORIENT3D_FACTOR};
use implicit_predicates_corpus::weighted::weighted_sample;
use implicit_predicates_corpus::{Call, Corpus, Predicate};

use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use super::{InputSet, SET_SIZE, insphere_sets, orient3d_sets, read_corpora, sample_corpora};

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
    let wanted = |count: usize| if k == 0 { count > 0 } else { count == k };
    let draws = draw_calls(Predicate::Orient3d, wanted, seed, extra);
    assert_eq!(draws.len(), SET_SIZE, "no eligible call in the samples");
    draws
        .into_iter()
        .map(|(points, sign)| (points, REFERENCE_ORIENT3D_FACTOR * sign))
        .collect()
}

/// [`SET_SIZE`] recorded calls of `predicate` with `N` points, a number of
/// implicit points for which `wanted` holds, every point provided, and for
/// which `extra` holds, drawn by weighted resampling with seed `seed`, each
/// with its recorded sign as stored, in the convention of the recording
/// program; empty when no call is eligible.
fn draw_calls<const N: usize>(
    predicate: Predicate,
    wanted: impl Fn(usize) -> bool,
    seed: u64,
    extra: impl Fn(&[AnyPoint; N]) -> bool,
) -> Vec<([AnyPoint; N], i8)> {
    let (corpora, mapped) = recorded();
    let input = |c: usize, call: &Call| -> Option<[AnyPoint; N]> {
        if call.predicate != predicate || !wanted(implicit_count(&corpora[c], call)) {
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
    draws
        .into_iter()
        .map(|(c, i)| {
            let call = &corpora[c].calls[i];
            let points = input(c, call).expect("an eligible call");
            (points, call.sign)
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

/// The stage on big integers on the points of one call, as the chain runs
/// it: the big-integer coordinates of the four points are written to
/// `scratch`, the stage evaluates the predicate, and the arena is left as it
/// was.
///
/// # Panics
///
/// Panics if the arena is exhausted (use a growable one) or a point has a
/// non-finite input.
pub fn big_integers_stage(scratch: &mut Scratch<'_>, p: &ImplicitInput) -> Sign {
    let mark = scratch.mark();
    let [q0, q1, q2, q3] = p
        .each_ref()
        .map(|q| q.exact_big(scratch).expect("big-integer coordinates"));
    let sign = stages::big_integers_in(scratch, [&q0, &q1, &q2, &q3]).expect("a growable arena");
    scratch.reset(mark);
    sign
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

/// Five points of `insphere`, of any type of `implicit-predicates`.
pub type InSphereImplicitInput = [AnyPoint; 5];

/// The largest number of implicit points of an `insphere` call.
pub const INSPHERE_MAX_IMPLICIT: usize = 5;

/// Seed of the weighted resampling of [`insphere_implicit_sets`]: the set of
/// index `k` uses `INSPHERE_IMPLICIT_SEED + k`.
pub const INSPHERE_IMPLICIT_SEED: u64 = 0x696e_7370;

/// Seed of the weighted resampling of [`insphere_exact_sets`]: the set of
/// calls with `k` implicit points uses `INSPHERE_EXACT_SEED + 2 k`.
pub const INSPHERE_EXACT_SEED: u64 = 0x6973_6578;

/// The name of the set of `insphere` calls with `k` implicit points, or of
/// the mix of calls with one to five for `k == 0` in
/// [`insphere_implicit_sets`].
#[must_use]
pub fn insphere_set_name(k: usize) -> &'static str {
    [
        "mix",
        "1_implicit",
        "2_implicit",
        "3_implicit",
        "4_implicit",
        "5_implicit",
    ][k]
}

/// The sets of recorded `insphere` calls with implicit points: `mix`, the
/// calls with one to five implicit points, then one set for each number of
/// implicit points, in the order of [`insphere_set_name`], each case with
/// its recorded sign in the convention of `implicit_predicates_corpus::exact`.
#[must_use]
pub fn insphere_implicit_sets() -> Vec<(InputSet<InSphereImplicitInput>, Vec<i8>)> {
    (0..=INSPHERE_MAX_IMPLICIT)
        .map(|k| {
            let wanted = |count: usize| if k == 0 { count > 0 } else { count == k };
            let seed = INSPHERE_IMPLICIT_SEED + k as u64;
            let draws = draw_calls(Predicate::InSphere, wanted, seed, |_| true);
            assert_eq!(draws.len(), SET_SIZE, "no eligible call in the samples");
            let (cases, signs) = draws
                .into_iter()
                .map(|(points, sign)| (points, REFERENCE_INSPHERE_FACTOR * sign))
                .unzip();
            (
                InputSet {
                    name: insphere_set_name(k),
                    cases,
                },
                signs,
            )
        })
        .collect()
}

/// The `insphere` sets of the parent module, `random`, `near_degenerate`,
/// `degenerate` and `corpus`, with every point an [`AnyPoint::Explicit`].
#[must_use]
pub fn insphere_any_explicit_sets() -> Vec<InputSet<InSphereImplicitInput>> {
    insphere_sets()
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

/// Whether the first test of the filter of `insphere`, `filter_points`,
/// declines a call: the second, `filter_axes_points`, decides some of these
/// calls, and `insphere` has no stage on expansions, so every other, with
/// explicit points too, goes to the stage on big integers.
///
/// `benches/tests/predicate_inputs.rs` checks every drawn case against the
/// logic of the corpus sweep.
fn insphere_undecided(p: &InSphereImplicitInput) -> bool {
    let [a, b, c, d, e] = p;
    insphere::filter_points(a, b, c, d, e).is_none()
}

/// The stage on big integers of `insphere` on the points of one call, as
/// the chain runs it: the big-integer coordinates of the five points are
/// written to `scratch`, the stage evaluates the predicate, and the arena
/// is left as it was.
///
/// # Panics
///
/// Panics if the arena is exhausted (use a growable one) or a point has a
/// non-finite input.
pub fn insphere_big_integers_stage(scratch: &mut Scratch<'_>, p: &InSphereImplicitInput) -> Sign {
    let mark = scratch.mark();
    let [q0, q1, q2, q3, q4] = p
        .each_ref()
        .map(|q| q.exact_big(scratch).expect("big-integer coordinates"));
    let sign =
        insphere::big_integers_in(scratch, [&q0, &q1, &q2, &q3, &q4]).expect("a growable arena");
    scratch.reset(mark);
    sign
}

/// The recorded `insphere` calls with `k` implicit points that the first
/// test of the filter does not decide.
pub struct InSphereExactSet {
    /// The number of implicit points of each call, zero to five.
    pub implicit: usize,
    /// The points of each call.
    pub points: Vec<InSphereImplicitInput>,
    /// The recorded sign of each call, in the convention of
    /// `implicit_predicates_corpus::exact`.
    pub recorded: Vec<i8>,
    /// The arena that the stages work in.
    pub scratch: Scratch<'static>,
}

impl InSphereExactSet {
    /// The name of the set, used in the benchmark identifiers:
    /// `undecided/<k>_implicit`.
    #[must_use]
    pub fn name(&self) -> String {
        format!("undecided/{}_implicit", self.implicit)
    }
}

/// For `k` from zero to five, the [`SET_SIZE`] recorded `insphere` calls
/// with `k` implicit points that the first test of the filter does not
/// decide, drawn by
/// weighted resampling with seed `INSPHERE_EXACT_SEED + 2 k`. A number of
/// implicit points with no such call in the corpora read is left out.
#[must_use]
pub fn insphere_exact_sets() -> Vec<InSphereExactSet> {
    let mut sets = Vec::new();
    for k in 0..=INSPHERE_MAX_IMPLICIT {
        let seed = INSPHERE_EXACT_SEED + 2 * k as u64;
        let draws = draw_calls(Predicate::InSphere, |n| n == k, seed, insphere_undecided);
        if draws.is_empty() {
            continue;
        }
        let (points, signs): (Vec<_>, Vec<_>) = draws.into_iter().unzip();
        sets.push(InSphereExactSet {
            implicit: k,
            points,
            recorded: signs
                .into_iter()
                .map(|s| REFERENCE_INSPHERE_FACTOR * s)
                .collect(),
            scratch: Scratch::growable(1 << 16),
        });
    }
    sets
}
