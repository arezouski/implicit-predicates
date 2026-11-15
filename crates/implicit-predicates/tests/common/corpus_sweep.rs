//! A sweep of the recorded calls of corpora, for `orient3d` ([`Tally`]) and
//! for `insphere` ([`InSphereTally`]): every call whose points this crate
//! provides gets the recorded sign, the chain agrees with the first stage
//! that decides, and the weight of the calls each stage decides first is
//! tallied by number of implicit points.
//!
//! What the two predicates share is here once: the buckets by number of
//! implicit points ([`Bucket`]), the calls decided on big integers by
//! reason ([`Reasons`]), the mismatches ([`Mismatches`]) and the table. Each
//! predicate keeps its own columns, its own way of calling the chain and its
//! own number of points.

use std::fmt::Write as _;

use implicit_predicates::stages::insphere as insphere_stages;
use implicit_predicates::stages::orient3d as stages;
use implicit_predicates::{AnyPoint, BigCoords, ExactCoords, ExactError, Point, Scratch, Sign};
use implicit_predicates_corpus::exact::{REFERENCE_INSPHERE_FACTOR, REFERENCE_ORIENT3D_FACTOR};
use implicit_predicates_corpus::weighted::weights;
use implicit_predicates_corpus::{Corpus, Predicate};

use super::corpus_points::{Mapped, call_points, implicit_count, points};
use super::insphere::{self as insphere_chain, Decision};
pub use super::why::{Why, why_exact_failed};
use super::{CHAIN, STAGED, all_in_range, first_deciding, in_range, refs};

/// What a sweep of corpora does, whatever the predicate.
pub trait Sweep: Default {
    /// Checks and tallies every call of the predicate in `corpus`; returns
    /// the number of calls checked without and with implicit points.
    fn add_corpus(&mut self, corpus: &Corpus) -> (usize, usize);

    /// Adds what another sweep found, as if its corpora had been added
    /// here after the ones already added.
    fn merge(&mut self, other: &Self);

    /// Every count of the sweep, which two sweeps of the same calls share
    /// exactly.
    fn counts(&self) -> Vec<usize>;

    /// Every weight of the sweep, which two sweeps of the same calls in
    /// another order share up to rounding.
    fn sums(&self) -> Vec<f64>;

    /// The calls that got a wrong sign or on which the stages disagree.
    fn mismatches(&self) -> &Mismatches;
}

/// The weighted tally of the calls with a given number of implicit points,
/// with `C` columns, one per stage a call can be attributed to.
#[derive(Clone, Debug)]
pub struct Bucket<const C: usize> {
    /// Stored calls checked.
    pub stored: usize,
    /// Their weight: the calls of the recording runs they stand for.
    pub weight: f64,
    /// The weight of the calls each column decides first.
    pub columns: [f64; C],
    /// The weight of the calls whose recorded sign is zero.
    pub recorded_zero: f64,
    /// Stored calls with a point of a kind this crate does not provide.
    pub unsupported: usize,
    /// Stored calls checked with a point on a line whose parameter is
    /// outside `[0, 1]`.
    pub unchecked: usize,
}

impl<const C: usize> Default for Bucket<C> {
    fn default() -> Self {
        Bucket {
            stored: 0,
            weight: 0.0,
            columns: [0.0; C],
            recorded_zero: 0.0,
            unsupported: 0,
            unchecked: 0,
        }
    }
}

impl<const C: usize> Bucket<C> {
    fn add(&mut self, other: &Bucket<C>) {
        self.stored += other.stored;
        self.weight += other.weight;
        for (a, b) in self.columns.iter_mut().zip(other.columns) {
            *a += b;
        }
        self.recorded_zero += other.recorded_zero;
        self.unsupported += other.unsupported;
        self.unchecked += other.unchecked;
    }

    /// Tallies a checked call decided first by `column`.
    fn tally<const N: usize>(
        &mut self,
        column: usize,
        recorded: i8,
        weight: f64,
        p: &[AnyPoint; N],
    ) {
        self.stored += 1;
        self.weight += weight;
        self.columns[column] += weight;
        self.recorded_zero += if recorded == 0 { weight } else { 0.0 };
        self.unchecked += usize::from(
            p.iter()
                .any(|q| matches!(q, AnyPoint::Lnc(l) if !(0.0..=1.0).contains(&l.t()))),
        );
    }

    /// The counts of the bucket, for [`Sweep::counts`].
    fn counts(&self) -> [usize; 3] {
        [self.stored, self.unsupported, self.unchecked]
    }

    /// The weights of the bucket, for [`Sweep::sums`].
    fn sums(&self) -> impl Iterator<Item = f64> + '_ {
        self.columns
            .iter()
            .copied()
            .chain([self.weight, self.recorded_zero])
    }
}

/// The sum of `buckets`.
fn total<const C: usize>(buckets: &[Bucket<C>]) -> Bucket<C> {
    let mut all = Bucket::default();
    for bucket in buckets {
        all.add(bucket);
    }
    all
}

/// The most examples kept of each reason for a call that the stage on big
/// integers decides.
const EXAMPLES: usize = 5;

