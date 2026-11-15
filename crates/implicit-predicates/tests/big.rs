//! The chain of `orient3d` ends with the stage on big integers, so it
//! decides every finite input: the calls that no stage on expansions can
//! decide get the sign of the oracle, and an input that is not finite, or a
//! point whose exact coordinates have a weight other than one, is a panic.

// `orient3d` needs the `std` feature.
#![cfg(feature = "std")]

mod common;

use common::args::{Arg, Args, Call, Orient3d, Orient3dIn, any_points, exact_sign, with_concrete};
use common::generated::{exact, exact_at};
use common::odd_points::{Long, NoCoordinates, Weighted, WeightedOutOfRange};
use common::{CHAIN, Points, first_deciding, pow2, sign_of};
use implicit_predicates::stages::orient3d::{self as stages, BIG_INTEGERS_EXPLICIT_ARENA};
use implicit_predicates::{
    AnyPoint, BigCoords, ExactCoords, Lnc, Point, Scratch, ScratchExhausted, Sign, orient3d,
    orient3d_in,
};
use implicit_predicates_core::eft;
use implicit_predicates_oracle::Dyadic;
use implicit_predicates_test_support::{config, extreme_value, leaf, lnc_parameter};
use proptest::prelude::*;

/// Three points of a plane, for a fourth point to be tested against.
const PLANE: [[f64; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// Each of these calls panics because an input is not finite.
mod an_input_that_is_not_finite_panics {
    use super::*;

    #[test]
    #[should_panic(expected = "not finite")]
    fn with_a_nan_in_an_array() {
        let [a, b, c] = PLANE;
        orient3d(&a, &b, &c, &[f64::NAN, 0.0, 0.0]);
    }

    #[test]
    #[should_panic(expected = "not finite")]
    fn with_an_infinity_in_an_array() {
        let [a, b, c] = PLANE;
        orient3d(&a, &b, &c, &[0.0, f64::INFINITY, 0.0]);
    }

    #[test]
    #[should_panic(expected = "not finite")]
    fn with_a_point_on_a_line_whose_parameter_is_nan() {
        let [a, b, c] = PLANE;
        orient3d(&a, &b, &c, &Lnc::new_unchecked(a, b, f64::NAN));
    }

    #[test]
    #[should_panic(expected = "not finite")]
    fn with_an_arena() {
        let [a, b, c] = PLANE;
        let mut buf = vec![0.0; 1 << 16];
        let _ = orient3d_in(
            &mut Scratch::borrowed(&mut buf),
            &a,
            &b,
            &c,
            &[0.0, 0.0, f64::NEG_INFINITY],
        );
    }
}

#[test]
#[should_panic(
    expected = "implicit-predicates: a point has a weight other than one, which the predicates do not support"
)]
fn a_point_with_a_weight_panics() {
    let [a, b, c] = PLANE;
    orient3d(&a, &b, &c, &Weighted);
}

#[test]
#[should_panic(
    expected = "implicit-predicates: a point has a weight other than one, which the predicates do not support"
)]
fn a_point_with_a_weight_panics_with_an_arena() {
    let [a, b, c] = PLANE;
    let mut buf = vec![0.0; 1 << 12];
    let _ = orient3d_in(&mut Scratch::borrowed(&mut buf), &a, &b, &c, &Weighted);
}

#[test]
#[should_panic(
    expected = "implicit-predicates: a point has a weight other than one, which the predicates do not support"
)]
fn a_point_with_a_weight_that_only_the_big_integers_see_panics() {
    let [a, b, c] = PLANE;
    orient3d(&a, &b, &c, &WeightedOutOfRange);
}

#[test]
#[should_panic(
    expected = "implicit-predicates: a point has a weight other than one, which the predicates do not support"
)]
fn a_point_with_a_weight_that_only_the_big_integers_see_panics_with_an_arena() {
    let [a, b, c] = PLANE;
    let mut buf = vec![0.0; 1 << 12];
    let _ = orient3d_in(
        &mut Scratch::borrowed(&mut buf),
        &a,
        &b,
        &c,
        &WeightedOutOfRange,
    );
}

