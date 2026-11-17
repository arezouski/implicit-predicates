//! The public `dot_sign_3d`, `dihedral_sign` and their `_in` variants against
//! the independent oracle on points of every type: random, grid, right-angled,
//! repeated and extreme points, points that share parents, points on a
//! vertex or an edge of their triangle, points on the shared edge of
//! `dihedral_sign`, the symmetries of the predicates, the arena of the `_in`
//! variants, with points of widely spread exponents too, and the panics. The
//! tests that call the functions without the suffix need the `std` feature;
//! those that call only the `_in` variants run without it.

mod common;

#[path = "generators/angle_calls.rs"]
mod angle_calls;

use std::ops::RangeInclusive;

use angle_calls::{
    Call3, Call4, Pt, any_points, calls3, calls4, exact_points, explicit_extreme_calls,
    extreme_calls, wide_calls, wide_explicit_calls, with_concrete3, with_concrete4,
};
use common::odd_points::{NoCoordinates, Weighted};
use common::{pow2, sign_of};
use implicit_predicates::stages::{dihedral_sign as dihedral_stages, dot_sign_3d as dot_stages};
use implicit_predicates::{
    Bpt, ExactCoords, Explicit, Lnc, Point, Scratch, ScratchExhausted, Sign, dihedral_sign_in,
    dot_sign_3d_in,
};
use implicit_predicates_corpus::exact;
use implicit_predicates_test_support::{config, leaf, point3, sample};
use proptest::prelude::*;

/// The initial size of the arena of each thread, in `f64`, which the
/// documentation quotes. It is also the cap that these tests set on the
/// peak arena of a call with points of their families other than those of
/// wide exponents: an observed bound, defended by the tests, not a derived
/// guarantee.
const THREAD_ARENA: usize = 16_384;

/// The cap on the peak arena, in `f64`, of a call of `dot_sign_3d_in` on the
/// points of [`wide_calls`], and of its stage on expansions on their exact
/// coordinates, those coordinates included: an observed bound, the next
/// power of two at least twice the worst peak seen on 25,000 such calls with
/// all their points on segments and as many on triangles (1,656), defended
/// by these tests against a regression that makes the stage need far more;
/// not a derived guarantee.
const DOT_WIDE_ARENA: usize = 1 << 12;

/// The same for `dihedral_sign_in`, whose worst peak seen on those calls is
/// 50,510.
const DIHEDRAL_WIDE_ARENA: usize = 1 << 17;

/// The exponent `k` of the power of two `2^k`.
fn exponent_of(power: f64) -> i32 {
    // `log2` is exact on a power of two.
    let k = power.log2() as i32;
    assert_eq!(pow2(k), power, "not a power of two");
    k
}

/// The exponents of the parents of [`wide_calls`] for a stage on expansions
/// whose range is `range`, as widely spread as that range allows. A
/// coordinate of a point on a segment or on a triangle is a parent plus
/// products of a parameter and exact differences of parents. Its smallest
/// component can be the low part of the product of a parameter, rarely below
/// `2^-14`, and of the low part of a difference, itself at least a unit in
/// the last place of the smaller parent: about `2^-118` times the smallest
/// parent. Its largest is below four times the largest parent.
fn wide_exponents((min, max): (f64, f64)) -> RangeInclusive<i32> {
    exponent_of(min) + 118..=exponent_of(max) - 2
}

/// The exponents of explicit points whose coordinates are all in `range`,
/// as widely spread as it allows.
fn wide_explicit_exponents((min, max): (f64, f64)) -> RangeInclusive<i32> {
    exponent_of(min)..=exponent_of(max) - 1
}

/// `dot_sign_3d_in` on `args` in an arena four times as large as `cap`:
/// checks its sign against the oracle and that its peak arena is at most
/// `cap`.
fn dot_within(args: &[Pt; 3], cap: usize) -> Result<(), TestCaseError> {
    let mut buf = vec![0.0; 4 * cap];
    let mut scratch = Scratch::borrowed(&mut buf);
    let [p, q, r] = any_points(args);
    prop_assert_eq!(
        dot_sign_3d_in(&mut scratch, &p, &q, &r),
        Ok(oracle3(args)),
        "{:?}",
        args
    );
    let peak = scratch.high_water();
    prop_assert!(peak <= cap, "{} > {} on {:?}", peak, cap, args);
    Ok(())
}

/// The same for `dihedral_sign_in`.
fn dihedral_within(args: &[Pt; 4], cap: usize) -> Result<(), TestCaseError> {
    let mut buf = vec![0.0; 4 * cap];
    let mut scratch = Scratch::borrowed(&mut buf);
    let [a, b, c, d] = any_points(args);
    prop_assert_eq!(
        dihedral_sign_in(&mut scratch, &a, &b, &c, &d),
        Ok(oracle4(args)),
        "{:?}",
        args
    );
    let peak = scratch.high_water();
    prop_assert!(peak <= cap, "{} > {} on {:?}", peak, cap, args);
    Ok(())
}