/// The most mismatches kept described.
const MISMATCHES: usize = 20;

/// The calls that the stage on big integers decides, because no stage
/// before it does, by reason.
#[derive(Clone, Debug, Default)]
pub struct Reasons {
    /// The stored count and the weight of the calls of each reason, in the
    /// order in which the reasons were first seen.
    pub counts: Vec<(Why, usize, f64)>,
    /// Up to a few of those calls of each reason, described.
    pub examples: Vec<(Why, String)>,
}

impl Reasons {
    /// Counts a call of reason `why`, described by `describe` if it is
    /// kept as an example.
    fn add(&mut self, why: Why, weight: f64, describe: impl FnOnce() -> String) {
        self.add_count(why, 1, weight);
        if self.examples.iter().filter(|(w, _)| *w == why).count() < EXAMPLES {
            self.examples.push((why, describe()));
        }
    }

    fn add_count(&mut self, why: Why, stored: usize, weight: f64) {
        match self.counts.iter_mut().find(|(w, ..)| *w == why) {
            Some(entry) => {
                entry.1 += stored;
                entry.2 += weight;
            }
            None => self.counts.push((why, stored, weight)),
        }
    }

    fn merge(&mut self, other: &Reasons) {
        for &(why, stored, weight) in &other.counts {
            self.add_count(why, stored, weight);
        }
        for (why, example) in &other.examples {
            if self.examples.iter().filter(|(w, _)| w == why).count() < EXAMPLES {
                self.examples.push((*why, example.clone()));
            }
        }
    }

    /// The stored count of each reason, in the order of [`Why`].
    fn sorted(&self) -> Vec<(Why, usize, f64)> {
        let mut counts = self.counts.clone();
        counts.sort_by_key(|&(why, ..)| why);
        counts
    }

    /// The reasons after `title`, each with its share of `base`, the weight
    /// that `of` names, then the examples.
    fn report(&self, line: &mut impl FnMut(String), title: &str, base: f64, of: &str) {
        let counts = self.sorted();
        line(format!("{title}: {}", counts.len()));
        for (why, stored, weight) in &counts {
            line(format!(
                "  {why:?}: {stored} stored, weight {weight:.1}, {:.6} % of {of}",
                100.0 * weight / base
            ));
        }
        for (why, example) in &self.examples {
            line(format!("  {why:?} example: {example}"));
        }
    }
}

/// The calls whose sign differs from the recorded one, or on which the
/// stages disagree.
#[derive(Clone, Debug, Default)]
pub struct Mismatches {
    /// How many.
    pub count: usize,
    /// The first [`MISMATCHES`] of them, described.
    pub described: Vec<String>,
}

impl Mismatches {
    fn add(&mut self, describe: impl FnOnce() -> String) {
        self.count += 1;
        if self.described.len() < MISMATCHES {
            self.described.push(describe());
        }
    }

    fn merge(&mut self, other: &Mismatches) {
        self.count += other.count;
        let room = MISMATCHES.saturating_sub(self.described.len());
        self.described
            .extend(other.described.iter().take(room).cloned());
    }

    fn report(&self, line: &mut impl FnMut(String)) {
        line(format!("mismatches: {}", self.count));
        for m in &self.described {
            line(format!("  {m}"));
        }
    }
}

/// The points of a corpus that a sweep could not use, or built unchecked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PointCounts {
    pub unsupported: usize,
    pub non_finite: usize,
    pub unchecked: usize,
}

impl PointCounts {
    fn add(&mut self, mapped: &Mapped) {
        self.unsupported += mapped.unsupported;
        self.non_finite += mapped.non_finite;
        self.unchecked += mapped.unchecked;
    }

    fn merge(&mut self, other: &PointCounts) {
        self.unsupported += other.unsupported;
        self.non_finite += other.non_finite;
        self.unchecked += other.unchecked;
    }

    fn report(&self, line: &mut impl FnMut(String)) {
        line(format!(
            "\npoints: {} unsupported, {} not finite, {} with t outside [0, 1]",
            self.unsupported, self.non_finite, self.unchecked
        ));
    }
}

/// The table of weighted fractions under the title `name`: one row per
/// number of implicit points, one for the calls with implicit points and one
/// for every call, with the `columns` and the largest high water of the
/// arena of each bucket.
fn table<const C: usize>(
    line: &mut impl FnMut(String),
    name: &str,
    columns: &[&str; C],
    buckets: &[Bucket<C>],
    high_water: &[usize],
) {
    line(format!("\n## {name}\n"));
    let header: Vec<_> = columns.iter().map(|c| format!("{c} %")).collect();
    line(format!(
        "| implicit | stored | weighted | recorded zero % | {} | unsupported | t outside [0, 1] | high water |",
        header.join(" | ")
    ));
    line(format!("|{}", " --- |".repeat(C + 7)));
    let row = |label: &str, b: &Bucket<C>, high: String| {
        let pct = |x: f64| {
            if b.weight > 0.0 {
                format!("{:.4}", 100.0 * x / b.weight)
            } else {
                "-".into()
            }
        };
        let columns: Vec<_> = b.columns.iter().map(|&x| pct(x)).collect();
        format!(
            "| {label} | {} | {:.0} | {} | {} | {} | {} | {high} |",
            b.stored,
            b.weight,
            pct(b.recorded_zero),
            columns.join(" | "),
            b.unsupported,
            b.unchecked
        )
    };
    for (k, bucket) in buckets.iter().enumerate() {
        line(row(&k.to_string(), bucket, high_water[k].to_string()));
    }
    let max = |s: &[usize]| s.iter().copied().max().unwrap_or(0).to_string();
    let most = buckets.len() - 1;
    line(row(
        &format!("1-{most}"),
        &total(&buckets[1..]),
        max(&high_water[1..]),
    ));
    line(row("all", &total(buckets), max(high_water)));
}

