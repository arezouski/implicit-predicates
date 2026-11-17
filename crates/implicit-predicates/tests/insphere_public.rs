//! The public `insphere` and `insphere_in` against the independent oracle on
//! points of every type: random, grid, cospherical and extreme points, the
//! symmetries of the predicate, the same point twice, the arena of
//! `insphere_in`, a point whose exact coordinates call a predicate, and the
//! panics. The tests that call `insphere` need the `std` feature; those that
//! call only `insphere_in` run without it.

mod common;

#[cfg(feature = "std")]
#[path = "generators/insphere_calls.rs"]
mod insphere_calls;

#[cfg(feature = "std")]
use std::cell::Cell;

use common::args::Arg;
#[cfg(feature = "std")]
use common::args::{Call5, any_points, with_concrete5};
#[cfg(feature = "std")]
use common::insphere::oracle_args;
use common::insphere::{BIG_INTEGERS, Points5, first_deciding};
use common::odd_points::{NoCoordinates, Weighted};
use implicit_predicates::stages::insphere::BIG_INTEGERS_EXPLICIT_ARENA;
#[cfg(feature = "std")]
use implicit_predicates::{BigCoords, ExactCoords, ExactError, insphere, orient3d};
use implicit_predicates::{Lnc, Point, Scratch, ScratchExhausted, Sign, insphere_in};
#[cfg(feature = "std")]
use implicit_predicates_test_support::{config, grid5, random5, sample};
#[cfg(feature = "std")]
use insphere_calls::{Args5, calls, cospherical_args, extreme_args, line_at, triangle_at};
#[cfg(feature = "std")]
use proptest::prelude::*;

/// `insphere`, and `insphere_in` with a growable arena, on points of any
/// type; checks that the two agree and that the arena is left as it was.
#[cfg(feature = "std")]
struct Public;

#[cfg(feature = "std")]
impl Call5 for Public {
    type Output = Sign;
    fn call<A: Point, B: Point, C: Point, D: Point, E: Point>(
        &mut self,
        a: &A,
        b: &B,
        c: &C,
        d: &D,
        e: &E,
    ) -> Sign {
        let sign = insphere(a, b, c, d, e);
        let mut scratch = Scratch::growable(1 << 10);
        let with_arena = insphere_in(&mut scratch, a, b, c, d, e);
        assert_eq!(scratch.used(), 0);
        assert_eq!(with_arena, Ok(sign));
        sign
    }
}

/// `insphere` on the five arguments in every type: arrays, `Explicit` and
/// `Lnc`, and `AnyPoint`; checks that they agree, and returns the sign.
#[cfg(feature = "std")]
fn every_type(args: &Args5) -> Sign {
    let sign = with_concrete5(args, true, &mut Public);
    assert_eq!(with_concrete5(args, false, &mut Public), sign, "{args:?}");
    let [a, b, c, d, e] = any_points(args);
    assert_eq!(Public.call(&a, &b, &c, &d, &e), sign, "{args:?}");
    sign
}

#[cfg(feature = "std")]
fn mixed_calls() -> impl Strategy<Value = Args5> {
    prop_oneof![calls(), cospherical_args(), extreme_args()]
}