#[test]
#[should_panic(
    expected = "implicit-predicates: a point has no exact coordinates: an input is not finite"
)]
fn a_point_without_big_coordinates_panics() {
    let [a, b, c] = PLANE;
    orient3d(&a, &b, &c, &NoCoordinates);
}

#[test]
#[should_panic(
    expected = "implicit-predicates: a point has no exact coordinates: an input is not finite"
)]
fn a_point_without_big_coordinates_panics_with_an_arena() {
    let [a, b, c] = PLANE;
    let mut buf = vec![0.0; 1 << 12];
    let _ = orient3d_in(&mut Scratch::borrowed(&mut buf), &a, &b, &c, &NoCoordinates);
}

/// The calls that the documentation of `orient3d` showed giving `None`,
/// outside the range of the stages on expansions, and one with explicit
/// points near the ends of `f64`, with the sign that its examples now show:
/// the sign of the oracle.
#[test]
fn the_examples_that_gave_none_have_the_sign_of_the_oracle() {
    let (a, c) = ([1.0, 0.25, 0.5], [0.3, 0.7, 5.0]);
    let b1 = [2f64.powi(-140), 3.0, 0.75];
    let b2 = [0.1, 3.0, 0.75];
    let (p, q, r) = ([1.0, -1.0, 0.0], [0.0, 1.0, -1.0], [1.0, 0.0, -1.0]);
    let e = Arg::Explicit;
    let cases: [(Args, Sign); 4] = [
        ([e(a), e(b1), e(c), Arg::Lnc(a, b1, 0.1)], Sign::Zero),
        (
            [e(a), e(b2), e(c), Arg::Lnc(a, b2, 2f64.powi(-200))],
            Sign::Zero,
        ),
        (
            [
                e(p),
                e(q),
                e(r),
                Arg::Lnc([0.0; 3], [0.0; 3], 2f64.powi(-600)),
            ],
            Sign::Zero,
        ),
        (
            [
                e([0.0; 3]),
                e([1e300, 0.0, 0.0]),
                e([0.0, 1e300, 0.0]),
                e([1e300, -1e300, 5e-324]),
            ],
            Sign::Negative,
        ),
    ];
    for (args, documented) in cases {
        let expected = exact_sign(&args);
        assert_eq!(expected, documented, "{args:?}");
        // The point on a line is valid: `Lnc::new` accepts it.
        for arg in args {
            if let Arg::Lnc(p, q, t) = arg {
                Lnc::new(p, q, t).unwrap();
            }
        }
        for raw in [false, true] {
            assert_eq!(
                with_concrete(&args, raw, &mut Orient3d),
                expected,
                "{args:?}"
            );
            assert_eq!(
                with_concrete(&args, raw, &mut Orient3dIn),
                expected,
                "{args:?}"
            );
        }
    }
}

/// The documentation of `orient3d_in` and the changelog give the arena that
/// suffices for explicit points.
#[test]
fn the_documented_arena_for_explicit_points_is_the_constant() {
    let words = BIG_INTEGERS_EXPLICIT_ARENA;
    let documented = format!("{},{:03} `f64`", words / 1000, words % 1000);
    for (file, text) in [
        ("lib.rs", include_str!("../src/lib.rs")),
        ("CHANGELOG.md", include_str!("../../../CHANGELOG.md")),
    ] {
        assert!(text.contains(&documented), "{file}: {documented}");
    }
    // No README quotes it, so that no other copy can drift.
    for (file, text) in [
        ("README.md", include_str!("../../../README.md")),
        (
            "crates/implicit-predicates/README.md",
            include_str!("../README.md"),
        ),
    ] {
        assert!(!text.contains(&format!("{words}")), "{file}");
        assert!(!text.contains(&documented), "{file}");
    }
}

