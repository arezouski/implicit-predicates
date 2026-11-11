//! A sweep of the recorded `orient3d` calls of corpora: every call whose
//! points this crate provides gets the recorded sign, the public functions
//! agree with the first stage that decides, and the weight of the calls each
//! stage decides first is tallied by number of implicit points.

use std::fmt::Write as _;

use implicit_predicates::stages::orient3d as stages;
use implicit_predicates::{AnyPoint, ExactCoords, ExactError, Point, Scratch, Sign};
use implicit_predicates_corpus::exact::REFERENCE_ORIENT3D_FACTOR;
use implicit_predicates_corpus::weighted::weights;
use implicit_predicates_corpus::{Corpus, Predicate};

use super::corpus_points::{call_points, implicit_count, points};
use super::{CHAIN, all_in_range, first_deciding, in_range, refs};

/// The stages a call can be attributed to, in the order of the chain: the
/// filter giving a sign, the filter giving `Zero` (some axis is flat, so the
/// value is exactly zero), the exact stages for explicit points, the exact
/// stages over arena spans, and no stage.
pub const COLUMNS: [&str; 8] = [
    "filter",
    "certain zero",
    "exact_differences",
    "expansions_translated",
    "expansions_untranslated",
    "expansions_translated_in",
    "expansions_untranslated_in",
    "none",
];

const CERTAIN_ZERO: usize = 1;
const FIRST_SPANS_STAGE: usize = 5;
const NONE: usize = 7;

/// Why no stage decides a call.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Why {
    /// Explicit points with a coordinate outside `UNTRANSLATED_RANGE`.
    CoordinateOutOfRange,
    /// `Point::exact` failed with `ExactError::OutOfRange`.
    ExactOutOfRange,
    /// An exact coordinate has more than `MAX_COORD_COMPONENTS` components.
    TooManyComponents,
    /// A component of an exact coordinate is outside
    /// `SPANS_UNTRANSLATED_RANGE`.
    ComponentOutOfRange,
    /// None of the above: every input is inside the documented ranges, so
    /// some stage should have decided. A sweep counts it as a mismatch.
    Unexplained,
}

/// The first stage that decides a call, as an index in [`COLUMNS`], with
/// its sign; or [`NONE`] and the reason.
fn attribute(scratch: &mut Scratch<'_>, p: &[AnyPoint; 4]) -> (usize, Result<Sign, Why>) {
    let [a, b, c, d] = p;
    if let Some(sign) = stages::filter_points(a, b, c, d) {
        let column = if sign == Sign::Zero { CERTAIN_ZERO } else { 0 };
        return (column, Ok(sign));
    }
    if p.iter().all(Point::is_explicit) {
        let q = p.map(|x| *x.approx());
        // The filter of `CHAIN` has already declined.
        return CHAIN
            .iter()
            .enumerate()
            .skip(1)
            .find_map(|(i, (_, stage))| stage(refs(&q)).map(|sign| (i + 1, Ok(sign))))
            .unwrap_or_else(|| {
                // `expansions_untranslated` decides whenever every coordinate
                // is in its range.
                let why = if all_in_range(&q, stages::UNTRANSLATED_RANGE) {
                    Why::Unexplained
                } else {
                    Why::CoordinateOutOfRange
                };
                (NONE, Err(why))
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
            Err(ExactError::OutOfRange) => return (NONE, Err(Why::ExactOutOfRange)),
            Err(ExactError::Exhausted(e)) => panic!("a growable arena is exhausted: {e:?}"),
        }
    }
    let q = q.map(|c| c.expect("every point is written"));
    let r = [&q[0], &q[1], &q[2], &q[3]];
    let exhausted = |e| panic!("a growable arena is exhausted: {e:?}");
    if let Some(sign) = stages::expansions_translated_in(scratch, r).unwrap_or_else(exhausted) {
        return (FIRST_SPANS_STAGE, Ok(sign));
    }
    if let Some(sign) = stages::expansions_untranslated_in(scratch, r).unwrap_or_else(exhausted) {
        return (FIRST_SPANS_STAGE + 1, Ok(sign));
    }
    let spans = q.iter().flat_map(|c| [c.x, c.y, c.z]);
    let why = if spans
        .clone()
        .any(|s| s.len() > stages::MAX_COORD_COMPONENTS)
    {
        Why::TooManyComponents
    } else if spans
        .flat_map(|s| scratch.get(s))
        .any(|&x| !in_range(x, stages::SPANS_UNTRANSLATED_RANGE))
    {
        Why::ComponentOutOfRange
    } else {
        Why::Unexplained
    };
    (NONE, Err(why))
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
        Some((i, _)) => i + 1,
        None => NONE,
    };
    let every_stage_right = CHAIN
        .iter()
        .all(|(_, stage)| !stage(refs(&q)).is_some_and(&wrong));
    column == expected && every_stage_right
}

/// The weighted tally of the calls with a given number of implicit points.
#[derive(Clone, Debug, Default)]
pub struct Bucket {
    /// Stored calls checked.
    pub stored: usize,
    /// Their weight: the calls of the recording runs they stand for.
    pub weight: f64,
    /// The weight of the calls each column of [`COLUMNS`] decides first.
    pub columns: [f64; COLUMNS.len()],
    /// The weight of the calls whose recorded sign is zero.
    pub recorded_zero: f64,
    /// Stored calls with a point of a kind this crate does not provide.
    pub unsupported: usize,
    /// Stored calls checked with a point on a line whose parameter is
    /// outside `[0, 1]`.
    pub unchecked: usize,
}