#[cfg(feature = "std")]
proptest! {
    #![proptest_config(config())]

    #[test]
    fn insphere_matches_the_oracle_on_every_point_type(args in mixed_calls()) {
        prop_assert_eq!(every_type(&args), oracle_args(&args), "{:?}", args);
    }

    #[test]
    fn swapping_two_points_flips_the_sign(args in calls()) {
        // The predicate is antisymmetric in its five points.
        let sign = every_type(&args);
        prop_assert_eq!(sign, oracle_args(&args), "{:?}", args);
        for i in 0..5 {
            for j in i + 1..5 {
                let mut swapped = args;
                swapped.swap(i, j);
                prop_assert_eq!(every_type(&swapped), -sign, "{:?}", swapped);
            }
        }
    }

    #[test]
    fn the_same_point_twice_gives_zero(
        p in prop_oneof![random5(), grid5()],
        i in 0usize..5,
        k in 1usize..5,
        kind in 0usize..3,
        t in 0.0..=1.0f64,
    ) {
        // Any two of the five points equal make a row of the determinant
        // repeat, or zero; the second is given as itself and as a point on a
        // line whose exact value is the first.
        let j = (i + k) % 5;
        let mut same = p;
        same[j] = p[i];
        let explicit: Args5 = same.map(Arg::Explicit);
        prop_assert_eq!(every_type(&explicit), Sign::Zero, "{:?}", explicit);
        let mut on_a_line = p.map(Arg::Explicit);
        on_a_line[j] = line_at(&p, i, j, kind, t);
        prop_assert_eq!(every_type(&on_a_line), Sign::Zero, "{:?}", on_a_line);
    }

    #[test]
    fn a_point_on_a_triangle_equal_to_another_argument_gives_zero(
        (p, fresh, kind) in prop_oneof![
            // The midpoint of an edge needs the grid, where `2 x - y` is exact.
            (random5(), random5(), prop::sample::select(vec![0usize, 1, 2, 3, 5, 6, 7, 8])),
            (grid5(), grid5(), 0usize..9),
        ],
        position in 0usize..5,
        (a, b, c) in (1usize..5, 1usize..5, 1usize..5),
        t in 0.0..=1.0f64,
    ) {
        // At `position`, a point on a triangle whose exact value is the
        // point of another position (`i`, or `l` for `kind` 5): a vertex,
        // the midpoint of an edge, or a point with all parents equal; or,
        // for the kinds 6 to 8, that point as a vertex of a triangle whose
        // other corners are points that are not arguments, which a wrong
        // vertex would not repeat. The other argument stays explicit, so
        // two arguments are the same point.
        let (i, j, l) = ((position + a) % 5, (position + b) % 5, (position + c) % 5);
        let x = p[i];
        let mut args = p.map(Arg::Explicit);
        args[position] = match kind {
            6 => Arg::Bpt(x, fresh[0], fresh[1], 1.0, 0.0),
            7 => Arg::Bpt(fresh[0], x, fresh[1], 0.0, 1.0),
            8 => Arg::Bpt(fresh[0], fresh[1], x, 0.0, 0.0),
            _ => triangle_at(&p, i, j, l, kind, t),
        };
        prop_assert_eq!(every_type(&args), Sign::Zero, "{:?}", args);
        prop_assert_eq!(oracle_args(&args), Sign::Zero, "{:?}", args);
    }
}

/// Values of every magnitude that `f64` has at its ends and in the middle.
#[cfg(feature = "std")]
const EXTREMES: [f64; 10] = [
    1e-300,
    -1e-300,
    1e300,
    -1e300,
    5e-324,
    -5e-324,
    f64::MIN_POSITIVE,
    f64::MAX,
    0.0,
    1.0,
];

#[cfg(feature = "std")]
fn extreme_point() -> impl Strategy<Value = [f64; 3]> {
    prop::array::uniform3(prop::sample::select(EXTREMES.to_vec()))
}

#[cfg(feature = "std")]
proptest! {
    #![proptest_config(config())]

    #[test]
    fn extreme_magnitudes_are_decided(
        p in prop::array::uniform5(extreme_point()),
        q in prop::array::uniform5(extreme_point()),
        t in prop::sample::select(vec![0.0, 1.0, 0.5, 1e-300, 5e-324, -1e300]),
        lines in prop::sample::subsequence(vec![0usize, 1, 2, 3, 4], 0..=5),
    ) {
        let args: Args5 = std::array::from_fn(|i| {
            if lines.contains(&i) {
                Arg::Lnc(p[i], q[i], t)
            } else {
                Arg::Explicit(p[i])
            }
        });
        prop_assert_eq!(every_type(&args), oracle_args(&args), "{:?}", args);
    }
}

/// Five points on the sphere of radius 5 about the origin: the result is
/// zero, which neither test of the filter decides.
const SPHERE: Points5 = [
    [3.0, 4.0, 0.0],
    [5.0, 0.0, 0.0],
    [0.0, 3.0, 4.0],
    [-4.0, 0.0, 3.0],
    [0.0, -5.0, 0.0],
];