/// Runs `f` with a function that appends a line to a string, and returns
/// the string.
fn lines(f: impl FnOnce(&mut dyn FnMut(String))) -> String {
    let mut out = String::new();
    f(&mut |s: String| writeln!(out, "{s}").expect("writing to a string"));
    out
}

/// The stages a call of `orient3d` can be attributed to, in the order of
/// the chain: the filter giving a sign, the filter giving `Zero` (some axis
/// is flat, so the value is exactly zero), the exact stages for explicit
/// points, the exact stage over arena spans, and the stage on big integers,
/// which decides every call that the others do not.
pub const COLUMNS: [&str; 6] = [
    "filter",
    "certain zero",
    "exact_differences",
    "expansions_translated",
    "expansions_translated_in",
    "big_integers",
];

const CERTAIN_ZERO: usize = 1;
const SPANS_STAGE: usize = 4;
const BIG_INTEGERS: usize = 5;

/// The first stage that decides a call, as an index in [`COLUMNS`], with
/// its sign; or [`BIG_INTEGERS`] and the reason why no stage on expansions
/// decides it.
fn attribute(scratch: &mut Scratch<'_>, p: &[AnyPoint; 4]) -> (usize, Result<Sign, Why>) {
    let [a, b, c, d] = p;
    if let Some(sign) = stages::filter_points(a, b, c, d) {
        let column = if sign == Sign::Zero { CERTAIN_ZERO } else { 0 };
        return (column, Ok(sign));
    }
    if p.iter().all(Point::is_explicit) {
        let q = p.map(|x| *x.approx());
        // The filter of `CHAIN` has already declined; the stage on big
        // integers is attributed with its reason.
        return CHAIN[..STAGED]
            .iter()
            .enumerate()
            .skip(1)
            .find_map(|(i, (_, stage))| stage(refs(&q)).map(|sign| (i + 1, Ok(sign))))
            .unwrap_or_else(|| {
                // `expansions_translated` decides whenever every coordinate
                // is in its range.
                let why = if all_in_range(&q, stages::TRANSLATED_RANGE) {
                    Why::Unexplained
                } else {
                    Why::CoordinateOutOfRange
                };
                (BIG_INTEGERS, Err(why))
            });
    }
    let mark = scratch.mark();
    let result = attribute_spans(scratch, p);
    scratch.reset(mark);
    result
}

/// [`attribute`] after the filter, for points that are not all explicit:
/// the stages of `exact_stages_in`, in its order.
fn attribute_spans(scratch: &mut Scratch<'_>, p: &[AnyPoint; 4]) -> (usize, Result<Sign, Why>) {
    let mut q: [Option<ExactCoords>; 4] = [None; 4];
    for (slot, point) in q.iter_mut().zip(p) {
        match point.exact(scratch) {
            Ok(c) => *slot = Some(c),
            Err(ExactError::OutOfRange) => return (BIG_INTEGERS, Err(why_exact_failed(point))),
            Err(ExactError::Exhausted(e)) => panic!("a growable arena is exhausted: {e:?}"),
            Err(e) => panic!("an error of `Point::exact` that this sweep does not know: {e:?}"),
        }
    }
    let q = q.map(|c| c.expect("every point is written"));
    let r = [&q[0], &q[1], &q[2], &q[3]];
    let exhausted = |e| panic!("a growable arena is exhausted: {e:?}");
    if let Some(sign) = stages::expansions_translated_in(scratch, r).unwrap_or_else(exhausted) {
        return (SPANS_STAGE, Ok(sign));
    }
    let spans = q.iter().flat_map(|c| [c.x, c.y, c.z]);
    let why = if spans
        .clone()
        .any(|s| s.len() > stages::MAX_COORD_COMPONENTS)
    {
        Why::TooManyComponents
    } else if spans
        .flat_map(|s| scratch.get(s))
        .any(|&x| !in_range(x, stages::SPANS_TRANSLATED_RANGE))
    {
        Why::ComponentOutOfRange
    } else {
        Why::Unexplained
    };
    (BIG_INTEGERS, Err(why))
}