/// Four arguments whose inputs are values of one family of
/// [`extreme_value`], with points on a line, in any number from none to
/// four, at any positions; the others are explicit.
fn extreme_args() -> impl Strategy<Value = Args> {
    (0usize..5).prop_flat_map(|family| {
        let v = || extreme_value(family);
        let point = move || [v(), v(), v()];
        (
            [point(), point(), point(), point()],
            [
                (point(), point(), lnc_parameter()),
                (point(), point(), lnc_parameter()),
                (point(), point(), lnc_parameter()),
                (point(), point(), lnc_parameter()),
            ],
            prop::sample::subsequence(vec![0, 1, 2, 3], 0..=4),
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

/// Asserts that `orient3d` and `orient3d_in` give `expected` on `args`,
/// with the points as `[f64; 3]` or `Explicit` and as `Lnc`, and as
/// `AnyPoint`s.
fn assert_every_call(args: &Args, expected: Sign) {
    for raw in [false, true] {
        assert_eq!(
            with_concrete(args, raw, &mut Orient3d),
            expected,
            "{args:?}"
        );
        assert_eq!(
            with_concrete(args, raw, &mut Orient3dIn),
            expected,
            "{args:?}"
        );
    }
    let [a, b, c, d] = any_points(args);
    assert_eq!(orient3d(&a, &b, &c, &d), expected, "{args:?}");
    assert_eq!(Orient3dIn.call(&a, &b, &c, &d), expected, "{args:?}");
}

/// A point of order 1 to be scaled.
fn base_point() -> impl Strategy<Value = [f64; 3]> {
    [leaf(-60, 0, 53), leaf(-60, 0, 53), leaf(-60, 0, 53)]
}

/// `2^-1000` or `2^900`.
fn scale() -> impl Strategy<Value = f64> {
    prop::sample::select(vec![pow2(-1000), pow2(900)])
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn orient3d_is_exact_on_extreme_magnitudes(args in extreme_args()) {
        assert_every_call(&args, exact_sign(&args));
    }

    #[test]
    fn degenerate_calls_on_extreme_magnitudes_are_zero(
        base in [base_point(), base_point(), base_point()],
        scales in [scale(), scale(), scale()],
        t in [lnc_parameter(), lnc_parameter(), lnc_parameter(), lnc_parameter()],
        (from, to) in (0usize..4, 0usize..3),
    ) {
        let [a, b, c] = std::array::from_fn(|i| base[i].map(|x| x * scales[i]));
        let e = Arg::Explicit;
        // A point repeated, at any two positions.
        let mut repeated = [e(a), e(b), e(c), e(a)];
        repeated.swap(3, from);
        repeated.swap(to, (to + 1) % 4);
        // Points on the edges of the triangle of `a`, `b` and `c`, which lie
        // in its plane, with one to four of them.
        let on_edges = [
            [e(a), e(b), e(c), Arg::Lnc(a, b, t[0])],
            [Arg::Lnc(c, a, t[1]), e(b), e(c), Arg::Lnc(a, b, t[0])],
            [Arg::Lnc(c, a, t[1]), e(b), Arg::Lnc(b, c, t[2]), Arg::Lnc(a, b, t[0])],
            [
                Arg::Lnc(c, a, t[1]),
                Arg::Lnc(a, b, t[3]),
                Arg::Lnc(b, c, t[2]),
                Arg::Lnc(a, b, t[0]),
            ],
        ];
        for args in [repeated].into_iter().chain(on_edges) {
            prop_assert_eq!(exact_sign(&args), Sign::Zero, "{:?}", args);
            assert_every_call(&args, Sign::Zero);
        }
    }
}

/// Four points near the plane `x + y + z = v`, so that the filter does not
/// decide, with `a.x`, `b.x` and `d.x` given: `[ax, 0, 0]`, `[bx, v, 0]`,
/// `[0, 0, v]` and `[dx, v / 2, v / 2 + v 2^-52]`. With `dx` zero, every
/// difference is exact.
fn near_plane(v: f64, ax: f64, bx: f64, dx: f64) -> Points {
    [
        [ax, 0.0, 0.0],
        [bx, v, 0.0],
        [0.0, 0.0, v],
        [dx, v / 2.0, v / 2.0 + v * pow2(-52)],
    ]
}

/// Explicit points near a plane, with a coordinate in `(2^238, 2^239]`,
/// above `TRANSLATED_RANGE`, and a difference that is not exact in `f64`:
/// no stage on the stack decides, so the stage on big integers does, in the
/// arena.
#[test]
fn coordinates_above_the_translated_range_go_to_the_big_integers() {
    assert_eq!(stages::TRANSLATED_RANGE.1, pow2(238));
    let v = 1.5 * pow2(238);
    let p = near_plane(v, v, 0.0, 1.0);
    // `a.x - d.x` is not exact.
    assert_ne!(eft::two_diff(p[0][0], p[3][0]).1, 0.0);
    let [a, b, c, d] = &p;
    assert_eq!(stages::staged([a, b, c, d]), None);
    assert_eq!(stages::exact_differences([a, b, c, d]), None);
    let [ea, eb, ec, ed] = p.map(|q| q.map(Dyadic::from_f64));
    let expected = sign_of(implicit_predicates_corpus::exact::orient3d(
        &ea, &eb, &ec, &ed,
    ));
    assert_ne!(expected, Sign::Zero);
    assert_eq!(orient3d(a, b, c, d), expected);
    let mut buf = [0.0];
    assert_eq!(
        orient3d_in(&mut Scratch::borrowed(&mut buf), a, b, c, d),
        Err(ScratchExhausted)
    );
}

/// Each range of the stages for explicit points, with an input that has a
/// coordinate or a difference at an end of it and the same input with that
/// value one step outside, and the stage of `CHAIN` that decides each: the
/// next stage whose range holds the input, and the stage on big integers
/// outside every range.
#[test]
fn each_range_hands_over_to_the_next_stage() {
    let (min, d) = stages::DIFFERENCES_RANGE;
    let t = stages::TRANSLATED_RANGE.1;
    assert_eq!(
        stages::TRANSLATED_RANGE.0,
        min,
        "both ranges have the same lower end"
    );
    assert!(t < d, "{t:e} {d:e}");
    let up = f64::next_up;
    let below = min.next_down();
    // An inexact difference, so that `exact_differences` declines.
    let inexact = pow2(-60);
    let cases: [(&str, Points, &str); 8] = [
        // `a.x - d.x` is the upper end of `DIFFERENCES_RANGE`.
        (
            "DIFFERENCES_RANGE.1",
            near_plane(d, d, 0.0, 0.0),
            "exact_differences",
        ),
        (
            "above DIFFERENCES_RANGE",
            near_plane(d, up(d), 0.0, 0.0),
            "big_integers",
        ),
        // `b.x - d.x` is the lower end.
        (
            "DIFFERENCES_RANGE.0",
            near_plane(1.0, 1.0, min, 0.0),
            "exact_differences",
        ),
        (
            "below DIFFERENCES_RANGE",
            near_plane(1.0, 1.0, below, 0.0),
            "big_integers",
        ),
        (
            "TRANSLATED_RANGE.1",
            near_plane(t, t, 0.0, 1.0),
            "expansions_translated",
        ),
        (
            "above TRANSLATED_RANGE",
            near_plane(t, up(t), 0.0, 1.0),
            "big_integers",
        ),
        (
            "TRANSLATED_RANGE.0",
            near_plane(1.0, 1.0, min, inexact),
            "expansions_translated",
        ),
        (
            "below TRANSLATED_RANGE",
            near_plane(1.0, 1.0, below, inexact),
            "big_integers",
        ),
    ];
    for (case, p, stage) in cases {
        let (i, sign) = first_deciding(&p).expect("finite inputs");
        assert_eq!(CHAIN[i].0, stage, "{case}: {p:?}");
        let expected = exact(&p);
        assert_eq!(sign, expected, "{case}: {p:?}");
        assert_eq!(common::orient3d(&p), expected, "{case}: {p:?}");
        assert_ne!(expected, Sign::Zero, "{case}: {p:?}");
    }
}

/// The first stage that decides on points of any type, in the order of
/// `exact_stages_in` after `filter_points`, by name, with its sign.
fn first_deciding_over_spans(p: &[AnyPoint; 4]) -> (&'static str, Sign) {
    let [a, b, c, d] = p;
    if let Some(sign) = stages::filter_points(a, b, c, d) {
        return ("filter_points", sign);
    }
    let mut buf = vec![0.0; 1 << 16];
    let mut scratch = Scratch::borrowed(&mut buf);
    let q: Option<Vec<ExactCoords>> = p.iter().map(|x| x.exact(&mut scratch).ok()).collect();
    if let Some(q) = q {
        let q = [&q[0], &q[1], &q[2], &q[3]];
        if let Some(sign) = stages::expansions_translated_in(&mut scratch, q).unwrap() {
            return ("expansions_translated_in", sign);
        }
    }
    let q: Vec<BigCoords> = p
        .iter()
        .map(|x| x.exact_big(&mut scratch).unwrap())
        .collect();
    let q = [&q[0], &q[1], &q[2], &q[3]];
    let sign = stages::big_integers_in(&mut scratch, q).unwrap();
    ("big_integers", sign)
}

/// The ranges of the stages over spans, checked as
/// [`each_range_hands_over_to_the_next_stage`] checks those for explicit
/// points, on points on a line whose parents are both the point, so that
/// each exact coordinate is one component.
#[test]
fn each_range_over_spans_hands_over_to_the_next_stage() {
    let (min, t) = stages::SPANS_TRANSLATED_RANGE;
    let up = f64::next_up;
    let below = min.next_down();
    let cases: [(&str, Points, &str); 4] = [
        (
            "SPANS_TRANSLATED_RANGE.1",
            near_plane(t, t, 0.0, 0.0),
            "expansions_translated_in",
        ),
        (
            "above SPANS_TRANSLATED_RANGE",
            near_plane(t, up(t), 0.0, 0.0),
            "big_integers",
        ),
        (
            "SPANS_TRANSLATED_RANGE.0",
            near_plane(1.0, 1.0, min, 0.0),
            "expansions_translated_in",
        ),
        (
            "below SPANS_TRANSLATED_RANGE",
            near_plane(1.0, 1.0, below, 0.0),
            "big_integers",
        ),
    ];
    for (case, p, stage) in cases {
        let args = p.map(|x| Arg::Lnc(x, x, 0.375));
        let points = any_points(&args);
        // Every point has exact coordinates, so that the test reaches the
        // range checks of the stages over spans.
        let mut buf = vec![0.0; 1 << 12];
        let mut scratch = Scratch::borrowed(&mut buf);
        assert!(
            points.iter().all(|x| x.exact(&mut scratch).is_ok()),
            "{case}: {p:?}"
        );
        let (name, sign) = first_deciding_over_spans(&points);
        assert_eq!(name, stage, "{case}: {p:?}");
        let expected = exact_sign(&args);
        assert_eq!(expected, exact(&p), "{case}: {p:?}");
        assert_eq!(sign, expected, "{case}: {p:?}");
        assert_ne!(expected, Sign::Zero, "{case}: {p:?}");
        assert_every_call(&args, expected);
    }
}

/// An exact coordinate of 16 components, which the stages over spans take,
/// and one of 17, which they decline, so that the stage on big integers
/// decides it, with the coordinates from `exact_big`.
#[test]
fn seventeen_components_go_to_the_big_integers() {
    assert_eq!(stages::MAX_COORD_COMPONENTS, 16);
    let (a, b, c) = ([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
    for (n, expected_big_calls) in [(16usize, 0), (17, 1)] {
        // Components 2^(-170 + 20 i), of alternating sign, inside the
        // ranges of the stages over spans.
        let components: Vec<f64> = (0..n)
            .map(|i| {
                let x = pow2(-170 + 20 * i32::try_from(i).unwrap());
                if i % 2 == 0 { x } else { -x }
            })
            .collect();
        let long = Long::new(components, 0.75, -0.5);
        let value = long.exact_value();
        let [da, db, dc] = [a, b, c].map(|x| x.map(Dyadic::from_f64));
        let expected = exact_at(&[da, db, dc, value]);
        assert_ne!(expected, Sign::Zero);
        assert_eq!(orient3d(&a, &b, &c, &long), expected, "{n} components");
        // In debug builds `orient3d` also checks the contract of the point,
        // which asks for its coordinates as big integers; `orient3d_in`
        // does not.
        if cfg!(not(debug_assertions)) {
            assert_eq!(long.big_calls.get(), expected_big_calls, "{n} components");
        }
        long.big_calls.set(0);
        let mut buf = vec![0.0; 1 << 12];
        let mut scratch = Scratch::borrowed(&mut buf);
        assert_eq!(orient3d_in(&mut scratch, &a, &b, &c, &long), Ok(expected));
        assert_eq!(scratch.used(), 0);
        assert_eq!(long.big_calls.get(), expected_big_calls, "{n} components");
        // The stage over spans alone: it decides on 16 components, not on
        // 17.
        let q = [a, b, c].map(|x| x.exact(&mut scratch).unwrap());
        let r = long.exact(&mut scratch).unwrap();
        let q = [&q[0], &q[1], &q[2], &r];
        let spans = stages::expansions_translated_in(&mut scratch, q).unwrap();
        let decided = if n == 16 { Some(expected) } else { None };
        assert_eq!(spans, decided, "{n} components");
    }
}

/// The calls with a point on a line that no stage on expansions decides,
/// which gave `None` before the stage on big integers: the documented
/// examples, a component of the exact coordinates one bit below the range of
/// the stages over spans, and inputs outside the range of the exact
/// arithmetic of `Lnc::exact`, a parameter below the product domain and
/// parents whose difference overflows. Each gets the sign of the oracle from
/// the stage on big integers.
#[test]
fn an_lnc_outside_the_range_of_its_expansions_is_decided() {
    let e = Arg::Explicit;
    let (a, c) = ([1.0, 0.25, 0.5], [0.3, 0.7, 5.0]);
    let (p, q, r) = ([1.0, -1.0, 0.0], [0.0, 1.0, -1.0], [1.0, 0.0, -1.0]);
    let k = 54;
    let next = 1.0 + f64::EPSILON;
    let low = [pow2(k - 84) * next / 2.0, 1.0, 1.0];
    let top = [pow2(233), 1.0, 1.0];
    let tk = pow2(-k) * next;
    let big = [f64::MAX, 1.0, -1.0];
    let minus_big = [-f64::MAX, 0.5, 5e-324];
    let cases: [(Args, bool); 7] = [
        // The documented examples.
        (
            [
                e(a),
                e([pow2(-140), 3.0, 0.75]),
                e(c),
                Arg::Lnc(a, [pow2(-140), 3.0, 0.75], 0.1),
            ],
            true,
        ),
        (
            [
                e(a),
                e([0.1, 3.0, 0.75]),
                e(c),
                Arg::Lnc(a, [0.1, 3.0, 0.75], pow2(-200)),
            ],
            true,
        ),
        (
            [e(p), e(q), e(r), Arg::Lnc([0.0; 3], [0.0; 3], pow2(-600))],
            false,
        ),
        // A component one bit below the range of the stages over spans.
        ([e(low), e(top), e(c), Arg::Lnc(low, top, tk)], true),
        // `t` below the product domain, and a difference of parents that
        // overflows: `exact` fails.
        ([e(a), Arg::Lnc(p, q, pow2(-600)), e(c), e(q)], false),
        ([Arg::Lnc(big, minus_big, 0.5), e(a), e(c), e(p)], false),
        (
            [
                Arg::Lnc(big, minus_big, 0.25),
                Arg::Lnc(minus_big, big, 0.75),
                Arg::Lnc(p, q, pow2(-600)),
                Arg::Lnc(a, c, 5e-324),
            ],
            false,
        ),
    ];
    let mut buf = vec![0.0; 1 << 12];
    let mut scratch = Scratch::borrowed(&mut buf);
    for (args, exact_succeeds) in cases {
        let points = any_points(&args);
        let mark = scratch.mark();
        let exact = points.iter().all(|x| x.exact(&mut scratch).is_ok());
        scratch.reset(mark);
        assert_eq!(exact, exact_succeeds, "{args:?}");
        let (stage, sign) = first_deciding_over_spans(&points);
        assert_eq!(stage, "big_integers", "{args:?}");
        let expected = exact_sign(&args);
        assert_eq!(sign, expected, "{args:?}");
        assert_every_call(&args, expected);
    }
}

/// The explicit points whose coordinate `k`, for `k = 3 i + axis`, is
/// `f64::MAX` when bit `k` of `large` is set and the least subnormal
/// otherwise, negated when bit `k` of `negative` is set.
fn extremes(large: u32, negative: u32) -> Points {
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
/// subnormal: alternating, under every pattern of signs of the twelve
/// coordinates; and in every arrangement, with the last point of the
/// opposite sign to the others, so that no difference cancels. The
/// differences and their products then span the whole range of `f64`, the
/// worst case of the arena of the stage on big integers. An arena of
/// exactly `BIG_INTEGERS_EXPLICIT_ARENA` slots suffices for each, and the
/// sign is that of the oracle.
#[test]
fn the_arena_of_the_explicit_stage_is_enough() {
    let alternating = [0b0101_0101_0101, 0b1010_1010_1010];
    let mut calls: Vec<(u32, u32)> = alternating
        .into_iter()
        .flat_map(|large| (0..1 << 12).map(move |negative| (large, negative)))
        .collect();
    calls.extend((0..1 << 12).map(|large| (large, 0b1110_0000_0000)));
    let mut high_water = 0;
    for (large, negative) in calls {
        let p = extremes(large, negative);
        let mut buf = vec![0.0; BIG_INTEGERS_EXPLICIT_ARENA];
        let mut scratch = Scratch::borrowed(&mut buf);
        let [a, b, c, d] = &p;
        let sign = orient3d_in(&mut scratch, a, b, c, d);
        assert_eq!(sign, Ok(exact(&p)), "{p:?}");
        assert_eq!(scratch.used(), 0);
        high_water = high_water.max(scratch.high_water());
    }
    println!("high water of the arena: {high_water} of {BIG_INTEGERS_EXPLICIT_ARENA} slots");
    // The worst case found, well within the constant: a borrowed arena of
    // `BIG_INTEGERS_EXPLICIT_ARENA` slots fails by itself above it, so this
    // pins that these inputs still reach the stage and use what they did.
    assert_eq!(high_water, 1216);
}

/// `orient3d_in` on calls that reach the stage on big integers, with
/// explicit points and with points on a line, leaves what the arena held
/// as it was, both when it decides and when the arena runs out.
#[test]
fn orient3d_in_leaves_the_arena_as_it_was() {
    let far = [
        [f64::MAX, 5e-324, -1.0],
        [-5e-324, f64::MAX, 2.0],
        [1.0, -f64::MAX, 5e-324],
        [0.5, 0.25, f64::MAX],
    ];
    let (p, q) = ([1.0, -1.0, 0.0], [0.0, 1.0, -1.0]);
    let explicit = far.map(Arg::Explicit);
    let lines = [
        Arg::Explicit(p),
        Arg::Explicit([1.0, 0.0, -1.0]),
        Arg::Lnc(p, q, pow2(-600)),
        Arg::Lnc([f64::MAX, 1.0, -1.0], [-f64::MAX, 0.0, 5e-324], 0.5),
    ];
    for args in [explicit, lines] {
        let points = any_points(&args);
        assert_eq!(first_deciding_over_spans(&points).0, "big_integers");
        let expected = exact_sign(&args);
        let [a, b, c, d] = points;
        for size in [1 << 14, 64] {
            let mut buf = vec![0.0; size];
            let mut scratch = Scratch::borrowed(&mut buf);
            let held = scratch.push(&[0.5, 4.0]).unwrap();
            let used = scratch.used();
            let result = orient3d_in(&mut scratch, &a, &b, &c, &d);
            if size == 64 {
                assert!(result.is_err(), "{args:?}: {result:?}");
            } else {
                assert_eq!(result, Ok(expected), "{args:?}");
            }
            assert_eq!(scratch.used(), used, "{args:?}");
            assert_eq!(scratch.get(held), [0.5, 4.0], "{args:?}");
        }
    }
}
