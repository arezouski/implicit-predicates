//! What the tests of `insphere` share: its sign by an oracle that does not
//! come from its definition, its stages in the order of the chain, by name,
//! and the stage that decides a call, with the reason why a call reaches the
//! stage on big integers, which for `insphere` is always the same.
//!
//! The oracle is `implicit_predicates_corpus::exact::insphere`, a cofactor
//! expansion written by hand on exact dyadic numbers: a sign that every
//! stage agrees on but that comes from a wrong definition still fails
//! against it.

use implicit_predicates::stages::insphere as stages;
use implicit_predicates::{Point, Scratch, Sign};
use implicit_predicates_corpus::exact;
use implicit_predicates_oracle::Dyadic;
pub use implicit_predicates_test_support::Points5;

use super::args::{Arg, Call5};
use super::sign_of;
use super::why::Why;

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
/// with their uncertainty, and the stage on big integers, which decides
/// every call that the filter does not. `insphere` is of degree five, so it
/// has no stage on expansions, for points of any type.
pub const CHAIN: [&str; 2] = ["filter", "big_integers"];

/// The index of the filter in [`CHAIN`].
pub const FILTER: usize = 0;

/// The index of the stage on big integers in [`CHAIN`].
pub const BIG_INTEGERS: usize = 1;

/// The stage that decides a call first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decision {
    /// Its index in [`CHAIN`].
    pub stage: usize,
    /// Its sign.
    pub sign: Sign,
    /// Why the stage on big integers decides the call: `Some` exactly when
    /// `stage` is [`BIG_INTEGERS`], and then always [`Why::BigIntegersOnly`].
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
/// decide: `filter_points`; or `big_integers_in` on the coordinates of the
/// points as big integers, which does not decide only when a point cannot
/// give them, when an input is not finite. `scratch` is left as it was.
///
/// # Panics
///
/// Panics if `scratch` runs out of room, or if `stage` is not an index of
/// [`CHAIN`].
pub fn run_stage<P: Point>(stage: usize, scratch: &mut Scratch<'_>, p: &[P; 5]) -> Option<Sign> {
    let [a, b, c, d, e] = p;
    in_mark(scratch, |scratch| match stage {
        FILTER => stages::filter_points(a, b, c, d, e),
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
/// stage on big integers, why: [`Why::BigIntegersOnly`]. `None` only when
/// an input is not finite. `scratch` is left as it was.
///
/// # Panics
///
/// Panics if `scratch` runs out of room.
pub fn first_deciding<P: Point>(scratch: &mut Scratch<'_>, p: &[P; 5]) -> Option<Decision> {
    match deciding_before_big_integers(scratch, p) {
        Ok(decision) => Some(decision),
        Err(why) => run_stage(BIG_INTEGERS, scratch, p).map(|sign| Decision {
            stage: BIG_INTEGERS,
            sign,
            why: Some(why),
        }),
    }
}

/// [`first_deciding`] without the stage on big integers: the filter, when it
/// decides on `p`, or, when it does not, why the stage on big integers
/// decides, [`Why::BigIntegersOnly`]: `insphere` has no stage on
/// expansions. `scratch` is left as it was.
pub fn deciding_before_big_integers<P: Point>(
    scratch: &mut Scratch<'_>,
    p: &[P; 5],
) -> Result<Decision, Why> {
    match run_stage(FILTER, scratch, p) {
        Some(sign) => Ok(Decision {
            stage: FILTER,
            sign,
            why: None,
        }),
        None => Err(Why::BigIntegersOnly),
    }
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