/// A stage on expansions of the predicate, on exact coordinates.
type OverSpans<const N: usize> =
    fn(&mut Scratch<'_>, [&ExactCoords; N]) -> Result<Option<Sign>, ScratchExhausted>;

/// `stage` on the exact coordinates of `args`, written to an arena four times
/// as large as `cap`, whatever the filter would say: checks that its sign,
/// when it decides, is `expected`, and that its peak arena, the coordinates
/// included, is at most `cap`. Returns whether it decided.
fn stage_within<const N: usize>(
    args: &[Pt; N],
    stage: OverSpans<N>,
    expected: Sign,
    cap: usize,
) -> Result<bool, TestCaseError> {
    let mut buf = vec![0.0; 4 * cap];
    let mut scratch = Scratch::borrowed(&mut buf);
    let points = any_points(args);
    let coords: Option<Vec<ExactCoords>> =
        points.iter().map(|p| p.exact(&mut scratch).ok()).collect();
    let decided = match coords {
        Some(q) => stage(&mut scratch, std::array::from_fn(|i| &q[i])),
        None => Ok(None),
    };
    prop_assert!(decided.is_ok(), "the arena runs out on {:?}", args);
    let decided = decided.unwrap();
    if let Some(sign) = decided {
        prop_assert_eq!(sign, expected, "{:?}", args);
    }
    let peak = scratch.high_water();
    prop_assert!(peak <= cap, "{} > {} on {:?}", peak, cap, args);
    Ok(decided.is_some())
}

/// Calls of `dot_sign_3d` of [`wide_calls`], in the range of its stage on
/// expansions.
fn wide_calls3() -> impl Strategy<Value = [Pt; 3]> {
    wide_calls(wide_exponents(dot_stages::SPANS_TRANSLATED_RANGE))
}

/// Calls of `dihedral_sign` of [`wide_calls`], in the range of its stage on
/// expansions.
fn wide_calls4() -> impl Strategy<Value = [Pt; 4]> {
    wide_calls(wide_exponents(dihedral_stages::SPANS_TRANSLATED_RANGE))
}

/// The exact sign of `dot_sign_3d` at the points of `args`, by the oracle
/// written by hand.
fn oracle3(args: &[Pt; 3]) -> Sign {
    let c = exact_points(args);
    sign_of(exact::dot_sign_3d(&c[0], &c[1], &c[2]))
}

/// The exact sign of `dihedral_sign` at the points of `args`, by the oracle
/// written by hand.
fn oracle4(args: &[Pt; 4]) -> Sign {
    let c = exact_points(args);
    sign_of(exact::dihedral_sign(&c[0], &c[1], &c[2], &c[3]))
}

/// `dot_sign_3d`, when there is `std`, and `dot_sign_3d_in` with an arena
/// large enough, on points of any type; checks that the two agree and that
/// the arena is left as it was.
struct Dot;

impl Call3 for Dot {
    type Output = Sign;
    fn call<A: Point, B: Point, C: Point>(&mut self, p: &A, q: &B, r: &C) -> Sign {
        let mut buf = vec![0.0; 1 << 16];
        let mut scratch = Scratch::borrowed(&mut buf);
        let with_arena = dot_sign_3d_in(&mut scratch, p, q, r).expect("a large enough arena");
        assert_eq!(scratch.used(), 0);
        #[cfg(feature = "std")]
        assert_eq!(implicit_predicates::dot_sign_3d(p, q, r), with_arena);
        with_arena
    }
}

/// The same for `dihedral_sign`.
struct Dihedral;

impl Call4 for Dihedral {
    type Output = Sign;
    fn call<A: Point, B: Point, C: Point, D: Point>(&mut self, a: &A, b: &B, c: &C, d: &D) -> Sign {
        let mut buf = vec![0.0; 1 << 16];
        let mut scratch = Scratch::borrowed(&mut buf);
        let with_arena = dihedral_sign_in(&mut scratch, a, b, c, d).expect("a large enough arena");
        assert_eq!(scratch.used(), 0);
        #[cfg(feature = "std")]
        assert_eq!(implicit_predicates::dihedral_sign(a, b, c, d), with_arena);
        with_arena
    }
}

/// `dot_sign_3d` on the three arguments in every type: arrays, `Explicit`,
/// `Lnc` and `Bpt`, and `AnyPoint`; checks that they agree, and returns the
/// sign.
fn dot_in_every_type(args: &[Pt; 3]) -> Sign {
    let sign = with_concrete3(args, true, &mut Dot);
    assert_eq!(with_concrete3(args, false, &mut Dot), sign, "{args:?}");
    let [p, q, r] = any_points(args);
    assert_eq!(Dot.call(&p, &q, &r), sign, "{args:?}");
    sign
}

/// The same for `dihedral_sign`.
fn dihedral_in_every_type(args: &[Pt; 4]) -> Sign {
    let sign = with_concrete4(args, true, &mut Dihedral);
    assert_eq!(with_concrete4(args, false, &mut Dihedral), sign, "{args:?}");
    let [a, b, c, d] = any_points(args);
    assert_eq!(Dihedral.call(&a, &b, &c, &d), sign, "{args:?}");
    sign
}

/// The explicit point `x` and a point of any implicit kind with the same
/// exact value, for tests that repeat a point in another type.
fn same_point() -> impl Strategy<Value = ([f64; 3], Pt)> {
    (point3(), point3(), point3()).prop_flat_map(|(x, y, z)| {
        angle_calls::same_value(x, y, z, false).prop_map(move |pt| (x, pt))
    })
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn the_public_dot_sign_agrees_with_the_exact_sign(args in calls3()) {
        prop_assert_eq!(dot_in_every_type(&args), oracle3(&args), "{:?}", args);
    }

    #[test]
    fn the_public_dihedral_sign_agrees_with_the_exact_sign(args in calls4()) {
        prop_assert_eq!(dihedral_in_every_type(&args), oracle4(&args), "{:?}", args);
    }

    #[test]
    fn the_dot_sign_is_symmetric_in_its_ends(args in calls3()) {
        let sign = dot_in_every_type(&args);
        prop_assert_eq!(sign, oracle3(&args), "{:?}", args);
        let mut swapped = args;
        swapped.swap(0, 2);
        prop_assert_eq!(dot_in_every_type(&swapped), sign, "{:?}", swapped);
    }

    #[test]
    fn the_dihedral_sign_is_symmetric_in_its_apexes_and_in_its_edge(args in calls4()) {
        let sign = dihedral_in_every_type(&args);
        prop_assert_eq!(sign, oracle4(&args), "{:?}", args);
        // The apexes `a` and `b`, the ends of the edge `c` and `d`, and both.
        for swaps in [[(0, 1), (0, 0)], [(2, 3), (0, 0)], [(0, 1), (2, 3)]] {
            let mut swapped = args;
            for (i, j) in swaps {
                swapped.swap(i, j);
            }
            prop_assert_eq!(dihedral_in_every_type(&swapped), sign, "{:?}", swapped);
        }
    }

    #[test]
    fn repeated_points_give_zero_or_the_exact_sign(
        args3 in calls3(),
        args4 in calls4(),
        (x, same) in same_point(),
        which in 0usize..2,
        reversed in any::<bool>(),
    ) {
        // `p = q` and `r = q`: no angle, whatever the types of the two; the
        // second is the explicit point and the first a point of another type
        // with its value, or the other way round.
        let (first, second) = if reversed { (Pt::Explicit(x), same) } else { (same, Pt::Explicit(x)) };
        let apex = 1;
        let end = [0, 2][which];
        let mut dot = args3;
        (dot[end], dot[apex]) = (first, second);
        prop_assert_eq!(dot_in_every_type(&dot), Sign::Zero, "{:?}", dot);
        // The same point as itself, in the same type.
        let mut dot = args3;
        dot[end] = dot[apex];
        prop_assert_eq!(dot_in_every_type(&dot), Sign::Zero, "{:?}", dot);

        // `c = d`: no edge.
        let mut dihedral = args4;
        (dihedral[2], dihedral[3]) = (first, second);
        prop_assert_eq!(dihedral_in_every_type(&dihedral), Sign::Zero, "{:?}", dihedral);
        let mut dihedral = args4;
        dihedral[2] = dihedral[3];
        prop_assert_eq!(dihedral_in_every_type(&dihedral), Sign::Zero, "{:?}", dihedral);

        // `a = b`: the two normals are equal, so the sign is that of the
        // square of one: zero, when a triangle is degenerate, or positive.
        for (a, b) in [(first, second), (args4[0], args4[0])] {
            let mut dihedral = args4;
            (dihedral[0], dihedral[1]) = (a, b);
            let sign = dihedral_in_every_type(&dihedral);
            prop_assert_eq!(sign, oracle4(&dihedral), "{:?}", dihedral);
            prop_assert_ne!(sign, Sign::Negative, "{:?}", dihedral);
        }
    }

    #[test]
    fn a_bpt_at_a_vertex_gives_the_sign_of_the_vertex(
        args3 in calls3(),
        args4 in calls4(),
        (x, y, z) in (point3(), point3(), point3()),
        vertex in 0usize..3,
        position in 0usize..4,
    ) {
        // The vertex `p`, `q` or `r` of the triangle is the point with
        // `(u, v)` equal to `(1, 0)`, `(0, 1)` or `(0, 0)`.
        let (u, v) = [(1.0, 0.0), (0.0, 1.0), (0.0, 0.0)][vertex];
        let triangle = [x, y, z];
        let at_vertex = Pt::Bpt(triangle[0], triangle[1], triangle[2], u, v);
        let vertex = Pt::Explicit(triangle[vertex]);

        let mut with_bpt = args4;
        with_bpt[position] = at_vertex;
        let mut with_vertex = args4;
        with_vertex[position] = vertex;
        prop_assert_eq!(
            dihedral_in_every_type(&with_bpt),
            dihedral_in_every_type(&with_vertex),
            "{:?}", with_bpt
        );
        prop_assert_eq!(dihedral_in_every_type(&with_bpt), oracle4(&with_bpt), "{:?}", with_bpt);

        let position = position % 3;
        let mut with_bpt = args3;
        with_bpt[position] = at_vertex;
        let mut with_vertex = args3;
        with_vertex[position] = vertex;
        prop_assert_eq!(
            dot_in_every_type(&with_bpt),
            dot_in_every_type(&with_vertex),
            "{:?}", with_bpt
        );
        prop_assert_eq!(dot_in_every_type(&with_bpt), oracle3(&with_bpt), "{:?}", with_bpt);

        // An end of the angle, or of the edge, that is the vertex of
        // another argument: no angle.
        for end in [0, 2] {
            let mut dot = args3;
            (dot[end], dot[1]) = (at_vertex, vertex);
            prop_assert_eq!(dot_in_every_type(&dot), Sign::Zero, "{:?}", dot);
        }
        let mut edge = args4;
        (edge[2], edge[3]) = (at_vertex, vertex);
        prop_assert_eq!(dihedral_in_every_type(&edge), Sign::Zero, "{:?}", edge);
    }

    /// A point on a triangle that lies on the line through `c` and `d`, as
    /// the apex `a` or `b`: the triangle of that apex is degenerate, so the
    /// sign is zero. The point is built with `Bpt::new` when its parameters
    /// are in the triangle, as every kind but the last has them, and with
    /// `Bpt::new_unchecked` in every case.
    #[test]
    fn a_bpt_on_the_shared_edge_gives_zero(
        args4 in calls4(),
        (c, d, x) in (point3(), point3(), point3()),
        (w, kind, apex) in (0.0..=1.0f64, 0usize..5, 0usize..2),
    ) {
        let mut args = args4;
        (args[2], args[3]) = (Pt::Explicit(c), Pt::Explicit(d));
        // `v`, `1 - w` rounded, is a multiple of `2^-53` in `[0, 1]`, so
        // `u = 1 - v` is exact: `u + v = 1` exactly.
        let v = 1.0 - w;
        let u = 1.0 - v;
        let on_edge = match kind {
            // `u c + (1 - u) d`, with `u + v = 1` exactly.
            0 => Pt::Bpt(c, d, x, u, v),
            // The same with the third corner in another place, and the
            // parameter of `x` zero.
            1 => Pt::Bpt(c, x, d, w, 0.0),
            2 => Pt::Bpt(x, c, d, 0.0, w),
            // Two corners are `d`: the point is on the line whatever the
            // parameters.
            3 => Pt::Bpt(c, d, d, 0.5 * w, 0.5 * v),
            // Outside the triangle, on the line beyond `c`: `u` in
            // `[3/2, 2]` and `v = 1 - u`, which is exact and negative.
            _ => {
                let u = 1.5 + 0.5 * w;
                Pt::Bpt(c, d, x, u, 1.0 - u)
            }
        };
        args[apex] = on_edge;
        prop_assert_eq!(oracle4(&args), Sign::Zero, "{:?}", args);
        prop_assert_eq!(dihedral_in_every_type(&args), oracle4(&args), "{:?}", args);

        let Pt::Bpt(p, q, r, u, v) = on_edge else { unreachable!() };
        let made = Bpt::new(p, q, r, u, v);
        prop_assert_eq!(made.is_ok(), kind < 4, "{:?}", on_edge);
        if let Ok(point) = made {
            let [a, b, c, d] = any_points(&args);
            let mut buf = vec![0.0; 1 << 16];
            let mut scratch = Scratch::borrowed(&mut buf);
            let sign = if apex == 0 {
                dihedral_sign_in(&mut scratch, &point, &b, &c, &d)
            } else {
                dihedral_sign_in(&mut scratch, &a, &point, &c, &d)
            };
            prop_assert_eq!(sign, Ok(oracle4(&args)), "{:?}", args);
            #[cfg(feature = "std")]
            {
                let sign = if apex == 0 {
                    implicit_predicates::dihedral_sign(&point, &b, &c, &d)
                } else {
                    implicit_predicates::dihedral_sign(&a, &point, &c, &d)
                };
                prop_assert_eq!(sign, oracle4(&args), "{:?}", args);
            }
        }
    }

    #[test]
    fn extreme_magnitudes_give_the_exact_sign(
        three in extreme_calls::<3>(),
        four in extreme_calls::<4>(),
    ) {
        prop_assert_eq!(dot_in_every_type(&three), oracle3(&three), "{:?}", three);
        prop_assert_eq!(dihedral_in_every_type(&four), oracle4(&four), "{:?}", four);
    }

    /// Explicit points of small integers times a power of two, in the range
    /// of the stages on the stack of `dot_sign_3d`: they decide without an
    /// arena.
    #[test]
    fn explicit_dot_signs_of_a_grid_in_the_stack_range_need_no_arena(
        n in prop::array::uniform3(prop::array::uniform3(-6i32..=6)),
        k in -420i32..=470,
    ) {
        let p: [[f64; 3]; 3] = n.map(|v| v.map(|x| f64::from(x) * 2f64.powi(k)));
        prop_assert_eq!(dot_decides_on_the_stack(&p), Ok(oracle3(&p.map(Pt::Explicit))), "{:?}", p);
    }

    /// The same with coordinates of full mantissas and any magnitude in the
    /// range the documentation states, `2^-428..=2^478`, or zero.
    #[test]
    fn explicit_dot_signs_of_any_coordinates_in_the_stack_range_need_no_arena(
        p in prop::array::uniform3(prop::array::uniform3(
            prop_oneof![1 => Just(0.0), 6 => leaf(-428, 477, 53)],
        )),
    ) {
        prop_assert_eq!(dot_decides_on_the_stack(&p), Ok(oracle3(&p.map(Pt::Explicit))), "{:?}", p);
    }

    /// Explicit points of a grid whose differences are exact and in the range
    /// of the exact differences of `dihedral_sign`: the call needs no arena.
    #[test]
    fn explicit_dihedral_signs_with_exact_differences_need_no_arena(
        n in prop::array::uniform4(prop::array::uniform3(-6i32..=6)),
        k in -180i32..=230,
    ) {
        let p: [[f64; 3]; 4] = n.map(|v| v.map(|x| f64::from(x) * 2f64.powi(k)));
        let [a, b, c, d] = p;
        prop_assume!(dihedral_stages::exact_differences(p.each_ref()).is_some());
        let mut no_room = [0.0; 0];
        prop_assert_eq!(
            dihedral_sign_in(&mut Scratch::borrowed(&mut no_room), &a, &b, &c, &d),
            Ok(oracle4(&p.map(Pt::Explicit))),
            "{:?}", p
        );
    }

    /// A call with explicit points of every magnitude, or with points on a
    /// segment or a triangle of the families of these tests, stays within
    /// the initial arena of the thread; one with points on segments or
    /// triangles of full mantissas and widely spread exponents, in the range
    /// of the stage on expansions, within the caps observed for them, and so
    /// does that stage on their exact coordinates, whatever the filter says.
    #[test]
    fn a_call_with_implicit_points_stays_within_a_bounded_arena(
        three in prop_oneof![calls3(), extreme_calls::<3>()],
        four in prop_oneof![calls4(), extreme_calls::<4>()],
        wide3 in wide_calls3(),
        wide4 in wide_calls4(),
    ) {
        dot_within(&three, THREAD_ARENA)?;
        dihedral_within(&four, THREAD_ARENA)?;
        dot_within(&wide3, DOT_WIDE_ARENA)?;
        dihedral_within(&wide4, DIHEDRAL_WIDE_ARENA)?;
        let expansions3 = dot_stages::expansions_translated_in;
        stage_within(&wide3, expansions3, oracle3(&wide3), DOT_WIDE_ARENA)?;
        let expansions4 = dihedral_stages::expansions_translated_in;
        stage_within(&wide4, expansions4, oracle4(&wide4), DIHEDRAL_WIDE_ARENA)?;
    }
}

/// The calls of wide exponents are in the range of the stages on expansions:
/// the stage decides nearly all of them, so the caps above bound the stage at
/// work, not a stage that declines at once.
#[test]
fn the_stages_on_expansions_decide_the_calls_of_wide_exponents() {
    let cases = 400;
    let decided3 = sample(wide_calls3(), cases)
        .iter()
        .filter(|args| {
            let expansions = dot_stages::expansions_translated_in;
            stage_within(args, expansions, oracle3(args), DOT_WIDE_ARENA).unwrap()
        })
        .count();
    let decided4 = sample(wide_calls4(), cases)
        .iter()
        .filter(|args| {
            let expansions = dihedral_stages::expansions_translated_in;
            stage_within(args, expansions, oracle4(args), DIHEDRAL_WIDE_ARENA).unwrap()
        })
        .count();
    // A stage may decline a call with a parameter far below `2^-14`, which
    // is rare.
    assert!(decided3 * 100 >= cases * 95, "{decided3} of {cases}");
    assert!(decided4 * 100 >= cases * 95, "{decided4} of {cases}");
}

proptest! {
    #![proptest_config(config())]

    /// For explicit points of every magnitude, the documented arenas suffice
    /// for the whole call, whatever stage decides it.
    #[test]
    fn the_documented_arenas_suffice_for_explicit_points(
        three in explicit_extreme_calls::<3>(),
        four in explicit_extreme_calls::<4>(),
        n in prop::array::uniform4(prop::array::uniform3(-6i32..=6)),
        k in -150i32..=190,
        wide in wide_explicit_calls::<4>(wide_explicit_exponents(
            dihedral_stages::SPANS_TRANSLATED_RANGE,
        )),
    ) {
        // Points of a grid, and points of full mantissas and widely spread
        // exponents, in the range of the stage on expansions, which runs
        // first in an arena for `dihedral_sign`.
        let grid: [Pt; 4] = n.map(|v| Pt::Explicit(v.map(|x| f64::from(x) * 2f64.powi(k))));
        for args in [grid, wide] {
            let [a, b, c, d] = any_points(&args);
            let mut buf = vec![0.0; dihedral_stages::BIG_INTEGERS_EXPLICIT_ARENA];
            let mut scratch = Scratch::borrowed(&mut buf);
            prop_assert_eq!(
                dihedral_sign_in(&mut scratch, &a, &b, &c, &d),
                Ok(oracle4(&args)),
                "{:?}", args
            );
        }
        let mut buf = vec![0.0; dot_stages::BIG_INTEGERS_EXPLICIT_ARENA];
        let mut scratch = Scratch::borrowed(&mut buf);
        let [p, q, r] = any_points(&three);
        prop_assert_eq!(dot_sign_3d_in(&mut scratch, &p, &q, &r), Ok(oracle3(&three)), "{:?}", three);
        let mut buf = vec![0.0; dihedral_stages::BIG_INTEGERS_EXPLICIT_ARENA];
        let mut scratch = Scratch::borrowed(&mut buf);
        let [a, b, c, d] = any_points(&four);
        prop_assert_eq!(
            dihedral_sign_in(&mut scratch, &a, &b, &c, &d),
            Ok(oracle4(&four)),
            "{:?}", four
        );
    }
}

/// `dot_sign_3d_in` on explicit points with no room at all in the arena,
/// after checking that a stage on the stack decides them with no help of the
/// filter.
fn dot_decides_on_the_stack(p: &[[f64; 3]; 3]) -> Result<Sign, ScratchExhausted> {
    assert!(dot_stages::exact_stages(p.each_ref()).is_some(), "{p:?}");
    let mut no_room = [0.0; 0];
    dot_sign_3d_in(&mut Scratch::borrowed(&mut no_room), &p[0], &p[1], &p[2])
}

/// Explicit points out of the range of the stages on expansions, with an
/// exactly right angle at `q`, which neither test of the filter decides.
const HUGE_ANGLE: [[f64; 3]; 3] = [[1e300, 0.0, 1e300], [0.0; 3], [0.0, 1e300, 0.0]];

/// Explicit points out of that range, with an exactly right dihedral angle
/// at the edge `c`, `d`: the normals are along `y` and along `x`.
const HUGE_DIHEDRAL: [[f64; 3]; 4] = [
    [1e300, 0.0, 1e300],
    [0.0, 1e300, 1e300],
    [0.0, 0.0, 1e300],
    [0.0; 3],
];

#[test]
fn the_huge_calls_reach_the_stage_on_big_integers() {
    let [p, q, r] = HUGE_ANGLE.each_ref();
    assert_eq!(dot_stages::staged([p, q, r]), None);
    let [a, b, c, d] = HUGE_DIHEDRAL.each_ref();
    let mut buf = vec![0.0; 1 << 14];
    assert_eq!(
        dihedral_stages::staged(&mut Scratch::borrowed(&mut buf), [a, b, c, d]),
        Ok(None)
    );
}

#[test]
fn a_small_arena_is_reported_and_left_as_it_was() {
    let [p, q, r] = HUGE_ANGLE;
    let [a, b, c, d] = HUGE_DIHEDRAL;
    let mut small = [0.0; 4];
    let mut scratch = Scratch::borrowed(&mut small);
    let mark = scratch.mark();
    assert_eq!(
        dot_sign_3d_in(&mut scratch, &p, &q, &r),
        Err(ScratchExhausted)
    );
    assert_eq!(scratch.mark(), mark);
    assert_eq!(scratch.used(), 0);
    assert_eq!(
        dihedral_sign_in(&mut scratch, &a, &b, &c, &d),
        Err(ScratchExhausted)
    );
    assert_eq!(scratch.mark(), mark);
    assert_eq!(scratch.used(), 0);

    // The same with points on a segment and on a triangle with the same
    // exact value as `p` and `a`.
    let on_a_segment = Lnc::new(p, p, 0.5).unwrap();
    assert_eq!(
        dot_sign_3d_in(&mut scratch, &on_a_segment, &q, &r),
        Err(ScratchExhausted)
    );
    assert_eq!(scratch.used(), 0);
    let on_a_triangle = Bpt::new(a, a, a, 0.25, 0.5).unwrap();
    assert_eq!(
        dihedral_sign_in(&mut scratch, &on_a_triangle, &b, &c, &d),
        Err(ScratchExhausted)
    );
    assert_eq!(scratch.used(), 0);
}

/// The smallest size of arena, in `f64`, with which `f` decides, searched
/// from zero, with the check that every larger size up to `limit` decides
/// too and that the arena is left as it was.
fn smallest_arena(
    limit: usize,
    f: impl Fn(&mut Scratch<'_>) -> Result<Sign, ScratchExhausted>,
) -> (usize, Sign) {
    let run = |n: usize| {
        let mut buf = vec![0.0; n];
        let mut scratch = Scratch::borrowed(&mut buf);
        let result = f(&mut scratch);
        assert_eq!(scratch.used(), 0);
        result
    };
    let smallest = (0..=limit)
        .find(|&n| run(n).is_ok())
        .expect("the documented arena suffices");
    let sign = run(smallest).unwrap();
    for n in (smallest + 1..=limit).step_by(97) {
        assert_eq!(run(n), Ok(sign), "{n}");
    }
    assert_eq!(run(limit), Ok(sign));
    for n in (0..smallest).step_by(13) {
        assert_eq!(run(n), Err(ScratchExhausted), "{n}");
    }
    assert_eq!(run(smallest - 1), Err(ScratchExhausted));
    (smallest, sign)
}

#[test]
fn the_smallest_arena_that_suffices_decides() {
    let [p, q, r] = HUGE_ANGLE;
    let (n, sign) = smallest_arena(dot_stages::BIG_INTEGERS_EXPLICIT_ARENA, |s| {
        dot_sign_3d_in(s, &p, &q, &r)
    });
    assert_eq!(sign, Sign::Zero);
    assert!(n > 4 && n <= dot_stages::BIG_INTEGERS_EXPLICIT_ARENA, "{n}");

    let [a, b, c, d] = HUGE_DIHEDRAL;
    let (n, sign) = smallest_arena(dihedral_stages::BIG_INTEGERS_EXPLICIT_ARENA, |s| {
        dihedral_sign_in(s, &a, &b, &c, &d)
    });
    assert_eq!(sign, Sign::Zero);
    assert!(
        n > 8 && n <= dihedral_stages::BIG_INTEGERS_EXPLICIT_ARENA,
        "{n}"
    );

    // The arena of the constant, and no more, decides, with `used` back to
    // zero, and does not run over it.
    let mut buf = vec![0.0; dot_stages::BIG_INTEGERS_EXPLICIT_ARENA];
    let mut scratch = Scratch::borrowed(&mut buf);
    assert_eq!(dot_sign_3d_in(&mut scratch, &p, &q, &r), Ok(Sign::Zero));
    assert!(scratch.high_water() <= dot_stages::BIG_INTEGERS_EXPLICIT_ARENA);
    let mut buf = vec![0.0; dihedral_stages::BIG_INTEGERS_EXPLICIT_ARENA];
    let mut scratch = Scratch::borrowed(&mut buf);
    assert_eq!(
        dihedral_sign_in(&mut scratch, &a, &b, &c, &d),
        Ok(Sign::Zero)
    );
    assert!(scratch.high_water() <= dihedral_stages::BIG_INTEGERS_EXPLICIT_ARENA);
}

/// The documentation of the `_in` variants gives the arena that suffices for
/// explicit points, and the documentation of `dot_sign_3d` the range of the
/// stages on the stack.
#[test]
fn the_documented_numbers_are_the_constants() {
    let lib = include_str!("../src/lib.rs");
    let docs_of = |function: &str| -> String {
        let end = lib.find(&format!("pub fn {function}(")).expect(function);
        let start = lib[..end].rfind("\n\n").expect(function);
        // The documentation wraps its lines.
        lib[start..end].replace("\n/// ", " ")
    };
    let quoted = |words: usize| format!("{},{:03} `f64`", words / 1000, words % 1000);
    // 568 has no thousands separator.
    assert_eq!(dot_stages::BIG_INTEGERS_EXPLICIT_ARENA, 568);
    assert!(docs_of("dot_sign_3d_in").contains("568 `f64`"));
    let words = dihedral_stages::BIG_INTEGERS_EXPLICIT_ARENA;
    assert!(
        docs_of("dihedral_sign_in").contains(&quoted(words)),
        "{words}"
    );
    assert!(!docs_of("dihedral_sign_in").contains(&format!("{words} `f64`")));
    // The ranges of the stages on the stack.
    assert_eq!(dot_stages::TRANSLATED_RANGE, (pow2(-428), pow2(478)));
    assert!(docs_of("dot_sign_3d").contains("`2^-428..=2^478`"));
    assert_eq!(dihedral_stages::DIFFERENCES_RANGE, (pow2(-188), pow2(239)));
    assert!(docs_of("dihedral_sign").contains("`2^-188..=2^239`"));
    // The initial size of the arena of the thread.
    for function in ["dot_sign_3d", "dihedral_sign"] {
        assert!(
            docs_of(function).contains(&quoted(THREAD_ARENA)),
            "{function}"
        );
    }
    let arena = include_str!("../src/arena.rs");
    assert!(arena.contains("pub(crate) const INITIAL: usize = 16_384;"));
}

/// Four points of the unit square, the angle at the origin and the dihedral
/// angle at the edge `c`, `d`, through the functions with and without an
/// arena, with each point as an array.
#[test]
fn the_unit_vectors_decide_by_the_side_of_the_angle() {
    let q = [0.0; 3];
    let x = [1.0, 0.0, 0.0];
    let angles = [
        ([1.0, 1.0, 0.0], Sign::Positive),
        ([0.0, 1.0, 0.0], Sign::Zero),
        ([-1.0, 1.0, 0.0], Sign::Negative),
    ];
    let mut buf = vec![0.0; 1 << 12];
    for (r, sign) in angles {
        let mut scratch = Scratch::borrowed(&mut buf);
        assert_eq!(dot_sign_3d_in(&mut scratch, &x, &q, &r), Ok(sign));
        #[cfg(feature = "std")]
        assert_eq!(implicit_predicates::dot_sign_3d(&x, &q, &r), sign);
        // The edge is the z axis from `d` to `c`.
        let (c, d) = ([0.0, 0.0, 1.0], [0.0; 3]);
        assert_eq!(dihedral_sign_in(&mut scratch, &x, &r, &c, &d), Ok(sign));
        #[cfg(feature = "std")]
        assert_eq!(implicit_predicates::dihedral_sign(&x, &r, &c, &d), sign);
    }
    // An explicit point, validated.
    let e = Explicit::new([0.0, 1.0, 0.0]).unwrap();
    assert_eq!(
        dot_sign_3d_in(&mut Scratch::borrowed(&mut buf), &x, &q, &e),
        Ok(Sign::Zero)
    );
}

/// `dot_sign_3d` with the first argument `point` and two other explicit
/// points.
#[cfg(feature = "std")]
fn dot_with(point: &impl Point) -> Sign {
    implicit_predicates::dot_sign_3d(point, &[0.0; 3], &[0.0, 1.0, 0.0])
}

/// `dot_sign_3d_in` with an arena that is large enough, on the same points.
fn dot_with_arena(point: &impl Point) -> Sign {
    let mut buf = vec![0.0; 1 << 16];
    dot_sign_3d_in(
        &mut Scratch::borrowed(&mut buf),
        point,
        &[0.0; 3],
        &[0.0, 1.0, 0.0],
    )
    .unwrap()
}

/// `dihedral_sign` with the first argument `point` and three other explicit
/// points.
#[cfg(feature = "std")]
fn dihedral_with(point: &impl Point) -> Sign {
    implicit_predicates::dihedral_sign(point, &[0.0, 1.0, 0.0], &[0.0, 0.0, 1.0], &[0.0; 3])
}

/// `dihedral_sign_in` with an arena that is large enough, on the same points.
fn dihedral_with_arena(point: &impl Point) -> Sign {
    let mut buf = vec![0.0; 1 << 16];
    dihedral_sign_in(
        &mut Scratch::borrowed(&mut buf),
        point,
        &[0.0, 1.0, 0.0],
        &[0.0, 0.0, 1.0],
        &[0.0; 3],
    )
    .unwrap()
}

macro_rules! panics {
    ($($name:ident: $point:expr => $message:expr;)*) => {$(
        mod $name {
            use super::*;

            #[cfg(feature = "std")]
            #[test]
            #[should_panic(expected = $message)]
            fn dot_sign_3d() {
                dot_with(&$point);
            }

            #[test]
            #[should_panic(expected = $message)]
            fn dot_sign_3d_in() {
                dot_with_arena(&$point);
            }

            #[cfg(feature = "std")]
            #[test]
            #[should_panic(expected = $message)]
            fn dihedral_sign() {
                dihedral_with(&$point);
            }

            #[test]
            #[should_panic(expected = $message)]
            fn dihedral_sign_in() {
                dihedral_with_arena(&$point);
            }
        }
    )*};
}

panics! {
    a_nan_in_an_array: [f64::NAN, 0.0, 0.0] => "not finite";
    an_infinity_in_an_array: [0.0, f64::INFINITY, 0.0] => "not finite";
    a_nan_parameter: Lnc::new_unchecked([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], f64::NAN) => "not finite";
    a_nan_parameter_of_a_triangle:
        Bpt::new_unchecked([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], 0.5, f64::NAN)
        => "not finite";
    a_weight: Weighted =>
        "implicit-predicates: a point has a weight other than one, which the predicates do not support";
    no_exact_coordinates: NoCoordinates =>
        "implicit-predicates: a point has no exact coordinates: an input is not finite";
}

/// Points on a triangle on the shared edge `c`, `d`, or on its line, as the
/// apex `a` or `b`, with parameters that `Bpt::new` accepts and with
/// parameters outside the triangle: the sign is zero.
#[test]
fn a_bpt_on_the_shared_edge_gives_zero_on_fixed_points() {
    let (c, d, x) = ([0.0, 0.0, 1.0], [0.0; 3], [1.0, 0.0, 0.0]);
    let other = [0.0, 1.0, 0.0];
    let on_edge = [
        // `(0, 0, 0.25)`, with `u + v = 1`.
        Bpt::new(c, d, x, 0.25, 0.75).unwrap(),
        // `(0, 0, 0.1)`, not representable, with `v = 0`.
        Bpt::new(c, x, d, 0.1, 0.0).unwrap(),
        // Two corners are `d`.
        Bpt::new(c, d, d, 0.3, 0.4).unwrap(),
        // `(0, 0, 1.5)`, beyond `c`, outside the triangle.
        Bpt::new_unchecked(c, d, x, 1.5, -0.5),
    ];
    let mut buf = vec![0.0; 1 << 12];
    for point in on_edge {
        let mut scratch = Scratch::borrowed(&mut buf);
        assert_eq!(
            dihedral_sign_in(&mut scratch, &point, &other, &c, &d),
            Ok(Sign::Zero)
        );
        assert_eq!(
            dihedral_sign_in(&mut scratch, &other, &point, &c, &d),
            Ok(Sign::Zero)
        );
        assert_eq!(
            dihedral_sign_in(&mut scratch, &point, &other, &d, &c),
            Ok(Sign::Zero)
        );
        #[cfg(feature = "std")]
        {
            assert_eq!(
                implicit_predicates::dihedral_sign(&point, &other, &c, &d),
                Sign::Zero
            );
            assert_eq!(
                implicit_predicates::dihedral_sign(&other, &point, &c, &d),
                Sign::Zero
            );
        }
    }
    // Off the line, at `(0.25, 0, 0.25)`, the half-plane of the apex is that
    // of the x axis, and at 135 degrees from it that of the other.
    let off = Bpt::new(c, d, x, 0.25, 0.5).unwrap();
    let mut scratch = Scratch::borrowed(&mut buf);
    assert_eq!(
        dihedral_sign_in(&mut scratch, &off, &[-1.0, 1.0, 0.0], &c, &d),
        Ok(Sign::Negative)
    );
}

/// An exactly right dihedral angle in integers, with every axis of some extent
/// so that neither test of the filter can certify the zero: the exact
/// differences decide it, so the arena of `dihedral_sign_in` is not used.
#[test]
fn the_exact_differences_decide_a_right_dihedral_angle_without_an_arena() {
    let p = [
        [0.0, 1.0, 2.0],
        [-2.0, 2.0, -1.0],
        [1.0, 1.0, 0.0],
        [0.0, 0.0, 0.0],
    ];
    let r = p.each_ref();
    assert_eq!(dihedral_stages::filter(r, 0.0), None);
    assert_eq!(dihedral_stages::filter_axes(r, [0.0; 3]), None);
    assert_eq!(dihedral_stages::exact_differences(r), Some(Sign::Zero));
    let [a, b, c, d] = p;
    let mut no_room = [0.0; 0];
    assert_eq!(
        dihedral_sign_in(&mut Scratch::borrowed(&mut no_room), &a, &b, &c, &d),
        Ok(Sign::Zero)
    );
    // An inexact difference, `0.1 - 0.7`, leaves the stage on the stack.
    let mut p = p;
    (p[0][0], p[3][0]) = (0.1, 0.7);
    assert_eq!(dihedral_stages::exact_differences(p.each_ref()), None);
}
