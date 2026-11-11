//! What the tests of `orient3d` share: the generated checks, among them the
//! exact sign `generated::exact`, the stages by name, and the condition
//! under which the chain is documented to decide.

// Each test binary uses a different part of this module and of the
// generated checks.
#![allow(dead_code)]

#[path = "../generated/orient3d.rs"]
pub mod generated;

pub mod boxes;

use indirect_predicates::Sign;
use indirect_predicates::stages::orient3d as stages;
pub use indirect_predicates_test_support::Points4 as Points;

/// A stage of the chain, as a function of the four points.
pub type Stage = fn([&[f64; 3]; 4]) -> Option<Sign>;

/// Every stage of the chain with its name, the filter for explicit points;
/// then the chain itself.
pub const STAGES: [(&str, Stage); 5] = [
    ("filter", |p| stages::filter(p, 0.0)),
    ("exact_differences", stages::exact_differences),
    ("expansions_translated", stages::expansions_translated),
    ("expansions_untranslated", stages::expansions_untranslated),
    ("staged", stages::staged),
];

/// The stages of the chain for explicit points, in the order `staged`
/// tries them, by name. The tests check that the first of them that decides
/// gives the result of `staged`, so this list cannot drift from the
/// generated chain.
pub const CHAIN: [(&str, Stage); 4] = [
    ("filter", |p| stages::filter(p, 0.0)),
    ("exact_differences", stages::exact_differences),
    ("expansions_translated", stages::expansions_translated),
    ("expansions_untranslated", stages::expansions_untranslated),
];

/// The index in [`CHAIN`] of the first stage that decides, and its result;
/// `None` when no stage decides.
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
pub fn orient3d(p: &Points) -> Option<Sign> {
    indirect_predicates::orient3d(&p[0], &p[1], &p[2], &p[3])
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
    let (d, t, u) = (
        stages::DIFFERENCES_RANGE,
        stages::TRANSLATED_RANGE,
        stages::UNTRANSLATED_RANGE,
    );
    vec![d.0, d.1, t.0, t.1, u.0, u.1, stages::D_MIN, stages::D_MAX]
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