/// For explicit points: whether `column` is the stage that the order of
/// `CHAIN`, the order of the generated chain, decides first, and whether
/// every stage of `CHAIN` that decides gives the recorded sign (`wrong` is
/// false), not only the first.
fn explicit_chain_agrees(p: &[AnyPoint; 4], column: usize, wrong: impl Fn(Sign) -> bool) -> bool {
    let q = p.map(|x| *x.approx());
    let expected = match first_deciding(&q) {
        Some((0, Sign::Zero)) => CERTAIN_ZERO,
        Some((0, _)) => 0,
        Some((i, _)) if i < STAGED => i + 1,
        Some(_) => BIG_INTEGERS,
        // Every input of the corpora is finite, so some stage decides.
        None => return false,
    };
    let every_stage_right = CHAIN
        .iter()
        .all(|(_, stage)| !stage(refs(&q)).is_some_and(&wrong));
    column == expected && every_stage_right
}

/// What a sweep of the `orient3d` calls found, by number of implicit points
/// (0 to 4).
#[derive(Debug)]
pub struct Tally {
    pub buckets: [Bucket<{ COLUMNS.len() }>; 5],
    /// The calls that the stage on big integers decides, by reason.
    pub big_integers: Reasons,
    /// Calls whose sign differs from the recorded one, or on which the
    /// public functions disagree with the stages.
    pub mismatches: Mismatches,
    /// Points of the corpora mapped to `None` or built unchecked.
    pub points: PointCounts,
    /// The largest `high_water` of the arena of `orient3d_in`, by number
    /// of implicit points.
    pub high_water: [usize; 5],
    /// The arena of `orient3d_in`, one per number of implicit points, and
    /// the one used to attribute calls.
    arenas: [Scratch<'static>; 5],
    scratch: Scratch<'static>,
}

impl Default for Tally {
    fn default() -> Self {
        Tally {
            buckets: Default::default(),
            big_integers: Reasons::default(),
            mismatches: Mismatches::default(),
            points: PointCounts::default(),
            high_water: [0; 5],
            arenas: std::array::from_fn(|_| Scratch::growable(1 << 14)),
            scratch: Scratch::growable(1 << 14),
        }
    }
}

impl Sweep for Tally {
    fn add_corpus(&mut self, corpus: &Corpus) -> (usize, usize) {
        let mapped = points(corpus);
        self.points.add(&mapped);
        let mut checked = (0, 0);
        for (index, (call, weight)) in corpus.calls.iter().zip(weights(corpus)).enumerate() {
            if call.predicate != Predicate::Orient3d {
                continue;
            }
            let implicit = implicit_count(corpus, call);
            let Some(p) = call_points::<4>(&mapped, call) else {
                self.buckets[implicit].unsupported += 1;
                continue;
            };
            if implicit == 0 {
                checked.0 += 1;
            } else {
                checked.1 += 1;
            }
            self.check_call(corpus, index, call.sign, weight, implicit, &p);
        }
        for (high, arena) in self.high_water.iter_mut().zip(&self.arenas) {
            *high = (*high).max(arena.high_water());
        }
        checked
    }

    fn merge(&mut self, other: &Tally) {
        for (bucket, added) in self.buckets.iter_mut().zip(&other.buckets) {
            bucket.add(added);
        }
        self.big_integers.merge(&other.big_integers);
        self.mismatches.merge(&other.mismatches);
        self.points.merge(&other.points);
        for (high, added) in self.high_water.iter_mut().zip(other.high_water) {
            *high = (*high).max(added);
        }
    }

    fn counts(&self) -> Vec<usize> {
        let mut counts: Vec<_> = self.buckets.iter().flat_map(Bucket::counts).collect();
        let PointCounts {
            unsupported,
            non_finite,
            unchecked,
        } = self.points;
        counts.extend([unsupported, non_finite, unchecked, self.mismatches.count]);
        counts.extend(self.high_water);
        counts.extend(self.big_integers.sorted().iter().map(|&(_, n, _)| n));
        counts
    }

    fn sums(&self) -> Vec<f64> {
        self.buckets.iter().flat_map(Bucket::sums).collect()
    }

    fn mismatches(&self) -> &Mismatches {
        &self.mismatches
    }
}

impl Tally {
    #[allow(clippy::too_many_arguments)]
    fn check_call(
        &mut self,
        corpus: &Corpus,
        index: usize,
        recorded: i8,
        weight: f64,
        implicit: usize,
        p: &[AnyPoint; 4],
    ) {
        let (column, result) = attribute(&mut self.scratch, p);
        let [a, b, c, d] = p;
        let public = implicit_predicates::orient3d(a, b, c, d);
        let arena = &mut self.arenas[implicit];
        let in_arena = implicit_predicates::orient3d_in(arena, a, b, c, d)
            .unwrap_or_else(|e| panic!("a growable arena is exhausted: {e:?}"));
        let describe = || {
            format!(
                "{} call {index}: recorded {recorded}, stage {}, orient3d {public:?}, \
                 orient3d_in {in_arena:?}, points {p:?}",
                corpus.model, COLUMNS[column]
            )
        };
        // The sign of the first stage on expansions that decides, if one
        // does; otherwise the stage on big integers decides.
        let staged = result.ok();
        let wrong = |s: Sign| REFERENCE_ORIENT3D_FACTOR * i8::from(s) != recorded;
        let unexplained = result == Err(Why::Unexplained);
        if wrong(public)
            || unexplained
            || staged.is_some_and(|s| s != public)
            || in_arena != public
            || arena.used() != 0
            || (p.iter().all(Point::is_explicit) && !explicit_chain_agrees(p, column, wrong))
        {
            self.mismatches.add(describe);
        }
        self.buckets[implicit].tally(column, recorded, weight, p);
        if let Err(why) = result {
            self.big_integers.add(why, weight, describe);
        }
    }

