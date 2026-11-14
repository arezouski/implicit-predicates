//! The stages of `insphere` against an oracle that does not come from the
//! definition, `implicit_predicates_corpus::exact::insphere`, on the exact
//! coordinates of the points: their constants against the derivation of
//! the filter and of its second test, `filter_axes`, the stages that are
//! emitted, the filter and the stage on big integers, their sign convention,
//! a few fixed calls, random, grid, cospherical and nearly cospherical points
//! of every type, the whole range of `f64`, the hand-over from the range of
//! the filter, the filter with an uncertainty, the edge cases of the
//! generated checks, the range of `filter_axes` on each axis and what its
//! uncertainty can change, the panics of the chain and the arena of the
//! stage on big integers.

mod common;

#[path = "generated/insphere.rs"]
#[allow(dead_code)]
mod insphere_checks;

use common::args::{Arg, Call5, any_points, with_concrete5};
use common::insphere::{
    BIG_INTEGERS, CHAIN, Decision, Points5, StagedIn, first_deciding, oracle, oracle_args,
    oracle_at, refs, run_stage,
};
use common::odd_points::{Long, NoCoordinates, NoExpansions, Weighted};
use common::why::Why;
use common::{is_derived, pow2, sign_of};
use implicit_predicates::stages::insphere::{self as stages, BIG_INTEGERS_EXPLICIT_ARENA};
use implicit_predicates::{AnyPoint, Explicit, Lnc, Point, Scratch, Sign};
use implicit_predicates_core::filter::uncertainty_max;
use implicit_predicates_corpus::exact;
use implicit_predicates_oracle::Dyadic;
use implicit_predicates_test_support::{
    config, cospherical5, extreme_value, grid5, lnc_parameter, lnc_params, near_cospherical5,
    random5, sample,
};
use insphere_checks::{
    axes_edge_cases, bound_edge_cases, check_axes_edge_cases, check_bound_edge_cases, exact_at,
};
use proptest::prelude::*;

/// The five arguments of a call.
type Args5 = [Arg; 5];

/// The sign of `orient3d` at the first four points of `p` by the
/// independent oracle.
fn oracle_orientation(p: &Points5) -> Sign {
    let c = p.map(|q| q.map(Dyadic::from_f64));
    sign_of(exact::orient3d(&c[0], &c[1], &c[2], &c[3]))
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
}

/// The degree vectors of the terms of `insphere`, in increasing order.
const VECTORS: [[u32; 3]; 3] = [[1, 1, 3], [1, 3, 1], [3, 1, 1]];

#[test]
fn the_emitted_constants_by_parts_are_those_of_the_derivation() {
    // The rules of the bound by parts give, for insphere, the terms of
    // epsilon 358 u D^(1, 1, 3), 382 u D^(1, 3, 1) and 382 u D^(3, 1, 1), and
    // on each axis k, for each vector g, a term of kappa E_k D^(g - e_k) of
    // 72 when g_k = 3 and 24 otherwise, plus terms of order u, each inflated
    // by `1 + 2^-30` and rounded up.
    let u = Dyadic::from_f64(pow2(-53));
    let epsilon = [358.0, 382.0, 382.0];
    assert_eq!(stages::AXES_EPSILON.map(|(g, _)| g), VECTORS);
    for ((g, value), n) in stages::AXES_EPSILON.into_iter().zip(epsilon) {
        assert!(is_derived(value, &(&Dyadic::from_f64(n) * &u)), "{g:?}");
    }
    let mut kappa = Vec::new();
    for axis in 0..3 {
        let mut terms: Vec<([u32; 3], f64)> = VECTORS
            .iter()
            .map(|&g| {
                let mut rest = g;
                rest[axis] -= 1;
                (rest, if g[axis] == 3 { 72.0 } else { 24.0 })
            })
            .collect();
        terms.sort_by_key(|&(rest, _)| rest);
        kappa.extend(terms.into_iter().map(|(rest, n)| (axis, rest, n)));
    }
    assert_eq!(stages::AXES_KAPPA.len(), 9);
    for ((axis, rest, value), (k, h, n)) in stages::AXES_KAPPA.into_iter().zip(kappa) {
        assert_eq!((axis, rest), (k, h));
        assert!(is_derived(value, &Dyadic::from_f64(n)), "{axis} {rest:?}");
    }
    // The range of the global form, here: its `d_min` binds with the
    // smallest term of epsilon, 358 u, as it does with 1122 u.
    assert_eq!(stages::AXES_D_MIN, pow2(-195));
    assert_eq!(stages::AXES_D_MAX, pow2(203));
    // With every size the same, the terms add up to the global bound, up to
    // the rounding of each constant: a few units in the last place.
    let sum = |values: &mut dyn Iterator<Item = f64>| {
        values.fold(Dyadic::from_f64(0.0), |s, x| &s + &Dyadic::from_f64(x))
    };
    let ulps = |x: f64| Dyadic::from_f64(4.0 * (x.next_up() - x));
    let around = |total: &Dyadic, x: f64| {
        let x_exact = Dyadic::from_f64(x);
        &x_exact - &ulps(x) <= *total && *total <= &x_exact + &ulps(x)
    };
    let epsilon = sum(&mut stages::AXES_EPSILON.iter().map(|t| t.1));
    assert!(around(&epsilon, stages::EPSILON));
    let kappa = sum(&mut stages::AXES_KAPPA.iter().map(|t| t.2));
    assert!(around(&kappa, stages::KAPPA));
}

