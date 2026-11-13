//! The stages of `insphere`: their constants against the derivation of the
//! filter, the stages that are emitted, their sign convention, and their sign
//! on a few fixed calls against an oracle that does not come from the
//! definition, `implicit_predicates_corpus::exact::insphere`.

mod common;

use common::insphere_checks::check_bound_edge_cases;
use common::pow2;
use implicit_predicates::stages::insphere as stages;
use implicit_predicates::{Point, Scratch, Sign};
use implicit_predicates_corpus::exact;
use implicit_predicates_oracle::Dyadic;

/// The five points of a call.
type Points5 = [[f64; 3]; 5];

/// The points as the stages take them.
fn refs(p: &Points5) -> [&[f64; 3]; 5] {
    [&p[0], &p[1], &p[2], &p[3], &p[4]]
}

/// The sign of `insphere` at `p` by the independent oracle.
fn oracle(p: &Points5) -> Sign {
    let c = p.map(|q| q.map(Dyadic::from_f64));
    sign_of(exact::insphere(&c[0], &c[1], &c[2], &c[3], &c[4]))
}

/// The sign of `orient3d` at the first four points of `p` by the
/// independent oracle.
fn oracle_orientation(p: &Points5) -> Sign {
    let c = p.map(|q| q.map(Dyadic::from_f64));
    sign_of(exact::orient3d(&c[0], &c[1], &c[2], &c[3]))
}

fn sign_of(s: i8) -> Sign {
    match s {
        1 => Sign::Positive,
        -1 => Sign::Negative,
        _ => Sign::Zero,
    }
}

/// Whether `x` is at least `n * (1 + 2^-30)` and below
/// `n * (1 + 2^-30) * (1 + 2^-39)`, exactly: the exact value `n (1 + t)`,
/// `0 <= t < 2^-40`, inflated by `1 + 2^-30` and rounded up.
fn is_derived(x: f64, n: &Dyadic) -> bool {
    let inflated = n * &(&Dyadic::from_f64(1.0) + &Dyadic::from_f64(pow2(-30)));
    let above = &inflated * &(&Dyadic::from_f64(1.0) + &Dyadic::from_f64(pow2(-39)));
    let x = Dyadic::from_f64(x);
    inflated <= x && x < above
}

const SOURCE: &str = include_str!("../src/generated/insphere.rs");

#[test]
fn the_emitted_constants_are_those_of_the_derivation() {
    // The rules of the global bound give, for insphere, epsilon = 1122 u and
    // kappa = 360, plus terms of order u, each inflated by `1 + 2^-30` and
    // rounded up: the emitted values are 1122 u (1 + 2^-30) (1 + 5.48 u)
    // and 360 (1 + 2^-30) (1 + 4.27 u).
    assert_eq!(stages::DEGREE, 5);
    let u = Dyadic::from_f64(pow2(-53));
    assert!(is_derived(
        stages::EPSILON,
        &(&Dyadic::from_f64(1122.0) * &u)
    ));
    assert!(is_derived(stages::KAPPA, &Dyadic::from_f64(360.0)));
    assert_eq!(stages::EPSILON, 1.2456702347895472e-13);
    assert_eq!(stages::KAPPA, 360.0000003352763);
    assert_eq!(stages::D_MIN, pow2(-195));
    assert_eq!(stages::D_MAX, pow2(203));
    assert_eq!(stages::BIG_INTEGERS_EXPLICIT_ARENA, 6835);
    assert_eq!(stages::SPANS_TRANSLATED_RANGE.0, pow2(-108));
    assert_eq!(stages::SPANS_UNTRANSLATED_RANGE.0, pow2(-108));
}

#[test]
fn no_stage_on_the_stack_is_emitted() {
    // The exact differences need 4,154 doubles, over the limit of 4,096 of
    // the stack, and the expansions more: every exact stage takes an arena.
    for stage in [
        "exact_differences",
        "expansions_translated",
        "expansions_untranslated",
    ] {
        assert!(!SOURCE.contains(&format!("pub fn {stage}(")), "{stage}");
    }
    for stage in [
        "filter",
        "filter_points",
        "expansions_translated_in",
        "expansions_untranslated_in",
        "big_integers_in",
        "staged",
        "staged_in",
        "exact_stages",
        "exact_stages_in",
        "exact_stages_explicit_in",
    ] {
        assert!(SOURCE.contains(&format!("pub fn {stage}")), "{stage}");
    }
}

/// The six points of the unit sphere on the axes.
const X: [f64; 3] = [1.0, 0.0, 0.0];
const Y: [f64; 3] = [0.0, 1.0, 0.0];
const Z: [f64; 3] = [0.0, 0.0, 1.0];
const MX: [f64; 3] = [-1.0, 0.0, 0.0];
const MY: [f64; 3] = [0.0, -1.0, 0.0];
const MZ: [f64; 3] = [0.0, 0.0, -1.0];

/// Points of the unit sphere with `orient3d(X, Y, Z, MX)` positive: `MX`
/// lies on the side of the plane through `X`, `Y` and `Z` from which they
/// appear clockwise.
const POSITIVE: [[f64; 3]; 4] = [X, Y, Z, MX];