    /// The tally of the calls with 1 to 4 implicit points.
    pub fn implicit(&self) -> Bucket<{ COLUMNS.len() }> {
        total(&self.buckets[1..])
    }

    /// The tally of every call.
    pub fn all(&self) -> Bucket<{ COLUMNS.len() }> {
        total(&self.buckets)
    }

    /// The table of weighted fractions, one row per number of implicit
    /// points, then the counts and the calls that the stage on big integers
    /// decides, by reason.
    pub fn report(&self, name: &str) -> String {
        lines(|line| {
            let mut line = |s| line(s);
            table(&mut line, name, &COLUMNS, &self.buckets, &self.high_water);
            self.points.report(&mut line);
            self.mismatches.report(&mut line);
            self.big_integers.report(
                &mut line,
                "decided on big integers, by reason",
                self.implicit().weight,
                "the weight with implicit points",
            );
        })
    }
}

/// The stages a call of `insphere` can be attributed to: those of its
/// chain, in its order, the two tests of the filter and the stage on big
/// integers. A certain zero of `filter_axes` is in its column.
pub const INSPHERE_COLUMNS: [&str; 3] = insphere_chain::CHAIN;

/// The `insphere` calls whose recorded sign is zero, the ties that
/// `insphere_sos` breaks, with the position of each point in the table of
/// points of its model as its index.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ties {
    /// Stored calls whose recorded sign is zero.
    pub calls: usize,
    /// Those whose five points are coplanar: every `orient3d` of four of
    /// them is zero. No order of the indices breaks such a tie.
    pub coplanar: usize,
    /// Those that give a position of the table twice: two equal indices,
    /// which no order of the indices can tell apart. They are not given to
    /// `insphere_sos`.
    pub repeated: usize,
    /// The others on which `insphere_sos` gives zero.
    pub sos_zero: usize,
}

impl Ties {
    /// Counts the call on the points `p` at the positions `idx`.
    fn add(&mut self, p: &[AnyPoint; 5], idx: [usize; 5]) {
        self.calls += 1;
        let [a, b, c, d, e] = p;
        let coplanar = [
            implicit_predicates::orient3d(b, c, d, e),
            implicit_predicates::orient3d(a, c, d, e),
            implicit_predicates::orient3d(a, b, d, e),
            implicit_predicates::orient3d(a, b, c, e),
            implicit_predicates::orient3d(a, b, c, d),
        ]
        .iter()
        .all(|&s| s == Sign::Zero);
        self.coplanar += usize::from(coplanar);
        let repeated = (0..5).any(|i| (i + 1..5).any(|j| idx[i] == idx[j]));
        self.repeated += usize::from(repeated);
        if !repeated {
            let sign = implicit_predicates::insphere_sos(a, b, c, d, e, idx);
            self.sos_zero += usize::from(sign == Sign::Zero);
        }
    }

    fn merge(&mut self, other: &Ties) {
        self.calls += other.calls;
        self.coplanar += other.coplanar;
        self.repeated += other.repeated;
        self.sos_zero += other.sos_zero;
    }

    fn counts(&self) -> [usize; 4] {
        [self.calls, self.coplanar, self.repeated, self.sos_zero]
    }

    fn report(&self, line: &mut impl FnMut(String)) {
        line(format!(
            "calls with a recorded sign of zero: {}, of which {} have five coplanar points \
             and {} repeat a point of the model; insphere_sos gives zero on {} of the others",
            self.calls, self.coplanar, self.repeated, self.sos_zero
        ));
    }
}

