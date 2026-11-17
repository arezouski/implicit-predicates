//! The public `dot_sign_3d`, `dihedral_sign` and their `_in` variants against
//! the independent oracle on points of every type: random, grid, right-angled,
//! repeated and extreme points, points that share parents, points on a
//! vertex or an edge of their triangle, the symmetries of the predicates, the
//! arena of the `_in` variants, and the panics. The tests that call the
//! functions without the suffix need the `std` feature; those that call only
//! the `_in` variants run without it.

mod common;

#[path = "generators/angle_calls.rs"]
mod angle_calls;

use angle_calls::{
    Call3, Call4, Pt, any_points, calls3, calls4, exact_points, explicit_extreme_calls,
    extreme_calls, with_concrete3, with_concrete4,
};
use common::odd_points::{NoCoordinates, Weighted};
use common::{pow2, sign_of};
use implicit_predicates::stages::{dihedral_sign as dihedral_stages, dot_sign_3d as dot_stages};
use implicit_predicates::{
    Bpt, Explicit, Lnc, Point, Scratch, ScratchExhausted, Sign, dihedral_sign_in, dot_sign_3d_in,
};
use implicit_predicates_corpus::exact;
use implicit_predicates_test_support::{config, point3};
use proptest::prelude::*;

/// The initial size of the arena of each thread, in `f64`, which the
/// documentation quotes: a call with points of the families of these tests
/// never needs more than this in an arena of the caller.
const THREAD_ARENA: usize = 16_384;

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

    #[test]
    fn extreme_magnitudes_give_the_exact_sign(
        three in extreme_calls::<3>(),
        four in extreme_calls::<4>(),
    ) {
        prop_assert_eq!(dot_in_every_type(&three), oracle3(&three), "{:?}", three);
        prop_assert_eq!(dihedral_in_every_type(&four), oracle4(&four), "{:?}", four);
    }

    /// Explicit points whose nonzero coordinates are in the range of the
    /// stages on the stack of `dot_sign_3d`: they decide without an arena.
    #[test]
    fn explicit_dot_signs_in_the_stack_range_need_no_arena(
        n in prop::array::uniform3(prop::array::uniform3(-6i32..=6)),
        k in -420i32..=470,
    ) {
        let p: [[f64; 3]; 3] =
            n.map(|v| v.map(|x| f64::from(x) * 2f64.powi(k)));
        let args = p.map(Pt::Explicit);
        let sign = dot_stages::exact_stages(p.each_ref())
            .or_else(|| dot_stages::filter(p.each_ref(), 0.0));
        prop_assert_eq!(sign, Some(oracle3(&args)), "{:?}", p);
        let mut no_room = [0.0; 0];
        prop_assert_eq!(
            dot_sign_3d_in(&mut Scratch::borrowed(&mut no_room), &p[0], &p[1], &p[2]),
            Ok(oracle3(&args)),
            "{:?}", p
        );
    }

    /// A call with explicit points of every magnitude, or with points on a
    /// segment or a triangle of the families of these tests, stays within
    /// the initial arena of the thread, and for explicit points within the
    /// documented sizes.
    #[test]
    fn a_call_with_implicit_points_stays_within_a_bounded_arena(
        three in prop_oneof![calls3(), extreme_calls::<3>()],
        four in prop_oneof![calls4(), extreme_calls::<4>()],
    ) {
        let mut buf = vec![0.0; THREAD_ARENA];
        let mut scratch = Scratch::borrowed(&mut buf);
        let [p, q, r] = any_points(&three);
        prop_assert_eq!(dot_sign_3d_in(&mut scratch, &p, &q, &r), Ok(oracle3(&three)), "{:?}", three);
        prop_assert!(scratch.high_water() <= THREAD_ARENA, "{:?}", three);
        let [a, b, c, d] = any_points(&four);
        prop_assert_eq!(
            dihedral_sign_in(&mut scratch, &a, &b, &c, &d),
            Ok(oracle4(&four)),
            "{:?}", four
        );
        prop_assert!(scratch.high_water() <= THREAD_ARENA, "{:?}", four);
    }
}

proptest! {
    #![proptest_config(config())]

    /// For explicit points of every magnitude, the documented arenas suffice.
    #[test]
    fn the_documented_arenas_suffice_for_explicit_points(
        three in explicit_extreme_calls::<3>(),
        four in explicit_extreme_calls::<4>(),
        n in prop::array::uniform4(prop::array::uniform3(-6i32..=6)),
        k in -150i32..=190,
    ) {
        // Points of a grid in the range of the stage on expansions, which
        // runs first in an arena for `dihedral_sign`.
        let grid: [Pt; 4] = n.map(|v| Pt::Explicit(v.map(|x| f64::from(x) * 2f64.powi(k))));
        let [a, b, c, d] = any_points(&grid);
        let mut buf = vec![0.0; dihedral_stages::BIG_INTEGERS_EXPLICIT_ARENA];
        let mut scratch = Scratch::borrowed(&mut buf);
        prop_assert_eq!(dihedral_sign_in(&mut scratch, &a, &b, &c, &d), Ok(oracle4(&grid)), "{:?}", grid);
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