#[test]
fn only_the_filters_and_the_stage_on_big_integers_are_emitted() {
    // Of degree 5, `insphere` has no stage on expansions, on the stack or
    // over spans, nor their ranges and helpers.
    for absent in [
        "pub fn exact_differences(",
        "pub fn expansions_translated(",
        "pub fn expansions_translated_in(",
        "pub fn exact_stages(",
        "fn staged_in_arena(",
        "fn exact(",
        "fn span_in_range(",
        "RANGE",
        "MAX_COORD_COMPONENTS",
    ] {
        assert!(!SOURCE.contains(absent), "{absent}");
    }
    for stage in [
        "filter",
        "filter_points",
        "filter_axes",
        "filter_axes_points",
        "big_integers_in",
        "staged",
        "staged_in",
        "exact_stages_in",
        "exact_stages_explicit_in",
    ] {
        assert!(SOURCE.contains(&format!("pub fn {stage}")), "{stage}");
    }
    // `staged` is the two tests of the filter, and takes no arena.
    assert!(SOURCE.contains(
        "pub fn staged(p: [&[f64; 3]; 5]) -> Option<Sign> {\n    \
         filter(p, 0.0).or_else(|| filter_axes(p, [0.0; 3]))\n}"
    ));
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
        let staged = stages::staged(refs(&p));
        assert!(staged.is_none_or(|s| s == expected), "staged {p:?}");
        let explicit = stages::exact_stages_explicit_in(&mut scratch, refs(&p));
        assert_eq!(explicit, Ok(expected), "exact_stages_explicit_in {p:?}");
        if let Some(sign) = stages::filter(refs(&p), 0.0) {
            assert_eq!(sign, expected, "filter {p:?}");
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

/// The chain has no stage on expansions: it never asks a point for its
/// exact coordinates as expansions, only as big integers, whether the
/// points are explicit or not, on calls that the filter decides and on
/// calls that it does not.
#[test]
fn the_chain_never_asks_for_exact_coordinates_as_expansions() {
    let mut declined = 0;
    for p in cases() {
        let expected = oracle(&p);
        let mut buf = vec![0.0; 1 << 16];
        let mut scratch = Scratch::borrowed(&mut buf);
        for explicit in [false, true] {
            let [a, b, c, d, e] = p.map(|point| NoExpansions { point, explicit });
            if stages::filter_points(&a, &b, &c, &d, &e).is_none() {
                declined += 1;
            }
            let sign = stages::staged_in(&mut scratch, &a, &b, &c, &d, &e);
            assert_eq!(sign, Ok(expected), "staged_in {p:?}");
            let sign = stages::exact_stages_in(&mut scratch, &a, &b, &c, &d, &e);
            assert_eq!(sign, Ok(expected), "exact_stages_in {p:?}");
        }
        let sign = stages::exact_stages_explicit_in(&mut scratch, refs(&p));
        assert_eq!(sign, Ok(expected), "exact_stages_explicit_in {p:?}");
        assert_eq!(scratch.used(), 0);
    }
    assert!(declined > 0, "the filter decides every call");
}

/// The points of `args` when they are all explicit.
fn explicit(args: &Args5) -> Option<Points5> {
    let mut p = [[0.0; 3]; 5];
    for (point, arg) in p.iter_mut().zip(args) {
        match arg {
            Arg::Explicit(x) => *point = *x,
            Arg::Lnc(..) => return None,
        }
    }
    Some(p)
}

/// Asserts that every way of running the chain of `insphere`, or one of its
/// stages, on `args` gives `expected` when it decides: `staged_in` on the
/// concrete types, as `[f64; 3]` or `Explicit` and `Lnc`, and on
/// `AnyPoint`s, `exact_stages_in`, each stage of [`CHAIN`] alone, and, when
/// every point is explicit, `filter`, `staged` and
/// `exact_stages_explicit_in`. The generated `exact_at` must agree too, in
/// addition to the oracle. Returns the stage that decides first.
fn assert_every_call(args: &Args5, expected: Sign, staged: &mut StagedIn) -> Decision {
    for raw in [false, true] {
        assert_eq!(with_concrete5(args, raw, staged), expected, "{args:?}");
    }
    let points = any_points(args);
    let [a, b, c, d, e] = &points;
    assert_eq!(staged.call(a, b, c, d, e), expected, "{args:?}");
    let mut scratch = staged.scratch();
    let exact_stages = stages::exact_stages_in(&mut scratch, a, b, c, d, e);
    assert_eq!(exact_stages, Ok(expected), "exact_stages_in {args:?}");
    for (i, name) in CHAIN.iter().enumerate() {
        if let Some(sign) = run_stage(i, &mut scratch, &points) {
            assert_eq!(sign, expected, "{name} {args:?}");
        }
    }
    let decision = first_deciding(&mut scratch, &points).expect("finite inputs");
    assert_eq!(decision.sign, expected, "{decision:?} {args:?}");
    let why = (decision.stage == BIG_INTEGERS).then_some(Why::BigIntegersOnly);
    assert_eq!(decision.why, why, "{args:?}");
    if let Some(p) = explicit(args) {
        let r = refs(&p);
        if let Some(sign) = stages::filter(r, 0.0) {
            assert_eq!(sign, expected, "filter {args:?}");
        }
        let decided = stages::staged(r);
        assert!(decided.is_none_or(|s| s == expected), "staged {args:?}");
        let sign = stages::exact_stages_explicit_in(&mut scratch, r);
        assert_eq!(sign, Ok(expected), "exact_stages_explicit_in {args:?}");
        for (i, name) in CHAIN.iter().enumerate() {
            if let Some(sign) = run_stage(i, &mut scratch, &p) {
                assert_eq!(sign, expected, "{name} on arrays {args:?}");
            }
        }
    }
    assert_eq!(scratch.used(), 0);
    let definition = exact_at(&args.each_ref().map(Arg::exact));
    assert_eq!(definition, expected, "exact_at {args:?}");
    decision
}

/// `x` with each coordinate moved by `steps[k]` units in the last place.
fn stepped(x: [f64; 3], steps: [i32; 3]) -> [f64; 3] {
    std::array::from_fn(|k| {
        let step = if steps[k] >= 0 {
            f64::next_up
        } else {
            f64::next_down
        };
        (0..steps[k].unsigned_abs()).fold(x[k], |y, _| step(y))
    })
}

/// A point on a line to stand for the point `x`: one whose parents are both
/// `x`, so that its exact value is `x`; one between `x` and `x` moved by a
/// few units in the last place, so that it is near `x` with coordinates of
/// several components; or any point on a line from `lnc_params`.
fn line_for(x: [f64; 3]) -> BoxedStrategy<Arg> {
    let t = || 0.0..=1.0f64;
    let steps = || [-3i32..=3, -3i32..=3, -3i32..=3];
    prop_oneof![
        t().prop_map(move |t| Arg::Lnc(x, x, t)),
        (steps(), t()).prop_map(move |(s, t)| Arg::Lnc(x, stepped(x, s), t)),
        lnc_params().prop_map(|(p, q, t)| Arg::Lnc(p, q, t)),
    ]
    // Boxed, so that the value trees of the five points stay small.
    .boxed()
}

/// The points `p` with none to five of them, at any positions, replaced by
/// points on a line from [`line_for`].
fn with_lines(p: Points5) -> impl Strategy<Value = Args5> {
    (
        prop::sample::subsequence(vec![0usize, 1, 2, 3, 4], 0..=5),
        [
            line_for(p[0]),
            line_for(p[1]),
            line_for(p[2]),
            line_for(p[3]),
            line_for(p[4]),
        ],
    )
        .prop_map(move |(at, lines)| {
            std::array::from_fn(|i| {
                if at.contains(&i) {
                    lines[i]
                } else {
                    Arg::Explicit(p[i])
                }
            })
        })
}

/// Calls on random points, points of a grid of small integers and points
/// of which four lie on a sphere and one is a few units in the last place
/// off it, with points on a line among them.
fn calls() -> impl Strategy<Value = Args5> {
    prop_oneof![random5(), grid5(), near_cospherical5()].prop_flat_map(with_lines)
}

/// A point on a line whose exact value is `p[i]`, or `p[j]` for another
/// point `j` of the call, by `kind`: parents both `p[i]`; `p[i]` at `t =
/// 1`; `p[i]` at `t = 0`; the midpoint of `2 p[i] - p[j]` and `p[j]`; and
/// `p[j]` at `t = 1`, which repeats it. On the grid of [`cospherical5`]
/// every input is exact.
fn line_at(p: &Points5, i: usize, j: usize, kind: usize, t: f64) -> Arg {
    let (x, y) = (p[i], p[j]);
    match kind {
        0 => Arg::Lnc(x, x, t),
        1 => Arg::Lnc(x, y, 1.0),
        2 => Arg::Lnc(y, x, 0.0),
        3 => Arg::Lnc(std::array::from_fn(|k| 2.0 * x[k] - y[k]), y, 0.5),
        _ => Arg::Lnc(y, x, 1.0),
    }
}

/// Calls whose exact points lie on one sphere: from [`cospherical5`], with
/// a point repeated in half of them, and none to five points replaced by
/// points on a line from [`line_at`].
fn cospherical_args() -> impl Strategy<Value = Args5> {
    let line = || (0usize..5, 1usize..5, 0.0..=1.0f64);
    (
        cospherical5(),
        prop::option::of((0usize..5, 1usize..5)),
        prop::sample::subsequence(vec![0usize, 1, 2, 3, 4], 0..=5),
        [line(), line(), line(), line(), line()],
    )
        .prop_map(|(mut p, repeated, at, lines)| {
            if let Some((i, k)) = repeated {
                p[(i + k) % 5] = p[i];
            }
            std::array::from_fn(|i| {
                let (kind, k, t) = lines[i];
                if at.contains(&i) {
                    line_at(&p, i, (i + k) % 5, kind, t)
                } else {
                    Arg::Explicit(p[i])
                }
            })
        })
}

/// Five arguments whose inputs are values of one family of
/// [`extreme_value`], with points on a line, in any number from none to
/// five, at any positions; the others are explicit.
fn extreme_args() -> impl Strategy<Value = Args5> {
    (0usize..5).prop_flat_map(|family| {
        let v = move || extreme_value(family);
        let point = move || [v(), v(), v()];
        let line = move || (point(), point(), lnc_parameter());
        (
            [point(), point(), point(), point(), point()],
            [line(), line(), line(), line(), line()],
            prop::sample::subsequence(vec![0usize, 1, 2, 3, 4], 0..=5),
        )
            .prop_map(|(explicit, lines, at)| {
                std::array::from_fn(|i| {
                    if at.contains(&i) {
                        let (p, q, t) = lines[i];
                        Arg::Lnc(p, q, t)
                    } else {
                        Arg::Explicit(explicit[i])
                    }
                })
            })
    })
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn insphere_is_exact_on_random_grid_and_near_cospherical_points(args in calls()) {
        let mut staged = StagedIn::new();
        assert_every_call(&args, oracle_args(&args), &mut staged);
    }

    #[test]
    fn cospherical_points_are_zero(args in cospherical_args()) {
        prop_assert_eq!(oracle_args(&args), Sign::Zero, "{:?}", args);
        // The filter never gives a zero, so it must not decide.
        let [a, b, c, d, e] = any_points(&args);
        prop_assert_eq!(stages::filter_points(&a, &b, &c, &d, &e), None, "{:?}", args);
        let mut staged = StagedIn::new();
        assert_every_call(&args, Sign::Zero, &mut staged);
    }

    #[test]
    fn insphere_is_exact_over_the_whole_range_of_f64(args in extreme_args()) {
        let mut staged = StagedIn::new();
        assert_every_call(&args, oracle_args(&args), &mut staged);
    }

    #[test]
    fn swapping_two_of_the_first_four_points_flips_the_sign(args in calls()) {
        // The predicate is antisymmetric in its first four points; nothing
        // is assumed of a swap with the fifth.
        let mut staged = StagedIn::new();
        let sign = with_concrete5(&args, false, &mut staged);
        prop_assert_eq!(sign, oracle_args(&args), "{:?}", args);
        for i in 0..4 {
            for j in i + 1..4 {
                let mut swapped = args;
                swapped.swap(i, j);
                prop_assert_eq!(oracle_args(&swapped), -sign, "{:?}", swapped);
                prop_assert_eq!(with_concrete5(&swapped, false, &mut staged), -sign, "{:?}", swapped);
            }
        }
    }
}

#[test]
fn the_points_of_a_sphere_of_radius_5_are_zero() {
    // Points with integer coordinates on the sphere of radius 5 about the
    // origin, from permutations and signs of (3, 4, 0), (5, 0, 0) and
    // (0, 3, 4).
    let sphere = [
        [3.0, 4.0, 0.0],
        [5.0, 0.0, 0.0],
        [0.0, 3.0, 4.0],
        [-4.0, 0.0, 3.0],
        [0.0, -5.0, 0.0],
        [0.0, 4.0, -3.0],
    ];
    let mut calls: Vec<Args5> = Vec::new();
    for skip in 0..6 {
        let mut p = [[0.0; 3]; 5];
        for (slot, point) in p.iter_mut().zip((0..6).filter(|&i| i != skip)) {
            *slot = sphere[point];
        }
        // Moved off the origin and scaled by a power of two.
        let moved = p.map(|x| x.map(|v| 1.0 + v * pow2(-30)));
        for p in [p, moved] {
            calls.push(p.map(Arg::Explicit));
            // A point repeated.
            let mut repeated = p;
            repeated[4] = p[1];
            calls.push(repeated.map(Arg::Explicit));
            // A point on a line whose exact value is another point.
            for kind in 0..5 {
                let mut args = p.map(Arg::Explicit);
                args[skip % 5] = line_at(&p, skip % 5, (skip + 2) % 5, kind, 0.375);
                calls.push(args);
            }
        }
    }
    let mut staged = StagedIn::new();
    for args in calls {
        assert_eq!(oracle_args(&args), Sign::Zero, "{args:?}");
        let [a, b, c, d, e] = any_points(&args);
        assert_eq!(stages::filter_points(&a, &b, &c, &d, &e), None, "{args:?}");
        assert_every_call(&args, Sign::Zero, &mut staged);
    }
}

/// Five points near the sphere through `[a, 0, 0]`, `[0, v, 0]`,
/// `[0, 0, v]` and `[-v, 0, 0]`: the fifth is `[b, -v, v 2^-30]`, whose
/// last coordinate puts it just outside the sphere when `a` is `v`, so
/// that the filter does not decide.
fn near_sphere(v: f64, a: f64, b: f64) -> Points5 {
    [
        [a, 0.0, 0.0],
        [0.0, v, 0.0],
        [0.0, 0.0, v],
        [-v, 0.0, 0.0],
        [b, -v, v * pow2(-30)],
    ]
}

/// The points `[a, 0, 0]`, `[0, s, 0]`, `[0, 0, s]`, `[-s, 0, 0]` and the
/// origin, inside their sphere, which the filter decides when the largest
/// difference, `max(a, s)`, is in `D_MIN..=D_MAX`.
fn centred(s: f64, a: f64) -> Points5 {
    [
        [a, 0.0, 0.0],
        [0.0, s, 0.0],
        [0.0, 0.0, s],
        [-s, 0.0, 0.0],
        [0.0; 3],
    ]
}

/// The ends of the range of the filter, with an input that has a coordinate
/// at an end and the same input with that value one step outside, and the
/// stage of `CHAIN` that decides each: the filter inside, the stage on big
/// integers outside. Points near a sphere, which the filter does not
/// decide, at scales from the bottom of the range of `f64` to its top, go
/// to the stage on big integers too, as explicit points and as points on a
/// line whose parents are both the point.
#[test]
fn the_range_of_the_filter_hands_over_to_the_big_integers() {
    let (d_min, d_max) = (stages::D_MIN, stages::D_MAX);
    let up = f64::next_up;
    let [filter, _, big] = CHAIN;
    let mut cases: Vec<(String, Points5, &str)> = vec![
        ("D_MAX".into(), centred(d_max, d_max), filter),
        ("above D_MAX".into(), centred(d_max, up(d_max)), big),
        ("D_MIN".into(), centred(d_min, d_min), filter),
        (
            "below D_MIN".into(),
            centred(d_min.next_down(), d_min.next_down()),
            big,
        ),
    ];
    for exponent in [-1000, -400, -195, -108, 0, 154, 203, 400, 1000] {
        let v = pow2(exponent);
        cases.push((
            format!("near a sphere of size 2^{exponent}"),
            near_sphere(v, v, 0.0),
            big,
        ));
    }
    let mut staged = StagedIn::new();
    for (i, (case, p, stage)) in cases.into_iter().enumerate() {
        let expected = oracle(&p);
        assert_ne!(expected, Sign::Zero, "{case}: {p:?}");
        let mut lines = vec![p.map(Arg::Explicit)];
        // The cases near a sphere, after the four of the filter, whose ends
        // depend on the uncertainty, zero for explicit points.
        if i >= 4 {
            lines.push(p.map(|x| Arg::Lnc(x, x, 0.375)));
        }
        for args in lines {
            let decision = assert_every_call(&args, expected, &mut staged);
            assert_eq!(decision.name(), stage, "{case}: {args:?}");
        }
    }
}

/// A point on a line whose exact coordinates are of order 1, near a point
/// of a sphere, with components far apart: the parents differ by
/// `3 * 2^-52` in `x` and `t = 2^-7 (1 + 2^-52)`, so the exact `x` is
/// `1 + 3 * 2^-59 + 3 * 2^-111`, whose last bit is `2^-111`. The filter
/// declines it and the stage on big integers decides.
#[test]
fn points_on_a_line_near_a_sphere_go_to_the_big_integers() {
    // The sphere of radius 5 about (1, 1, 1).
    let on = [
        [4.0, 5.0, 1.0],
        [1.0, 4.0, 5.0],
        [5.0, 1.0, 4.0],
        [-2.0, -3.0, 1.0],
    ];
    let q = [1.0, -2.0, -3.0];
    let p = [1.0 + 3.0 * pow2(-52), -2.0, -3.0];
    let t = pow2(-7) * (1.0 + f64::EPSILON);
    for position in 0..5 {
        let mut args: Vec<Arg> = on.iter().copied().map(Arg::Explicit).collect();
        args.insert(position, Arg::Lnc(p, q, t));
        let args: Args5 = args.try_into().unwrap();
        let expected = oracle_args(&args);
        assert_ne!(expected, Sign::Zero);
        let decision = assert_every_call(&args, expected, &mut StagedIn::new());
        assert_eq!(decision.stage, BIG_INTEGERS, "{args:?}");
        assert_eq!(decision.why, Some(Why::BigIntegersOnly), "{args:?}");
    }
}

/// An exact coordinate of 16 components and one of 17: the chain takes both
/// on big integers, from `exact_big`, and never asks for the exact
/// coordinates as expansions, of any length.
#[test]
fn exact_coordinates_of_any_length_go_to_the_big_integers() {
    let one = |p: [f64; 3]| {
        let x = if p[0] == 0.0 { vec![] } else { vec![p[0]] };
        Long::new(x, p[1], p[2])
    };
    for n in [16usize, 17] {
        // Components 2^(-100 + 14 i), of alternating sign.
        let components: Vec<f64> = (0..n)
            .map(|i| {
                let x = pow2(-100 + 14 * i32::try_from(i).unwrap());
                if i % 2 == 0 { x } else { -x }
            })
            .collect();
        for position in 0..5 {
            let mut p: Vec<Long> = POSITIVE.into_iter().map(one).collect();
            p.insert(position, Long::new(components.clone(), 0.75, -0.5));
            let p: [Long; 5] = p.try_into().unwrap_or_else(|_| unreachable!());
            let expected = oracle_at(&p.each_ref().map(Long::exact_value));
            assert_ne!(expected, Sign::Zero);
            let mut buf = vec![0.0; 1 << 16];
            let mut scratch = Scratch::borrowed(&mut buf);
            let decision = first_deciding(&mut scratch, &p).unwrap();
            assert_eq!(
                decision,
                Decision {
                    stage: BIG_INTEGERS,
                    sign: expected,
                    why: Some(Why::BigIntegersOnly),
                },
                "{n} components at {position}"
            );
            let [a, b, c, d, e] = &p;
            let sign = stages::staged_in(&mut scratch, a, b, c, d, e);
            assert_eq!(sign, Ok(expected), "{n} components at {position}");
            assert_eq!(scratch.used(), 0);
            // Once for `first_deciding`, once for the chain; never `exact`.
            assert_eq!(p[position].big_calls.get(), 2, "{n}");
            assert!(p.iter().all(|q| q.exact_calls.get() == 0), "{n}");
        }
    }
}

/// Points on a line whose inputs are outside the range of the exact
/// arithmetic of `Lnc::exact`, a parameter below the product domain and
/// parents whose difference overflows, near a sphere: the chain does not
/// need `exact`, and the stage on big integers decides.
#[test]
fn a_point_without_exact_coordinates_as_expansions_goes_to_the_big_integers() {
    let [a, b, c, d] = POSITIVE.map(Arg::Explicit);
    let big = [f64::MAX, 1.0, -1.0];
    let minus_big = [-f64::MAX, 0.5, 5e-324];
    let cases: [Args5; 3] = [
        [a, b, c, d, Arg::Lnc(X, MY, pow2(-600))],
        [Arg::Lnc(Y, X, pow2(-700)), b, c, d, Arg::Explicit(MY)],
        [a, Arg::Lnc(big, minus_big, 0.5), c, d, Arg::Explicit(MZ)],
    ];
    let mut staged = StagedIn::new();
    for args in cases {
        let expected = oracle_args(&args);
        let decision = assert_every_call(&args, expected, &mut staged);
        assert_eq!(decision.stage, BIG_INTEGERS, "{args:?}");
        assert_eq!(decision.why, Some(Why::BigIntegersOnly), "{args:?}");
    }
}

/// The exact value of the in-sphere determinant at the points with the
/// exact coordinates `c`, to choose the corners of the boxes of
/// [`a_sign_given_with_uncertainty_holds_in_the_whole_box`]; its sign is
/// checked against the oracle there.
fn value_at(c: &[[Dyadic; 3]; 5]) -> Dyadic {
    let rows: [[Dyadic; 3]; 4] =
        std::array::from_fn(|i| std::array::from_fn(|k| &c[i][k] - &c[4][k]));
    let lifted = |r: &[Dyadic; 3]| &(&(&r[0] * &r[0]) + &(&r[1] * &r[1])) + &(&r[2] * &r[2]);
    let det3 = |r: [&[Dyadic; 3]; 3]| {
        let minor = |j: usize, k: usize| &(&r[1][j] * &r[2][k]) - &(&r[1][k] * &r[2][j]);
        &(&(&r[0][0] * &minor(1, 2)) - &(&r[0][1] * &minor(0, 2))) + &(&r[0][2] * &minor(0, 1))
    };
    // Along the last column, `|r_i|^2`, with the signs (-1)^(i + 3).
    let mut det = Dyadic::from_f64(0.0);
    for i in 0..4 {
        let others: Vec<&[Dyadic; 3]> = (0..4).filter(|&j| j != i).map(|j| &rows[j]).collect();
        let term = &lifted(&rows[i]) * &det3([others[0], others[1], others[2]]);
        det = if i % 2 == 1 {
            &det + &term
        } else {
            &det - &term
        };
    }
    det
}

/// How the errors are spread over the five points.
#[derive(Clone, Copy, Debug)]
enum Spread {
    /// The same error on every point.
    Equal,
    /// The error on two points, none on the others.
    Two(usize, usize),
    /// The error on one point and `2^-t` times it on the others.
    OneLarge(usize, i32),
}

fn spread() -> impl Strategy<Value = Spread> {
    prop_oneof![
        Just(Spread::Equal),
        (0usize..5, 1usize..5).prop_map(|(i, k)| Spread::Two(i, (i + k) % 5)),
        (0usize..5, 1i32..=10).prop_map(|(i, t)| Spread::OneLarge(i, t)),
    ]
}

/// The error of each point: `error`, spread as `spread` says.
fn errors(error: f64, spread: Spread) -> [f64; 5] {
    match spread {
        Spread::Equal => [error; 5],
        Spread::Two(i, j) => std::array::from_fn(|k| if k == i || k == j { error } else { 0.0 }),
        Spread::OneLarge(i, t) => {
            std::array::from_fn(|k| if k == i { error } else { error * pow2(-t) })
        }
    }
}

/// The corners of the boxes of the points `p`, each coordinate of point `i`
/// within `errors[i]`, at which the sign is checked: the two corners that
/// move the value furthest each way to first order, with each coordinate
/// pushed by the sign of the exact central difference of the value across
/// its box, the corners with every coordinate up and every one down, and
/// the corners whose coordinates are up where the bits of each of
/// `patterns` are set. Every corner is exact.
fn corners(p: &Points5, errors: [f64; 5], patterns: &[u16]) -> Vec<[[Dyadic; 3]; 5]> {
    let centre = p.map(|q| q.map(Dyadic::from_f64));
    let offset = |i: usize, up: bool| {
        let e = Dyadic::from_f64(errors[i]);
        if up { e } else { -&e }
    };
    let corner = |up: &dyn Fn(usize, usize) -> bool| {
        let mut c = centre.clone();
        for i in 0..5 {
            for k in 0..3 {
                c[i][k] = &centre[i][k] + &offset(i, up(i, k));
            }
        }
        c
    };
    // The sign of the central difference on each coordinate.
    let mut slope = [[0i8; 3]; 5];
    for i in 0..5 {
        for k in 0..3 {
            let mut above = centre.clone();
            let mut under = centre.clone();
            above[i][k] = &centre[i][k] + &offset(i, true);
            under[i][k] = &centre[i][k] + &offset(i, false);
            slope[i][k] = (&value_at(&above) - &value_at(&under)).sign();
        }
    }
    let mut out = vec![
        corner(&|i, k| slope[i][k] >= 0),
        corner(&|i, k| slope[i][k] < 0),
        corner(&|_, _| true),
        corner(&|_, _| false),
    ];
    for &bits in patterns {
        out.push(corner(&|i, k| bits >> (3 * i + k) & 1 == 1));
    }
    out
}

/// The binary exponent of the largest magnitude among the coordinates, or
/// 0 if they are all zero.
fn top_exponent(p: &Points5) -> i32 {
    let max = p.iter().flatten().fold(0.0f64, |m, x| m.max(x.abs()));
    if max > 0.0 {
        max.log2().floor() as i32
    } else {
        0
    }
}

/// Points for the filter with an uncertainty, and the exponent of the error
/// relative to their largest coordinate: points on a sphere, near one and at
/// random, with errors from `2^-60` to `2^-8` of their size; and points on
/// a sphere with one coordinate moved off it by `2^-m` of their size, `m`
/// from 20 to 44, with an error within a factor of four of the move, so
/// that the box often holds a cospherical truth and values of both signs.
fn uncertain_points() -> impl Strategy<Value = (Points5, i32)> {
    let points = prop_oneof![
        2 => near_cospherical5(),
        1 => cospherical5(),
        1 => random5(),
    ];
    let moved = (
        cospherical5(),
        0usize..5,
        0usize..3,
        20i32..=44,
        any::<bool>(),
        -2i32..=2,
    )
        .prop_map(|(mut p, point, axis, m, negative, k)| {
            let step = pow2(top_exponent(&p) - m);
            p[point][axis] += if negative { -step } else { step };
            (p, k - m)
        });
    prop_oneof![(points, -60i32..=-8), moved]
}

/// The filter with an uncertainty: approximations `p` of exact points, each
/// coordinate of point `i` within `errors[i]` of the true one, and `e` the
/// sum of the two largest errors, as `uncertainty_max` computes it. When
/// the filter decides, the oracle gives its sign at every corner of a
/// sample of corners of the boxes: the two that move the value furthest
/// each way to first order, two more and eight at random.
///
/// What this shows and what it does not: the form of the filter is global,
/// one size and one error for every coordinate, and the value is of degree
/// two in each coordinate, not affine, so its extremes over a box can lie
/// inside it, away from every corner, and the sample of corners covers few
/// of the `2^15`. A failure here is a wrong sign of the filter; a pass does
/// not prove the bound over the whole box, which rests on its derivation,
/// whose constants `the_emitted_constants_are_those_of_the_derivation`
/// checks. Nor can it tell the derived `KAPPA` from a smaller one above the
/// first-order coefficient that inputs reach, which a search over points on
/// a sphere put well below the derived one: that slack is the derivation's,
/// which bounds every term separately. With a much smaller `KAPPA`
/// this test fails, so it guards the presence of the term, and the
/// constants test its value (the figures were measured when the test was
/// written, and depend on the seed and the number of cases). The sample includes cases that the filter
/// decides and cases that it does not.
#[test]
fn a_sign_given_with_uncertainty_holds_in_the_whole_box() {
    let inputs = (uncertain_points(), spread(), [any::<u16>(); 8]);
    let mut counts = [0usize; 2];
    for ((p, exponent), spread, patterns) in sample(inputs, config().cases as usize) {
        let error = pow2((top_exponent(&p) + exponent).clamp(-1074, 1023));
        let errors = errors(error, spread);
        let err: [[f64; 3]; 5] = errors.map(|e| [e; 3]);
        let e = uncertainty_max(err.each_ref());
        let Some(sign) = stages::filter(refs(&p), e) else {
            counts[1] += 1;
            continue;
        };
        counts[0] += 1;
        let centre = p.map(|q| q.map(Dyadic::from_f64));
        assert_eq!(value_at(&centre).sign(), i8::from(oracle(&p)), "{p:?}");
        for c in corners(&p, errors, &patterns) {
            assert_eq!(oracle_at(&c), sign, "{p:?} within {errors:?}, e = {e:e}");
        }
    }
    println!("decided {}, declined {}", counts[0], counts[1]);
    assert!(counts[0] > 0 && counts[1] > 0, "{counts:?}");
}

/// The cases at the edge of the error bound of the filter, by the generated
/// checks, as those of `orient3d` are run: their exact sign by the
/// definition against the oracle; the filter, which decides some and not
/// others; each stage of `CHAIN` alone; and the chain, `staged_in`, which
/// decides every case, with the points as `[f64; 3]`, `Explicit`,
/// `AnyPoint` and points on a line whose parents are both the point.
#[test]
fn the_emitted_edge_cases_pass() {
    let cases = bound_edge_cases();
    assert!(cases.len() >= 64, "{} cases", cases.len());
    for p in &cases {
        assert_eq!(insphere_checks::exact(p), oracle(p), "{p:?}");
    }
    let [decided, declined] = check_bound_edge_cases(|p| stages::filter(refs(p), 0.0));
    assert!(decided > 0 && declined > 0, "{decided} {declined}");
    let mut staged = StagedIn::new();
    for stage in 0..CHAIN.len() {
        let mut scratch = staged.scratch();
        check_bound_edge_cases(|p| run_stage(stage, &mut scratch, p));
    }
    for raw in [false, true] {
        let [decided, declined] = check_bound_edge_cases(|p| {
            Some(with_concrete5(&p.map(Arg::Explicit), raw, &mut staged))
        });
        assert_eq!((decided, declined), (cases.len(), 0), "raw {raw}");
    }
    type Build = fn([f64; 3]) -> AnyPoint;
    let chains: [(&str, Build); 2] = [
        ("AnyPoint", |x| AnyPoint::from(Explicit::new_unchecked(x))),
        ("Lnc", |x| AnyPoint::from(Lnc::new_unchecked(x, x, 0.375))),
    ];
    for (name, point) in chains {
        let [decided, declined] = check_bound_edge_cases(|p| {
            let [a, b, c, d, e] = p.map(point);
            Some(staged.call(&a, &b, &c, &d, &e))
        });
        assert_eq!((decided, declined), (cases.len(), 0), "{name}");
    }
}

/// The cases at the edge of the error bound by parts, by the generated
/// checks: their exact sign by the definition against the oracle;
/// `filter_axes`, which decides some and not others, with no uncertainty
/// and with one, and declines or decides the pairs with an uncertainty when
/// it is doubled or halved, as `check_axes_edge_cases` requires;
/// `filter_axes_points` on explicit points, which is the same stage with no
/// uncertainty; and the chain, `staged_in`, which decides every case.
#[test]
fn the_emitted_edge_cases_of_filter_axes_pass() {
    let cases = axes_edge_cases();
    assert!(cases.len() >= 128, "{} cases", cases.len());
    for (p, _) in &cases {
        assert_eq!(insphere_checks::exact(p), oracle(p), "{p:?}");
    }
    let counts = check_axes_edge_cases(|p, e| stages::filter_axes(refs(p), e));
    println!("decided and declined, with no uncertainty and with one: {counts:?}");
    for [decided, declined] in counts {
        assert!(decided > 0 && declined > 0, "{counts:?}");
    }
    let mut staged = StagedIn::new();
    let mut explicit = [0usize; 2];
    for (p, e) in &cases {
        let [a, b, c, d, f] = p;
        if *e == [0.0; 3] {
            let sign = stages::filter_axes_points(a, b, c, d, f);
            assert_eq!(sign, stages::filter_axes(refs(p), [0.0; 3]), "{p:?}");
            explicit[usize::from(sign.is_none())] += 1;
        }
        assert_eq!(staged.call(a, b, c, d, f), oracle(p), "{p:?}");
    }
    assert_eq!(explicit, counts[0]);
}

/// `filter_axes` with an uncertainty on one axis: a point just inside the
/// unit sphere, near the point of the sphere on that axis, has a sign that
/// an error of `e[k] / 2` on that axis changes, so `filter_axes` must
/// decline it, which only the terms of kappa make it do; with no
/// uncertainty it decides. An uncertainty that is negative, infinite or NaN
/// never decides, on any axis.
#[test]
fn filter_axes_declines_what_the_uncertainty_can_change() {
    let h = pow2(-10);
    let [a, b, c, d] = POSITIVE;
    for k in 0..3 {
        let mut inside = [0.0; 3];
        inside[k] = -1.0 + h;
        let p = [a, b, c, d, inside];
        assert_eq!(oracle(&p), Sign::Positive);
        assert_eq!(
            stages::filter_axes(refs(&p), [0.0; 3]),
            Some(Sign::Positive)
        );
        // Outside the sphere, `2 h` away on axis `k`.
        let mut outside = inside;
        outside[k] -= 2.0 * h;
        assert_eq!(oracle(&[a, b, c, d, outside]), Sign::Negative);
        let mut e = [0.0; 3];
        e[k] = 4.0 * h;
        assert_eq!(stages::filter_axes(refs(&p), e), None, "axis {k}");
        assert_eq!(stages::filter(refs(&p), 4.0 * h), None, "axis {k}");
        for bad in [-pow2(-1074), -h, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut e = [0.0; 3];
            e[k] = bad;
            assert_eq!(
                stages::filter_axes(refs(&p), e),
                None,
                "axis {k}, e = {e:?}"
            );
        }
    }
}

/// The points of the axes at the distances `s` from the origin, with two on
/// axis `j`, on either side, and the origin: the leaves of each axis `k`
/// have the largest magnitude `s[k]`, and the value is `2 s_j^3 s_l s_m` or
/// its opposite, for the other two axes `l` and `m`.
fn on_axes(j: usize, s: [f64; 3]) -> Points5 {
    let mut p = [[0.0; 3]; 5];
    for (i, k) in [j, (j + 1) % 3, (j + 2) % 3].into_iter().enumerate() {
        p[i][k] = s[k];
    }
    p[3][j] = -s[j];
    p
}

/// The range of `filter_axes` on each axis, with the other two of size 1:
/// at `AXES_D_MAX`, with the two points of that axis, so that the value
/// grows as its cube, and at `AXES_D_MIN`, as a single point, so that it
/// shrinks as the size itself; one step outside, it declines. At the
/// bottom, the global filter, sized by the largest axis, declines.
#[test]
fn the_range_of_filter_axes_is_checked_on_every_axis() {
    let (low, high) = (stages::AXES_D_MIN, stages::AXES_D_MAX);
    for k in 0..3 {
        let other = (k + 1) % 3;
        for (j, v, decides) in [
            (k, high, true),
            (k, high.next_down(), true),
            (k, high.next_up(), false),
            (other, low, true),
            (other, low.next_up(), true),
            (other, low.next_down(), false),
        ] {
            let mut s = [1.0; 3];
            s[k] = v;
            let p = on_axes(j, s);
            let expected = oracle(&p);
            assert_ne!(expected, Sign::Zero);
            let decision = stages::filter_axes(refs(&p), [0.0; 3]);
            assert_eq!(
                decision,
                decides.then_some(expected),
                "axis {k}, size {v:e}"
            );
            if j == other {
                assert_eq!(stages::filter(refs(&p), 0.0), None, "axis {k}, size {v:e}");
            }
        }
    }
}

/// `staged_in` on four points of the unit sphere and `e`.
fn staged_in_with(e: &impl Point) -> Sign {
    let [a, b, c, d] = POSITIVE;
    let mut buf = vec![0.0; 1 << 16];
    let mut scratch = Scratch::borrowed(&mut buf);
    stages::staged_in(&mut scratch, &a, &b, &c, &d, e).unwrap()
}

/// Each of these calls of `staged_in` panics because an input is not
/// finite.
mod an_input_that_is_not_finite_panics {
    use super::*;

    #[test]
    #[should_panic(expected = "not finite")]
    fn with_a_nan_in_an_array() {
        staged_in_with(&[f64::NAN, 0.0, 0.0]);
    }

    #[test]
    #[should_panic(expected = "not finite")]
    fn with_an_infinity_in_an_array() {
        staged_in_with(&[0.0, f64::INFINITY, 0.0]);
    }

    #[test]
    #[should_panic(expected = "not finite")]
    fn with_a_point_on_a_line_whose_parameter_is_nan() {
        staged_in_with(&Lnc::new_unchecked(X, Y, f64::NAN));
    }

    #[test]
    #[should_panic(expected = "not finite")]
    fn with_an_any_point() {
        staged_in_with(&AnyPoint::from(Explicit::new_unchecked([
            0.0,
            0.0,
            f64::NEG_INFINITY,
        ])));
    }
}

#[test]
#[should_panic(
    expected = "implicit-predicates: a point has a weight other than one, which the predicates do not support"
)]
fn a_point_with_a_weight_panics() {
    staged_in_with(&Weighted);
}

