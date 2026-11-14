//! What the tests of `insphere` share: its sign by an oracle that does not
//! come from its definition, its stages in the order of the chain, by name,
//! and the stage that decides a call, with the reason why a call reaches the
//! stage on big integers.
//!
//! The oracle is `implicit_predicates_corpus::exact::insphere`, a cofactor
//! expansion written by hand on exact dyadic numbers: a sign that every
//! stage agrees on but that comes from a wrong definition still fails
//! against it.

use implicit_predicates::stages::insphere as stages;
use implicit_predicates::{AnyPoint, ExactError, Point, Scratch, Sign, Weight};
use implicit_predicates_corpus::exact;
use implicit_predicates_oracle::Dyadic;
pub use implicit_predicates_test_support::Points5;

use super::args::{Arg, Call5};
use super::why::{Why, why_exact_failed};
use super::{in_range, sign_of};

/// The points as the stages take them.
pub fn refs(p: &Points5) -> [&[f64; 3]; 5] {
    [&p[0], &p[1], &p[2], &p[3], &p[4]]
}

/// The sign of `insphere` at the points with the exact coordinates `c`, by
/// the independent oracle.
pub fn oracle_at(c: &[[Dyadic; 3]; 5]) -> Sign {
    sign_of(exact::insphere(&c[0], &c[1], &c[2], &c[3], &c[4]))
}

/// The sign of `insphere` at the explicit points `p`, by the independent
/// oracle.
///
/// # Panics
///
/// Panics if a coordinate is not finite.
pub fn oracle(p: &Points5) -> Sign {
    oracle_at(&p.map(|q| q.map(Dyadic::from_f64)))
}

/// The sign of `insphere` at the points of `args`, on their exact
/// coordinates, by the independent oracle.
///
/// # Panics
///
/// Panics if an input is not finite.
pub fn oracle_args(args: &[Arg; 5]) -> Sign {
    oracle_at(&args.each_ref().map(Arg::exact))
}

/// The stages of the chain of `insphere`, in the order in which `staged_in`
/// tries them, by name: the filter, on the approximations of the points
/// with their uncertainty, the stage over spans, on their exact
/// coordinates, and the stage on big integers, which decides every call
/// that the others do not. There are no stages on the stack: the points of
/// every type, explicit ones included, go through the stage over spans.
pub const CHAIN: [&str; 3] = ["filter", "expansions_translated_in", "big_integers"];

/// The index of the filter in [`CHAIN`].
pub const FILTER: usize = 0;

/// The index of the stage over spans in [`CHAIN`].
pub const SPANS: usize = 1;

/// The index of the stage on big integers in [`CHAIN`].
pub const BIG_INTEGERS: usize = 2;

/// The stage that decides a call first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decision {
    /// Its index in [`CHAIN`].
    pub stage: usize,
    /// Its sign.
    pub sign: Sign,
    /// Why no stage on expansions decides the call: `Some` exactly when
    /// `stage` is [`BIG_INTEGERS`].
    pub why: Option<Why>,
}

impl Decision {
    /// The name of the stage, from [`CHAIN`].
    pub fn name(&self) -> &'static str {
        CHAIN[self.stage]
    }
}

/// Ends a call of the tests whose arena runs out.
fn room<T>(e: impl std::fmt::Debug) -> T {
    panic!("the arena is too small: {e:?}")
}

/// Runs `f` in `scratch` and resets it to its mark on entry.
fn in_mark<T>(scratch: &mut Scratch<'_>, f: impl FnOnce(&mut Scratch<'_>) -> T) -> T {
    let mark = scratch.mark();
    let result = f(scratch);
    scratch.reset(mark);
    result
}

/// What `stage` of [`CHAIN`] alone gives on `p`, `None` when it does not
/// decide: `filter_points`; the stage over spans on the exact coordinates of
/// the points, which does not decide when a point cannot give them; or
/// `big_integers_in` on their coordinates as big integers, which does not
/// decide only when a point cannot give them, when an input is not finite.
/// `scratch` is left as it was.
///
/// # Panics
///
/// Panics if `scratch` runs out of room, or if `stage` is not an index of
/// [`CHAIN`].
pub fn run_stage<P: Point>(stage: usize, scratch: &mut Scratch<'_>, p: &[P; 5]) -> Option<Sign> {
    let [a, b, c, d, e] = p;
    in_mark(scratch, |scratch| match stage {
        FILTER => stages::filter_points(a, b, c, d, e),
        SPANS => {
            let q: Vec<_> = p
                .iter()
                .map(|x| x.exact(scratch).ok())
                .collect::<Option<_>>()?;
            let q = [&q[0], &q[1], &q[2], &q[3], &q[4]];
            stages::expansions_translated_in(scratch, q).unwrap_or_else(room)
        }
        BIG_INTEGERS => {
            let q: Vec<_> = p
                .iter()
                .map(|x| x.exact_big(scratch).ok())
                .collect::<Option<_>>()?;
            let q = [&q[0], &q[1], &q[2], &q[3], &q[4]];
            Some(stages::big_integers_in(scratch, q).unwrap_or_else(room))
        }
        _ => panic!("no stage {stage}"),
    })
}