/// What a sweep of the `insphere` calls found, by number of implicit points
/// (0 to 5).
///
/// `insphere` has no stage on expansions: the sweep calls `staged_in` of the
/// hidden module, and the public `insphere` and `insphere_in`, which must
/// give its sign, and every call that neither test
/// of the filter decides, with explicit points too, goes to the stage on
/// big integers, in the arena, for the reason [`Why::BigIntegersOnly`].
#[derive(Debug)]
pub struct InSphereTally {
    pub buckets: [Bucket<{ INSPHERE_COLUMNS.len() }>; 6],
    /// The stored count and the weight of the explicit calls that the
    /// first test of the filter does not decide.
    pub explicit_past_filter: (usize, f64),
    /// The stored count and the weight of the explicit calls that neither
    /// test of the filter decides: those that use the arena.
    pub explicit_past_filters: (usize, f64),
    /// The calls with implicit points that the stage on big integers
    /// decides, by reason.
    pub big_integers: Reasons,
    /// The explicit calls that the stage on big integers decides, by
    /// reason.
    pub big_integers_explicit: Reasons,
    /// Calls whose sign times the factor differs from the recorded one, or
    /// on which a stage that decides alone disagrees with the chain.
    pub mismatches: Mismatches,
    /// Points of the corpora mapped to `None` or built unchecked.
    pub points: PointCounts,
    /// The largest `high_water` of the arena of `staged_in`, by number of
    /// implicit points.
    pub high_water: [usize; 6],
    /// The stored count and the weight of the calls by the sign of
    /// `orient3d(a, b, c, d)` on their first four points: negative, zero,
    /// positive. The sign of `insphere` is read against it.
    pub orientation: [(usize, f64); 3],
    /// The calls whose recorded sign is zero, the ties of `insphere`.
    pub ties: Ties,
    /// The arena of `staged_in`, one per number of implicit points, and the
    /// one used to attribute calls and run the stages alone.
    arenas: [Scratch<'static>; 6],
    scratch: Scratch<'static>,
    /// The recorded sign is taken to be this factor times ours.
    factor: i8,
    /// How much each call is cross-checked.
    cross_check: CrossCheck,
}

/// How much a sweep of `insphere` cross-checks each call, beyond the chain
/// against the recorded sign, the stage that decides first with its reason,
/// and the arena left as it was. Both give the same table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrossCheck {
    /// Also every stage of the chain alone on every call: each that decides
    /// must give the sign of the chain, and the first that does must be the
    /// one the chain stops at.
    EveryStageAlone,
    /// No stage alone: the sign of the stage on big integers is that of the
    /// chain, which reaches it only when the filter does not decide.
    ChainOnly,
}

impl Default for InSphereTally {
    /// A sweep with the factor of the reference and
    /// [`CrossCheck::EveryStageAlone`].
    fn default() -> Self {
        InSphereTally::new(REFERENCE_INSPHERE_FACTOR, CrossCheck::EveryStageAlone)
    }
}

impl InSphereTally {
    /// A sweep that takes the recorded sign to be `factor` times ours and
    /// cross-checks each call as `cross_check` says. The factor of the
    /// reference is `REFERENCE_INSPHERE_FACTOR`; any other must give
    /// mismatches on calls whose recorded sign is not zero.
    pub fn new(factor: i8, cross_check: CrossCheck) -> InSphereTally {
        InSphereTally {
            buckets: Default::default(),
            explicit_past_filter: (0, 0.0),
            explicit_past_filters: (0, 0.0),
            big_integers: Reasons::default(),
            big_integers_explicit: Reasons::default(),
            mismatches: Mismatches::default(),
            points: PointCounts::default(),
            high_water: [0; 6],
            orientation: [(0, 0.0); 3],
            ties: Ties::default(),
            arenas: std::array::from_fn(|_| Scratch::growable(1 << 14)),
            scratch: Scratch::growable(1 << 14),
            factor,
            cross_check,
        }
    }
}

impl Sweep for InSphereTally {
    fn add_corpus(&mut self, corpus: &Corpus) -> (usize, usize) {
        let mapped = points(corpus);
        self.points.add(&mapped);
        let mut checked = (0, 0);
        for (index, (call, weight)) in corpus.calls.iter().zip(weights(corpus)).enumerate() {
            if call.predicate != Predicate::InSphere {
                continue;
            }
            let implicit = implicit_count(corpus, call);
            let Some(p) = call_points::<5>(&mapped, call) else {
                self.buckets[implicit].unsupported += 1;
                continue;
            };
            if implicit == 0 {
                checked.0 += 1;
            } else {
                checked.1 += 1;
            }
            self.check_call(corpus, index, call.sign, weight, implicit, &p);
            if call.sign == 0 {
                let idx: [usize; 5] = std::array::from_fn(|k| call.points()[k] as usize);
                self.ties.add(&p, idx);
            }
        }
        for (high, arena) in self.high_water.iter_mut().zip(&self.arenas) {
            *high = (*high).max(arena.high_water());
        }
        checked
    }

    fn merge(&mut self, other: &InSphereTally) {
        self.ties.merge(&other.ties);
        for (bucket, added) in self.buckets.iter_mut().zip(&other.buckets) {
            bucket.add(added);
        }
        self.explicit_past_filter.0 += other.explicit_past_filter.0;
        self.explicit_past_filter.1 += other.explicit_past_filter.1;
        self.explicit_past_filters.0 += other.explicit_past_filters.0;
        self.explicit_past_filters.1 += other.explicit_past_filters.1;
        self.big_integers.merge(&other.big_integers);
        self.big_integers_explicit
            .merge(&other.big_integers_explicit);
        self.mismatches.merge(&other.mismatches);
        self.points.merge(&other.points);
        for (high, added) in self.high_water.iter_mut().zip(other.high_water) {
            *high = (*high).max(added);
        }
        for (sum, added) in self.orientation.iter_mut().zip(other.orientation) {
            sum.0 += added.0;
            sum.1 += added.1;
        }
    }