#[test]
fn the_sign_is_positive_inside_the_sphere_when_the_orientation_is_positive() {
    let centre = [0.0; 3];
    let p = [POSITIVE[0], POSITIVE[1], POSITIVE[2], POSITIVE[3], centre];
    assert_eq!(oracle_orientation(&p), Sign::Positive);
    assert_eq!(oracle(&p), Sign::Positive);
    assert_eq!(chain(&p), Sign::Positive);
    // With two points swapped the orientation is negative, and so is the
    // sign at the centre.
    let q = [POSITIVE[1], POSITIVE[0], POSITIVE[2], POSITIVE[3], centre];
    assert_eq!(oracle_orientation(&q), Sign::Negative);
    assert_eq!(oracle(&q), Sign::Negative);
    assert_eq!(chain(&q), Sign::Negative);
}

/// A dozen fixed calls: inside, outside and on the unit sphere through
/// points with integer coordinates, in both orientations, a point repeated,
/// a degenerate sphere, and a call off the origin and scaled.
fn cases() -> Vec<Points5> {
    let [a, b, c, d] = POSITIVE;
    let mut cases = vec![
        [a, b, c, d, [0.0; 3]],
        [a, b, c, d, [0.5, 0.25, -0.125]],
        [a, b, c, d, [2.0, 0.0, 0.0]],
        [a, b, c, d, [1.0, 1.0, 1.0]],
        [a, b, c, d, MY],
        [a, b, c, d, MZ],
        [b, a, c, d, [0.0; 3]],
        [b, a, c, d, [0.0, 3.0, -1.0]],
        [a, b, c, d, a],
        [a, a, c, d, [0.25; 3]],
        // Four coplanar points: the sphere degenerates to their plane.
        [X, Y, MX, MY, [0.0, 0.0, 0.5]],
        [X, Y, MX, MY, [0.25, 0.5, 0.0]],
    ];
    // The same calls moved off the origin and scaled by a power of two,
    // which does not change their signs.
    let moved: Vec<Points5> = cases
        .iter()
        .map(|p| p.map(|q| q.map(|x| 3.0 + x * pow2(-20))))
        .collect();
    cases.extend(moved);
    cases
}

/// The sign of the whole chain on explicit points, `staged_in`, with an
/// arena that suffices; checks that the arena is left as it was.
fn chain(p: &Points5) -> Sign {
    let mut buf = vec![0.0; 1 << 16];
    let mut scratch = Scratch::borrowed(&mut buf);
    let sign = stages::staged_in(&mut scratch, &p[0], &p[1], &p[2], &p[3], &p[4]).unwrap();
    assert_eq!(scratch.used(), 0);
    sign
}

#[test]
fn the_chain_has_the_sign_of_an_independent_oracle_on_a_few_calls() {
    let mut signs = [0; 3];
    for p in cases() {
        let expected = oracle(&p);
        signs[match expected {
            Sign::Negative => 0,
            Sign::Zero => 1,
            Sign::Positive => 2,
        }] += 1;
        let mut buf = vec![0.0; 1 << 16];
        let mut scratch = Scratch::borrowed(&mut buf);
        assert_eq!(chain(&p), expected, "staged_in {p:?}");
        let staged = stages::staged(&mut scratch, refs(&p)).unwrap();
        assert!(staged.is_none_or(|s| s == expected), "staged {p:?}");
        let explicit = stages::exact_stages_explicit_in(&mut scratch, refs(&p));
        assert_eq!(explicit, Ok(expected), "exact_stages_explicit_in {p:?}");
        if let Some(sign) = stages::filter(refs(&p), 0.0) {
            assert_eq!(sign, expected, "filter {p:?}");
        }
        // Each stage over spans, on the exact coordinates of the points.
        let q = p.map(|x| x.exact(&mut scratch).unwrap());
        let q = [&q[0], &q[1], &q[2], &q[3], &q[4]];
        for (name, sign) in [
            (
                "translated",
                stages::expansions_translated_in(&mut scratch, q),
            ),
            (
                "untranslated",
                stages::expansions_untranslated_in(&mut scratch, q),
            ),
        ] {
            assert_eq!(sign, Ok(Some(expected)), "{name} {p:?}");
        }
        // The stage on big integers, in the arena that suffices for
        // explicit points.
        let mut buf = vec![0.0; stages::BIG_INTEGERS_EXPLICIT_ARENA];
        let mut scratch = Scratch::borrowed(&mut buf);
        let q = p.map(|x| x.exact_big(&mut scratch).unwrap());
        let big = stages::big_integers_in(&mut scratch, [&q[0], &q[1], &q[2], &q[3], &q[4]]);
        assert_eq!(big, Ok(expected), "big_integers_in {p:?}");
    }
    // Every sign occurs.
    assert!(signs.iter().all(|&n| n >= 4), "{signs:?}");
}

#[test]
fn the_filter_and_the_chain_have_the_exact_sign_on_the_bound_edge_cases() {
    let [decided, declined] = check_bound_edge_cases(|p| stages::filter(refs(p), 0.0));
    assert!(decided > 0 && declined > 0, "{decided} {declined}");
    let mut buf = vec![0.0; 1 << 16];
    let mut scratch = Scratch::borrowed(&mut buf);
    let [decided, declined] = check_bound_edge_cases(|p| {
        Some(stages::staged_in(&mut scratch, &p[0], &p[1], &p[2], &p[3], &p[4]).unwrap())
    });
    assert_eq!(declined, 0);
    assert!(decided > 0);
}