#[test]
#[should_panic(
    expected = "implicit-predicates: a point has no exact coordinates: an input is not finite"
)]
fn a_point_without_big_coordinates_panics() {
    staged_in_with(&NoCoordinates);
}

/// The explicit points whose coordinate `k`, for `k = 3 i + axis`, is
/// `f64::MAX` when bit `k` of `large` is set and the least subnormal
/// otherwise, negated when bit `k` of `negative` is set.
fn extremes(large: u32, negative: u32) -> Points5 {
    std::array::from_fn(|i| {
        std::array::from_fn(|axis| {
            let k = 3 * i + axis;
            let x = if large >> k & 1 == 1 {
                f64::MAX
            } else {
                5e-324
            };
            if negative >> k & 1 == 1 { -x } else { x }
        })
    })
}

/// Explicit points whose coordinates are `f64::MAX` or the least
/// subnormal: alternating, under every pattern of signs of the fifteen
/// coordinates; and in every arrangement, with the last point of the
/// opposite sign to the others, so that no difference cancels. The
/// differences and their products then span the whole range of `f64`, the
/// worst case of the arena of the stage on big integers. An arena of
/// exactly `BIG_INTEGERS_EXPLICIT_ARENA` slots suffices for `staged_in` and
/// for the stage alone, and the sign is that of the oracle.
///
/// The 98,304 calls take over a minute in a debug build, so there it runs one
/// call in 64 and the call of the worst case found by the whole run, in a
/// release build; the high water is the same.
#[test]
fn the_arena_of_the_explicit_stage_is_enough() {
    let alternating = [0b010_1010_1010_1010, 0b101_0101_0101_0101];
    let mut calls: Vec<(u32, u32)> = alternating
        .into_iter()
        .flat_map(|large| (0..1 << 15).map(move |negative| (large, negative)))
        .collect();
    calls.extend((0..1 << 15).map(|large| (large, 0b111 << 12)));
    let worst = (0b111_0010_1010_0000, 0b111 << 12);
    assert!(calls.contains(&worst));
    if cfg!(debug_assertions) {
        calls = calls.into_iter().step_by(64).chain([worst]).collect();
    }
    let mut high_water = [0; 2];
    for (large, negative) in calls {
        let p = extremes(large, negative);
        let expected = oracle(&p);
        let mut buf = vec![0.0; BIG_INTEGERS_EXPLICIT_ARENA];
        let mut scratch = Scratch::borrowed(&mut buf);
        let [a, b, c, d, e] = &p;
        let sign = stages::staged_in(&mut scratch, a, b, c, d, e);
        assert_eq!(sign, Ok(expected), "{p:?}");
        assert_eq!(scratch.used(), 0);
        high_water[0] = high_water[0].max(scratch.high_water());
        let mut buf = vec![0.0; BIG_INTEGERS_EXPLICIT_ARENA];
        let mut scratch = Scratch::borrowed(&mut buf);
        let sign = run_stage(BIG_INTEGERS, &mut scratch, &p);
        assert_eq!(sign, Some(expected), "{p:?}");
        high_water[1] = high_water[1].max(scratch.high_water());
    }
    println!("high water of the arena: {high_water:?} of {BIG_INTEGERS_EXPLICIT_ARENA} slots");
    // The worst case found, within the constant: a borrowed arena of
    // `BIG_INTEGERS_EXPLICIT_ARENA` slots fails by itself above it, so this
    // pins that these inputs still reach the stage and use what they did.
    assert_eq!(high_water, [5883, 5883]);
}
