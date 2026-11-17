//! What the tests of the predicates share: for `orient3d`, the generated
//! checks, among them the exact sign `generated::exact`, the stages by name,
//! and the condition under which the chain is documented to decide; for
//! `insphere`, its stages by name and the stage that decides a call
//! ([`insphere`]); for both, the arguments of a call with implicit points
//! ([`args`]), why a call reaches the stage on big integers ([`why`]),
//! points of types made for the tests ([`odd_points`]) and a few helpers.
//!
//! The generated checks of `insphere` are not here: only its own tests
//! compile them.

// Each test binary uses a different part of this module and of the
// generated checks.
#![allow(dead_code)]

#[path = "../generated/orient3d.rs"]
pub mod generated;

#[path = "../generated/points.rs"]
pub mod point_checks;

pub mod args;
pub mod boxes;
pub mod bpt;
pub mod corpus_points;
#[cfg(feature = "std")]
pub mod corpus_sweep;
pub mod insphere;
pub mod odd_points;
pub mod why;

use implicit_predicates::stages::orient3d as stages;
use implicit_predicates::{Point, Scratch, Sign};
use implicit_predicates_oracle::Dyadic;
pub use implicit_predicates_test_support::Points4 as Points;

/// A stage of the chain, as a function of the four points.
pub type Stage = fn([&[f64; 3]; 4]) -> Option<Sign>;

/// A stage of the generated module with its name, written once so that the
/// name cannot be that of another function.
macro_rules! stage {
    ($name:ident) => {
        (stringify!($name), stages::$name as Stage)
    };
}

/// The filter for explicit points, under the name of the generated function.
const FILTER: (&str, Stage) = ("filter", |p| stages::filter(p, [0.0; 3]));

/// Every stage of the chain with its name, the filter for explicit points;
/// then the chain itself.
pub const STAGES: [(&str, Stage); 4] = [
    FILTER,
    stage!(exact_differences),
    stage!(expansions_translated),
    stage!(staged),
];

/// The stages of the chain for explicit points, in the order `orient3d`
/// tries them, by name: those of `staged`, then the stage on big integers.
/// The tests check that the first of them that decides gives the result of
/// `orient3d`, so this list cannot drift from the generated chain.
pub const CHAIN: [(&str, Stage); 4] = [
    FILTER,
    stage!(exact_differences),
    stage!(expansions_translated),
    ("big_integers", big_integers),
];

/// The number of stages of [`CHAIN`] that `staged` runs: all but the stage
/// on big integers.
pub const STAGED: usize = 3;

/// `big_integers_in` on the coordinates of the points as big integers,
/// written by `exact_big` to an arena of
/// [`stages::BIG_INTEGERS_EXPLICIT_ARENA`] slots, which suffice for
/// explicit points; `None` only when a coordinate is not finite, which
/// `exact_big` declines.
pub fn big_integers(p: [&[f64; 3]; 4]) -> Option<Sign> {
    let mut buf = vec![0.0; stages::BIG_INTEGERS_EXPLICIT_ARENA];
    let mut scratch = Scratch::borrowed(&mut buf);
    let mut q = Vec::with_capacity(4);
    for x in p {
        q.push(x.exact_big(&mut scratch).ok()?);
    }
    let sign = stages::big_integers_in(&mut scratch, [&q[0], &q[1], &q[2], &q[3]]);
    Some(sign.expect("the arena of the stage for explicit points"))
}

/// The index in [`CHAIN`] of the first stage that decides, and its result;
/// `None` only when an input is not finite.
pub fn first_deciding(p: &Points) -> Option<(usize, Sign)> {
    CHAIN
        .iter()
        .enumerate()
        .find_map(|(i, (_, stage))| stage(refs(p)).map(|sign| (i, sign)))
}

/// The points as the stages take them.
pub fn refs(p: &Points) -> [&[f64; 3]; 4] {
    [&p[0], &p[1], &p[2], &p[3]]
}

/// `orient3d` on the four points.
#[cfg(feature = "std")]
pub fn orient3d(p: &Points) -> Sign {
    implicit_predicates::orient3d(&p[0], &p[1], &p[2], &p[3])
}

/// The condition of the range checks of the generated stages: `x` is zero
/// or has a magnitude in `min..=max`, which also rules out NaN and
/// infinities.
pub fn in_range(x: f64, (min, max): (f64, f64)) -> bool {
    x == 0.0 || (min..=max).contains(&x.abs())
}

/// Whether every coordinate is in `range`, as [`in_range`] defines it.
pub fn all_in_range(p: &Points, range: (f64, f64)) -> bool {
    p.iter().flatten().all(|&x| in_range(x, range))
}

/// Whether every coordinate is finite.
pub fn all_finite(p: &Points) -> bool {
    p.iter().flatten().all(|x| x.is_finite())
}

/// The ends of the ranges of the exact stages and of the filter, for
/// generators of values just inside and just outside each of them.
pub fn edges() -> Vec<f64> {
    let (d, t) = (stages::DIFFERENCES_RANGE, stages::TRANSLATED_RANGE);
    vec![d.0, d.1, t.0, t.1, stages::D_MIN, stages::D_MAX]
}

/// `2^exponent`, for `exponent` in `-1074..=1023`, built from its bits.
pub fn pow2(exponent: i32) -> f64 {
    assert!((-1074..=1023).contains(&exponent), "2^{exponent}");
    if exponent >= -1022 {
        f64::from_bits(u64::try_from(exponent + 1023).unwrap() << 52)
    } else {
        f64::from_bits(1 << (exponent + 1074))
    }
}

/// The sign of `s`, a sign given as `-1`, `0` or `1` as the oracles of
/// `implicit_predicates_corpus::exact` give it.
///
/// # Panics
///
/// Panics if `s` is not one of them.
pub fn sign_of(s: i8) -> Sign {
    match s {
        1 => Sign::Positive,
        0 => Sign::Zero,
        -1 => Sign::Negative,
        _ => panic!("not a sign: {s}"),
    }
}

/// Whether `x` is at least `n * (1 + 2^-30)` and below
/// `n * (1 + 2^-30) * (1 + 2^-39)`, exactly: the exact value `n (1 + t)`,
/// `0 <= t < 2^-40`, inflated by `1 + 2^-30` and rounded up, as the
/// generator emits the constants of a filter.
pub fn is_derived(x: f64, n: &Dyadic) -> bool {
    let inflated = n * &(&Dyadic::from_f64(1.0) + &Dyadic::from_f64(pow2(-30)));
    let above = &inflated * &(&Dyadic::from_f64(1.0) + &Dyadic::from_f64(pow2(-39)));
    let x = Dyadic::from_f64(x);
    inflated <= x && x < above
}