/// The first stage of [`CHAIN`] that decides on `p`, and, when it is the
/// stage on big integers, why no stage on expansions decides it; `None`
/// only when an input is not finite. `scratch` is left as it was.
///
/// A call can have several reasons; the one given is the first of: a point
/// whose `Point::exact` fails ([`Why::ExactOutOfRange`], or
/// [`Why::Unexplained`] inside the range of its exact arithmetic), an
/// exact coordinate with too many components ([`Why::TooManyComponents`]),
/// a component out of range ([`Why::ComponentOutOfRange`]).
///
/// # Panics
///
/// Panics if `scratch` runs out of room, or if the exact coordinates of a
/// point have a weight other than one, as the chain does.
pub fn first_deciding(scratch: &mut Scratch<'_>, p: &[AnyPoint; 5]) -> Option<Decision> {
    first_deciding_with(scratch, p, why_exact_failed)
}

/// [`first_deciding`] on points of any one type, with `exact_failed`
/// giving the reason when `Point::exact` fails with
/// `ExactError::OutOfRange` on a point.
///
/// # Panics
///
/// As [`first_deciding`], and if `Point::exact` fails with another error.
pub fn first_deciding_with<P: Point>(
    scratch: &mut Scratch<'_>,
    p: &[P; 5],
    exact_failed: impl Fn(&P) -> Why,
) -> Option<Decision> {
    match deciding_on_expansions_with(scratch, p, exact_failed) {
        Ok(decision) => Some(decision),
        Err(why) => run_stage(BIG_INTEGERS, scratch, p).map(|sign| Decision {
            stage: BIG_INTEGERS,
            sign,
            why: Some(why),
        }),
    }
}

/// [`first_deciding`] without the stage on big integers: the first stage
/// on expansions that decides on `p`, or, when none does, why, with the
/// same precedence of reasons. `scratch` is left as it was.
///
/// # Panics
///
/// As [`first_deciding`].
pub fn deciding_on_expansions(
    scratch: &mut Scratch<'_>,
    p: &[AnyPoint; 5],
) -> Result<Decision, Why> {
    deciding_on_expansions_with(scratch, p, why_exact_failed)
}

/// [`deciding_on_expansions`] on points of any one type, with
/// `exact_failed` as in [`first_deciding_with`].
///
/// # Panics
///
/// As [`first_deciding_with`].
pub fn deciding_on_expansions_with<P: Point>(
    scratch: &mut Scratch<'_>,
    p: &[P; 5],
    exact_failed: impl Fn(&P) -> Why,
) -> Result<Decision, Why> {
    let decided = |stage, sign| Decision {
        stage,
        sign,
        why: None,
    };
    if let Some(sign) = run_stage(FILTER, scratch, p) {
        return Ok(decided(FILTER, sign));
    }
    in_mark(scratch, |scratch| {
        let mut q = Vec::with_capacity(5);
        for point in p {
            match point.exact(scratch) {
                Ok(c) => q.push(c),
                Err(ExactError::OutOfRange) => return Err(exact_failed(point)),
                Err(e) => panic!("`Point::exact` failed: {e:?}"),
            }
        }
        assert!(
            q.iter().all(|c| c.w == Weight::One),
            "a point has a weight other than one"
        );
        let r = [&q[0], &q[1], &q[2], &q[3], &q[4]];
        if let Some(sign) = stages::expansions_translated_in(scratch, r).unwrap_or_else(room) {
            return Ok(decided(SPANS, sign));
        }
        let spans = q.iter().flat_map(|c| [c.x, c.y, c.z]);
        Err(
            if spans
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
            },
        )
    })
}

/// `staged_in` on points of any type, in an arena of its own, large enough
/// for every call of the tests and kept from call to call; checks that the
/// arena is left as it was.
pub struct StagedIn(Vec<f64>);

impl StagedIn {
    /// The arena that suffices for every call of the tests, in `f64` slots.
    pub const ARENA: usize = 1 << 18;

    pub fn new() -> StagedIn {
        StagedIn(vec![0.0; Self::ARENA])
    }

    /// An arena on the same room, for other calls between those of
    /// `staged_in`.
    pub fn scratch(&mut self) -> Scratch<'_> {
        Scratch::borrowed(&mut self.0)
    }
}

impl Default for StagedIn {
    fn default() -> Self {
        StagedIn::new()
    }
}

impl Call5 for StagedIn {
    type Output = Sign;
    fn call<A: Point, B: Point, C: Point, D: Point, E: Point>(
        &mut self,
        a: &A,
        b: &B,
        c: &C,
        d: &D,
        e: &E,
    ) -> Sign {
        let mut scratch = Scratch::borrowed(&mut self.0);
        let result = stages::staged_in(&mut scratch, a, b, c, d, e);
        assert_eq!(scratch.used(), 0);
        result.expect("a large enough arena")
    }
}