    fn counts(&self) -> Vec<usize> {
        let mut counts: Vec<_> = self.buckets.iter().flat_map(Bucket::counts).collect();
        let PointCounts {
            unsupported,
            non_finite,
            unchecked,
        } = self.points;
        counts.extend([unsupported, non_finite, unchecked, self.mismatches.count]);
        counts.extend([self.explicit_past_filter.0, self.explicit_past_filters.0]);
        counts.extend(self.high_water);
        counts.extend(self.orientation.map(|(n, _)| n));
        counts.extend(self.ties.counts());
        for reasons in [&self.big_integers, &self.big_integers_explicit] {
            counts.extend(reasons.sorted().iter().map(|&(_, n, _)| n));
        }
        counts
    }

    fn sums(&self) -> Vec<f64> {
        let mut sums: Vec<_> = self.buckets.iter().flat_map(Bucket::sums).collect();
        sums.extend([self.explicit_past_filter.1, self.explicit_past_filters.1]);
        sums.extend(self.orientation.map(|(_, w)| w));
        sums
    }

    fn mismatches(&self) -> &Mismatches {
        &self.mismatches
    }
}

impl InSphereTally {
    #[allow(clippy::too_many_arguments)]
    fn check_call(
        &mut self,
        corpus: &Corpus,
        index: usize,
        recorded: i8,
        weight: f64,
        implicit: usize,
        p: &[AnyPoint; 5],
    ) {
        let before_big = insphere_chain::deciding_before_big_integers(&mut self.scratch, p);
        // In `EveryStageAlone`, every stage alone; otherwise none.
        let alone: Option<[Option<Sign>; INSPHERE_COLUMNS.len()]> =
            (self.cross_check == CrossCheck::EveryStageAlone).then(|| {
                std::array::from_fn(|stage| insphere_chain::run_stage(stage, &mut self.scratch, p))
            });
        let [a, b, c, d, e] = p;
        let arena = &mut self.arenas[implicit];
        let chain = insphere_stages::staged_in(arena, a, b, c, d, e)
            .unwrap_or_else(|e| panic!("a growable arena is exhausted: {e:?}"));
        // The public functions: the one with the arena of the thread, and
        // the one with an arena of the caller.
        let public = implicit_predicates::insphere(a, b, c, d, e);
        let used_before = self.scratch.used();
        let public_in = implicit_predicates::insphere_in(&mut self.scratch, a, b, c, d, e);
        let public_in_used = self.scratch.used();
        let orientation = implicit_predicates::orient3d(a, b, c, d);
        let slot = match orientation {
            Sign::Negative => 0,
            Sign::Zero => 1,
            Sign::Positive => 2,
        };
        self.orientation[slot].0 += 1;
        self.orientation[slot].1 += weight;
        // When neither test of the filter decides, the sign of the stage on
        // big integers: the one it gave alone, or else the one of the chain,
        // which ends with it.
        let decision = before_big.map(Some).unwrap_or_else(|why| {
            let sign = match alone {
                Some(alone) => alone[insphere_chain::BIG_INTEGERS],
                None => Some(chain),
            };
            sign.map(|sign| Decision {
                stage: insphere_chain::BIG_INTEGERS,
                sign,
                why: Some(why),
            })
        });
        let describe = || {
            let alone = match alone {
                Some(alone) => INSPHERE_COLUMNS
                    .iter()
                    .zip(alone)
                    .map(|(name, sign)| format!("{name} {sign:?}"))
                    .collect::<Vec<_>>()
                    .join(", "),
                None => "not run".into(),
            };
            format!(
                "{} call {index}: recorded {recorded}, decision {decision:?}, staged_in {chain:?}, \
                 insphere {public:?}, insphere_in {public_in:?}, alone: {alone}, points {p:?}",
                corpus.model
            )
        };
        let agrees = decision.is_some_and(|d| {
            d.sign == chain
                && d.why != Some(Why::Unexplained)
                && alone.is_none_or(|alone| {
                    alone.iter().position(Option::is_some) == Some(d.stage)
                        && alone.iter().all(|s| s.is_none_or(|s| s == chain))
                })
        });
        let public_agrees =
            public == chain && public_in == Ok(chain) && public_in_used == used_before;
        if self.factor * i8::from(chain) != recorded
            || !agrees
            || !public_agrees
            || arena.used() != 0
        {
            self.mismatches.add(describe);
        }
        // A call without a decision has an input that is not finite, which
        // the corpora do not have; it is a mismatch, tallied with the stage
        // on big integers.
        let column = decision.map_or(insphere_chain::BIG_INTEGERS, |d| d.stage);
        self.buckets[implicit].tally(column, recorded, weight, p);
        let explicit = implicit == 0;
        if explicit && column != insphere_chain::FILTER {
            self.explicit_past_filter.0 += 1;
            self.explicit_past_filter.1 += weight;
        }
        if explicit && column == insphere_chain::BIG_INTEGERS {
            self.explicit_past_filters.0 += 1;
            self.explicit_past_filters.1 += weight;
        }
        if let Some(why) = decision.and_then(|d| d.why) {
            let reasons = if explicit {
                &mut self.big_integers_explicit
            } else {
                &mut self.big_integers
            };
            reasons.add(why, weight, describe);
        }
    }

    /// The tally of the calls with 1 to 5 implicit points.
    pub fn implicit(&self) -> Bucket<{ INSPHERE_COLUMNS.len() }> {
        total(&self.buckets[1..])
    }