impl Bucket {
    fn add(&mut self, other: &Bucket) {
        self.stored += other.stored;
        self.weight += other.weight;
        for (a, b) in self.columns.iter_mut().zip(other.columns) {
            *a += b;
        }
        self.recorded_zero += other.recorded_zero;
        self.unsupported += other.unsupported;
        self.unchecked += other.unchecked;
    }
}

/// What a sweep found, by number of implicit points (0 to 4).
#[derive(Debug)]
pub struct Tally {
    pub buckets: [Bucket; 5],
    /// The weight and stored count of the undecided calls, by reason.
    pub undecided: Vec<(Why, usize, f64)>,
    /// Up to a few undecided calls of each reason, described.
    pub examples: Vec<(Why, String)>,
    /// Calls whose sign differs from the recorded one, or on which the
    /// public functions disagree with the stages.
    pub mismatch_count: usize,
    /// The first [`MISMATCHES`] of them, described.
    pub mismatches: Vec<String>,
    /// Points of the corpora mapped to `None` or built unchecked.
    pub points_unsupported: usize,
    pub points_non_finite: usize,
    pub points_unchecked: usize,
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
            undecided: Vec::new(),
            examples: Vec::new(),
            mismatch_count: 0,
            mismatches: Vec::new(),
            points_unsupported: 0,
            points_non_finite: 0,
            points_unchecked: 0,
            high_water: [0; 5],
            arenas: std::array::from_fn(|_| Scratch::growable(1 << 14)),
            scratch: Scratch::growable(1 << 14),
        }
    }
}

/// The most examples kept of each reason for an undecided call.
const EXAMPLES: usize = 5;

/// The most mismatches kept described.
const MISMATCHES: usize = 20;

impl Tally {
    /// Checks and tallies every `orient3d` call of `corpus`; returns the
    /// number of calls checked without and with implicit points.
    pub fn add_corpus(&mut self, corpus: &Corpus) -> (usize, usize) {
        let mapped = points(corpus);
        self.points_unsupported += mapped.unsupported;
        self.points_non_finite += mapped.non_finite;
        self.points_unchecked += mapped.unchecked;
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
        let staged = result.ok();
        let wrong = |s: Sign| REFERENCE_ORIENT3D_FACTOR * i8::from(s) != recorded;
        let unexplained = result == Err(Why::Unexplained);
        if staged.is_some_and(wrong)
            || unexplained
            || public != staged
            || in_arena != staged
            || arena.used() != 0
            || (p.iter().all(Point::is_explicit) && !explicit_chain_agrees(p, column, wrong))
        {
            self.mismatch_count += 1;
            if self.mismatches.len() < MISMATCHES {
                self.mismatches.push(describe());
            }
        }
        let bucket = &mut self.buckets[implicit];
        bucket.stored += 1;
        bucket.weight += weight;
        bucket.columns[column] += weight;
        bucket.recorded_zero += if recorded == 0 { weight } else { 0.0 };
        bucket.unchecked += usize::from(
            p.iter()
                .any(|q| matches!(q, AnyPoint::Lnc(l) if !(0.0..=1.0).contains(&l.t()))),
        );
        if let Err(why) = result {
            match self.undecided.iter_mut().find(|(w, ..)| *w == why) {
                Some(entry) => {
                    entry.1 += 1;
                    entry.2 += weight;
                }
                None => self.undecided.push((why, 1, weight)),
            }
            if self.examples.iter().filter(|(w, _)| *w == why).count() < EXAMPLES {
                self.examples.push((why, describe()));
            }
        }
    }

    /// The tally of the calls with 1 to 4 implicit points.
    pub fn implicit(&self) -> Bucket {
        let mut all = Bucket::default();
        for bucket in &self.buckets[1..] {
            all.add(bucket);
        }
        all
    }

    /// The tally of every call.
    pub fn all(&self) -> Bucket {
        let mut all = self.buckets[0].clone();
        all.add(&self.implicit());
        all
    }

    /// The table of weighted fractions, one row per number of implicit
    /// points, then the counts and the undecided calls.
    pub fn report(&self, name: &str) -> String {
        let mut out = String::new();
        let mut line = |s: String| writeln!(out, "{s}").expect("writing to a string");
        line(format!("\n## {name}\n"));
        let header: Vec<_> = COLUMNS.iter().map(|c| format!("{c} %")).collect();
        line(format!(
            "| implicit | stored | weighted | recorded zero % | {} | unsupported | t outside [0, 1] | high water |",
            header.join(" | ")
        ));
        line(format!("|{}", " --- |".repeat(COLUMNS.len() + 7)));
        let row = |label: &str, b: &Bucket, high: String| {
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
        for (k, bucket) in self.buckets.iter().enumerate() {
            line(row(&k.to_string(), bucket, self.high_water[k].to_string()));
        }
        let max = |s: &[usize]| s.iter().copied().max().unwrap_or(0).to_string();
        line(row("1-4", &self.implicit(), max(&self.high_water[1..])));
        line(row("all", &self.all(), max(&self.high_water)));
        line(format!(
            "\npoints: {} unsupported, {} not finite, {} with t outside [0, 1]",
            self.points_unsupported, self.points_non_finite, self.points_unchecked
        ));
        line(format!("mismatches: {}", self.mismatch_count));
        for m in &self.mismatches {
            line(format!("  {m}"));
        }
        let implicit_weight = self.implicit().weight;
        let mut undecided = self.undecided.clone();
        undecided.sort_by_key(|&(why, ..)| why);
        line(format!("undecided: {}", undecided.len()));
        for (why, stored, weight) in &undecided {
            line(format!(
                "  {why:?}: {stored} stored, weight {weight:.1}, {:.6} % of the weight with implicit points",
                100.0 * weight / implicit_weight
            ));
        }
        for (why, example) in &self.examples {
            line(format!("  {why:?} example: {example}"));
        }
        out
    }
}