#[test]
fn a_small_arena_is_reported_and_left_as_it_was() {
    let [a, b, c, d, e] = SPHERE;
    // The call goes to the stage on big integers, for explicit points.
    let mut buf = vec![0.0; 1 << 15];
    let decision = first_deciding(
        &mut Scratch::borrowed(&mut buf),
        &SPHERE.map(Arg::Explicit).map(|x| x.any()),
    );
    assert_eq!(decision.map(|d| d.stage), Some(BIG_INTEGERS));

    let mut small = [0.0; 8];
    let mut scratch = Scratch::borrowed(&mut small);
    let mark = scratch.mark();
    assert_eq!(
        insphere_in(&mut scratch, &a, &b, &c, &d, &e),
        Err(ScratchExhausted)
    );
    assert_eq!(scratch.mark(), mark);
    assert_eq!(scratch.used(), 0);

    // The same with a point on a line, which has the same exact value.
    let m = Lnc::new(a, b, 1.0).unwrap();
    assert_eq!(
        insphere_in(&mut scratch, &m, &b, &c, &d, &e),
        Err(ScratchExhausted)
    );
    assert_eq!(scratch.used(), 0);

    // The arena that suffices for explicit points, and no more.
    let mut buf = vec![0.0; BIG_INTEGERS_EXPLICIT_ARENA];
    let mut scratch = Scratch::borrowed(&mut buf);
    assert_eq!(
        insphere_in(&mut scratch, &a, &b, &c, &d, &e),
        Ok(Sign::Zero)
    );
    assert_eq!(scratch.used(), 0);
    assert!(scratch.high_water() <= BIG_INTEGERS_EXPLICIT_ARENA);
    assert!(scratch.high_water() > 8);
}

/// The documentation of `insphere_in` gives the arena that suffices for
/// explicit points.
#[test]
fn the_documented_arena_for_explicit_points_is_the_constant() {
    let words = BIG_INTEGERS_EXPLICIT_ARENA;
    let documented = format!("{},{:03} `f64`", words / 1000, words % 1000);
    let lib = include_str!("../src/lib.rs");
    let insphere_in_docs = lib
        .split("pub fn insphere_in(")
        .next()
        .and_then(|before| before.rsplit("pub fn orient3d_in(").next())
        .expect("the documentation of insphere_in");
    assert!(insphere_in_docs.contains(&documented), "{documented}");
    // The number is not quoted in a different form.
    assert!(!insphere_in_docs.contains(&format!("{words} `f64`")));
}

#[cfg(feature = "std")]
thread_local! {
    /// The calls to `exact_big` of [`Reentrant`], and how many of them saw a
    /// predicate return the right sign.
    static CALLS: Cell<usize> = const { Cell::new(0) };
    /// The calls to `exact` of [`Reentrant`].
    static EXACT_CALLS: Cell<usize> = const { Cell::new(0) };
}

/// The calls to `Point::exact` of one point that the check of the contract
/// makes in a call of `insphere` that reaches the exact stages: two in debug
/// builds, none in release builds.
#[cfg(feature = "std")]
const CHECK_EXACT_CALLS: usize = if cfg!(debug_assertions) { 2 } else { 0 };

/// A point whose exact coordinates are those of `point`, written after calls
/// to `orient3d` and `insphere` with implicit points, which need an arena
/// while the arena of the thread is in use; its error is infinite, so the
/// filter never decides.
#[cfg(feature = "std")]
struct Reentrant {
    point: [f64; 3],
}

#[cfg(feature = "std")]
impl Point for Reentrant {
    fn approx(&self) -> &[f64; 3] {
        &self.point
    }

    fn err(&self) -> &[f64; 3] {
        &[f64::INFINITY; 3]
    }

    fn is_explicit(&self) -> bool {
        false
    }

    // The stages of `insphere` never ask for it, but in debug builds the
    // check of the contract of the point does.
    fn exact(&self, scratch: &mut Scratch<'_>) -> Result<ExactCoords, ExactError> {
        EXACT_CALLS.with(|n| n.set(n.get() + 1));
        self.point.exact(scratch)
    }

    fn exact_big(&self, scratch: &mut Scratch<'_>) -> Result<BigCoords, ExactError> {
        let [a, b, c] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let m = Lnc::new_unchecked(a, b, 0.1);
        assert_eq!(orient3d(&a, &b, &c, &m), Sign::Zero);
        let [s0, s1, s2, s3, s4] = SPHERE;
        let on_a_line = Lnc::new_unchecked(s0, s1, 1.0);
        assert_eq!(insphere(&on_a_line, &s1, &s2, &s3, &s4), Sign::Zero);
        CALLS.with(|n| n.set(n.get() + 1));
        self.point.exact_big(scratch)
    }
}