    /// The table of weighted fractions, one row per number of implicit
    /// points; then the explicit calls that the first test of the filter
    /// does not decide and those that neither test decides, and the calls
    /// that the stage on big integers decides, by reason, for the calls with
    /// implicit points and for the explicit ones.
    pub fn report(&self, name: &str) -> String {
        lines(|line| {
            let mut line = |s| line(s);
            let explicit_weight = self.buckets[0].weight;
            table(
                &mut line,
                name,
                &INSPHERE_COLUMNS,
                &self.buckets,
                &self.high_water,
            );
            self.points.report(&mut line);
            let [negative, zero, positive] = self.orientation;
            let total = negative.1 + zero.1 + positive.1;
            line(format!(
                "orientation of the first four points, weighted: negative {:.4} %, zero {:.4} %, \
                 positive {:.4} % ({} / {} / {} stored)",
                100.0 * negative.1 / total,
                100.0 * zero.1 / total,
                100.0 * positive.1 / total,
                negative.0,
                zero.0,
                positive.0,
            ));
            for (what, (stored, weight)) in [
                (
                    "the first test of the filter does not",
                    self.explicit_past_filter,
                ),
                ("neither test of the filter", self.explicit_past_filters),
            ] {
                line(format!(
                    "explicit calls that {what} decide: {stored} stored, weight {weight:.1}, \
                     {:.6} % of the weight of explicit calls",
                    100.0 * weight / explicit_weight
                ));
            }
            self.ties.report(&mut line);
            self.mismatches.report(&mut line);
            self.big_integers.report(
                &mut line,
                "decided on big integers, by reason",
                self.implicit().weight,
                "the weight with implicit points",
            );
            self.big_integers_explicit.report(
                &mut line,
                "explicit calls decided on big integers, by reason",
                explicit_weight,
                "the weight of explicit calls",
            );
        })
    }
}

/// What the stage on big integers alone gave on the calls of one predicate
/// of corpora, without the stages before it.
#[derive(Debug, Default)]
pub struct BigAlone {
    /// Calls checked.
    pub calls: usize,
    /// Calls with a point of a kind this crate does not provide, skipped.
    pub unsupported: usize,
    /// Calls whose sign differs from the recorded one.
    pub mismatches: Mismatches,
}

impl BigAlone {
    /// Runs `big_integers_in` on the coordinates as big integers of the
    /// points of every `orient3d` call of `corpus`, and compares its sign
    /// with the recorded one.
    pub fn add_corpus(&mut self, corpus: &Corpus) {
        self.add_calls::<4>(
            corpus,
            Predicate::Orient3d,
            REFERENCE_ORIENT3D_FACTOR,
            |scratch, p| {
                let q: Vec<BigCoords> = p
                    .iter()
                    .map(|x| {
                        x.exact_big(scratch)
                            .unwrap_or_else(|e| panic!("no big coordinates: {e:?}, {x:?}"))
                    })
                    .collect();
                stages::big_integers_in(scratch, [&q[0], &q[1], &q[2], &q[3]])
                    .unwrap_or_else(|e| panic!("a growable arena is exhausted: {e:?}"))
            },
        );
    }

    /// [`BigAlone::add_corpus`] for the `insphere` calls of `corpus`.
    pub fn add_insphere_corpus(&mut self, corpus: &Corpus) {
        self.add_calls::<5>(
            corpus,
            Predicate::InSphere,
            REFERENCE_INSPHERE_FACTOR,
            |scratch, p| {
                insphere_chain::run_stage(insphere_chain::BIG_INTEGERS, scratch, p)
                    .unwrap_or_else(|| panic!("no big coordinates: {p:?}"))
            },
        );
    }

    /// Runs `big` on the points of every call of `predicate` in `corpus`,
    /// which have `N` points, and compares `factor` times its sign with the
    /// recorded one.
    fn add_calls<const N: usize>(
        &mut self,
        corpus: &Corpus,
        predicate: Predicate,
        factor: i8,
        big: impl Fn(&mut Scratch<'_>, &[AnyPoint; N]) -> Sign,
    ) {
        let mapped = points(corpus);
        let mut scratch = Scratch::growable(1 << 14);
        for (index, call) in corpus.calls.iter().enumerate() {
            if call.predicate != predicate {
                continue;
            }
            let Some(p) = call_points::<N>(&mapped, call) else {
                self.unsupported += 1;
                continue;
            };
            let mark = scratch.mark();
            let sign = big(&mut scratch, &p);
            scratch.reset(mark);
            self.calls += 1;
            if factor * i8::from(sign) != call.sign {
                self.mismatches.add(|| {
                    format!(
                        "{} call {index}: recorded {}, big_integers {sign:?}, points {p:?}",
                        corpus.model, call.sign
                    )
                });
            }
        }
    }

    /// Adds what another sweep found, as if its corpora had been added
    /// here after the ones already added.
    pub fn merge(&mut self, other: &BigAlone) {
        self.calls += other.calls;
        self.unsupported += other.unsupported;
        self.mismatches.merge(&other.mismatches);
    }
}