#[cfg(feature = "std")]
#[test]
fn a_point_that_calls_a_predicate_is_decided() {
    let [a, b, c, d, e] = SPHERE;
    let exact_calls = || EXACT_CALLS.with(|n| n.replace(0));
    exact_calls();
    let r = Reentrant { point: e };
    assert_eq!(insphere(&a, &b, &c, &d, &r), Sign::Zero);
    assert!(CALLS.with(Cell::get) >= 1);
    // The stages ask only for the coordinates as big integers.
    assert_eq!(exact_calls(), CHECK_EXACT_CALLS);
    let r = Reentrant { point: a };
    assert_eq!(insphere(&r, &b, &c, &d, &e), Sign::Zero);
    assert_eq!(exact_calls(), CHECK_EXACT_CALLS);
    // With an arena of the caller, the predicates inside use the arena of
    // the thread.
    let mut buf = vec![0.0; 1 << 15];
    let mut scratch = Scratch::borrowed(&mut buf);
    let r = Reentrant { point: c };
    assert_eq!(
        insphere_in(&mut scratch, &a, &b, &r, &d, &e),
        Ok(Sign::Zero)
    );
    assert_eq!(scratch.used(), 0);
    assert_eq!(exact_calls(), 0);
}

/// `insphere` on four points of the unit sphere and `e`.
#[cfg(feature = "std")]
fn with(e: &impl Point) -> Sign {
    let (a, b, c, d) = (
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [-1.0, 0.0, 0.0],
    );
    insphere(&a, &b, &c, &d, e)
}

/// `insphere_in` with an arena that is large enough, on the same points.
fn with_arena(e: &impl Point) -> Sign {
    let (a, b, c, d) = (
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [-1.0, 0.0, 0.0],
    );
    let mut buf = vec![0.0; 1 << 16];
    insphere_in(&mut Scratch::borrowed(&mut buf), &a, &b, &c, &d, e).unwrap()
}

#[test]
fn the_unit_sphere_decides_by_the_side_of_the_point() {
    // a, b, c, d = (1,0,0), (0,1,0), (0,0,1), (-1,0,0) are positively
    // oriented, so a point inside has a positive sign.
    let signs =
        |f: fn(&[f64; 3]) -> Sign| (f(&[0.0; 3]), f(&[2.0, 0.0, 0.0]), f(&[0.0, 0.0, -1.0]));
    let expected = (Sign::Positive, Sign::Negative, Sign::Zero);
    assert_eq!(signs(with_arena), expected);
    #[cfg(feature = "std")]
    assert_eq!(signs(with), expected);
}

macro_rules! panics {
    ($($name:ident: $point:expr => $message:expr;)*) => {$(
        mod $name {
            use super::*;

            #[cfg(feature = "std")]
            #[test]
            #[should_panic(expected = $message)]
            fn insphere() {
                with(&$point);
            }

            #[test]
            #[should_panic(expected = $message)]
            fn insphere_in() {
                with_arena(&$point);
            }
        }
    )*};
}

panics! {
    a_nan_in_an_array: [f64::NAN, 0.0, 0.0] => "not finite";
    an_infinity_in_an_array: [0.0, f64::INFINITY, 0.0] => "not finite";
    a_nan_parameter: Lnc::new_unchecked([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], f64::NAN) => "not finite";
    a_weight: Weighted =>
        "implicit-predicates: a point has a weight other than one, which the predicates do not support";
    no_exact_coordinates: NoCoordinates =>
        "implicit-predicates: a point has no exact coordinates: an input is not finite";
}

/// The calls of the property tests keep a large share of calls whose
/// implicit points are all on lines, the configuration of every call recorded
/// from a mesher, and a share of calls whose implicit points are all on
/// triangles and of calls with both: [`insphere_calls::mix`] chooses the kind
/// once per call.
#[cfg(feature = "std")]
#[test]
fn the_calls_keep_a_large_share_of_calls_on_lines_only() {
    let generators = [
        ("calls", sample(calls(), 4000)),
        ("cospherical", sample(cospherical_args(), 4000)),
        ("extreme", sample(extreme_args(), 4000)),
    ];
    for (name, args) in generators {
        let implicit: Vec<&Args5> = args
            .iter()
            .filter(|a| a.iter().any(|x| !matches!(x, Arg::Explicit(_))))
            .collect();
        let only = |kind: fn(&Arg) -> bool| {
            implicit
                .iter()
                .filter(|a| a.iter().all(|x| matches!(x, Arg::Explicit(_)) || kind(x)))
                .count()
        };
        let n = implicit.len();
        let lines = only(|x| matches!(x, Arg::Lnc(..)));
        let triangles = only(|x| matches!(x, Arg::Bpt(..)));
        let both = n - lines - triangles;
        assert!(5 * lines >= 2 * n, "{name}: {lines} of {n}");
        assert!(5 * triangles >= n, "{name}: {triangles} of {n}");
        assert!(10 * both >= n, "{name}: {both} of {n}");
    }
}
