//! `orient3d` on every combination of explicit points, points on a line and
//! points on a triangle, against the exact sign: every stage alone and the
//! chain, on `AnyPoint`s and on the concrete types; the degenerate cases;
//! and inputs that are not finite, which panic.

// The tests that cannot run on wasm32 leave their helpers unused there.
#![cfg_attr(target_arch = "wasm32", allow(dead_code, unused_imports))]
// `orient3d` needs the `std` feature.
#![cfg(feature = "std")]

mod common;

use common::args::{
    Arg, Args, Orient3d, Orient3dIn, all_explicit, decidable, decided_on_expansions, every_stage,
    exact_sign, spans_decidable, with_concrete,
};
use common::{Points, edges};
use implicit_predicates::stages::orient3d as stages;
use implicit_predicates::{Bpt, Sign};
use implicit_predicates_oracle::Dyadic;
use implicit_predicates_test_support::{
    bpt_params, config, coplanar4, extreme4, grid4, lnc_params, near_coplanar4, point3, random4,
    sample,
};
use proptest::prelude::*;

/// The names of [`every_stage`] that run the whole chain: they must all
/// give the same result.
const CHAIN: [&str; 6] = [
    "staged_in",
    "orient3d",
    "orient3d_in",
    "orient3d, concrete",
    "orient3d, arrays",
    "orient3d_in, concrete",
];

/// The names of [`every_stage`] that decide whenever the stages over spans
/// are documented to.
const OVER_SPANS: [&str; 1] = ["expansions_translated_in"];

/// The ends of the ranges of the stages on the stack, of the filter and of
/// the stage over spans.
fn span_edges() -> Vec<f64> {
    let t = stages::SPANS_TRANSLATED_RANGE;
    let mut edges = edges();
    edges.extend([t.0, t.1]);
    edges
}

/// The arguments with an explicit point where bit `i` of `mask` is clear
/// and a point on a line where it is set.
fn combine(mask: u8, explicit: Points, lines: [(Points, f64); 4]) -> Args {
    std::array::from_fn(|i| {
        if mask & (1 << i) == 0 {
            Arg::Explicit(explicit[i])
        } else {
            let ([p, q, ..], t) = lines[i];
            Arg::Lnc(p, q, t)
        }
    })
}

/// Unrelated points: explicit points in `[-1, 1)^3` and points on lines
/// with the parameters of [`lnc_params`].
fn random_args() -> impl Strategy<Value = Args> {
    let lnc = || lnc_params().prop_map(|(p, q, t)| ([p, q, p, q], t));
    (0u8..16, random4(), [lnc(), lnc(), lnc(), lnc()])
        .prop_map(|(mask, explicit, lines)| combine(mask, explicit, lines))
}

/// Unrelated points: explicit points in `[-1, 1)^3` and points on triangles
/// with the parents and parameters of `bpt_params`.
fn random_bpt_args() -> impl Strategy<Value = Args> {
    let triangle = || bpt_params().boxed();
    (
        0u8..16,
        random4(),
        [triangle(), triangle(), triangle(), triangle()],
    )
        .prop_map(|(mask, explicit, triangles)| {
            std::array::from_fn(|i| {
                let (p, q, r, u, v) = triangles[i];
                if mask & (1 << i) == 0 {
                    Arg::Explicit(explicit[i])
                } else {
                    Arg::Bpt(p, q, r, u, v)
                }
            })
        })
}

/// A parameter of a point on a line: in `[0, 1]`, at its ends, tiny, or
/// outside it.
fn parameter() -> impl Strategy<Value = f64> {
    prop_oneof![
        3 => 0.0..=1.0f64,
        1 => prop::sample::select(vec![0.0, -0.0, 1.0, 0.5, 2f64.powi(-60), 1.0f64.next_down()]),
        1 => -2.0..2.0f64,
    ]
}

/// Points built from the four points of `base`: argument `i` is `base[i]`,
/// or a point on the line through two points of `base`, so that the
/// arguments keep the relationship of `base` (coplanar, nearly coplanar...)
/// up to the rounding of none of them.
fn on_lines_of(base: impl Strategy<Value = Points>) -> impl Strategy<Value = Args> {
    let line = || (0usize..4, 0usize..4, parameter());
    (0u8..16, base, [line(), line(), line(), line()]).prop_map(|(mask, base, lines)| {
        let lines = lines.map(|(j, k, t)| ([base[j], base[k], base[j], base[k]], t));
        combine(mask, base, lines)
    })
}

/// A parameter of a point on a triangle: a short dyadic number, as in a
/// mesh, or any of [`parameter`].
fn triangle_parameter() -> impl Strategy<Value = f64> {
    prop_oneof![
        3 => prop::sample::select(vec![0.0, 1.0, 0.5, 0.25, 0.75, 1.0 / 256.0, 255.0 / 256.0]),
        1 => parameter(),
    ]
}

/// Points built from the four points of `base`, which stay the parents:
/// argument `i` is `base[i]`, a point on the line through two points of
/// `base` or a point on the triangle with three of them as corners, with
/// parameters of a mesh or any, so that the arguments keep the relationship
/// of `base` and share their parents with each other.
fn on_triangles_of(base: impl Strategy<Value = Points>) -> impl Strategy<Value = Args> {
    let one = || {
        (
            0u8..3,
            [0usize..4, 0usize..4, 0usize..4],
            [triangle_parameter(), triangle_parameter()],
        )
            .boxed()
    };
    (base, [one(), one(), one(), one()]).prop_map(|(base, kinds)| {
        std::array::from_fn(|i| {
            let (kind, [j, k, l], [u, v]) = kinds[i];
            match kind {
                0 => Arg::Explicit(base[i]),
                1 => Arg::Lnc(base[j], base[k], u),
                _ => Arg::Bpt(base[j], base[k], base[l], u, v),
            }
        })
    })
}

/// Every family of arguments.
fn any_args() -> impl Strategy<Value = Args> {
    prop_oneof![
        random_args().boxed(),
        on_lines_of(random4()).boxed(),
        on_lines_of(coplanar4()).boxed(),
        on_lines_of(near_coplanar4()).boxed(),
        on_lines_of(grid4()).boxed(),
        on_lines_of(extreme4(&span_edges())).boxed(),
        random_bpt_args().boxed(),
        on_triangles_of(random4()).boxed(),
        on_triangles_of(coplanar4()).boxed(),
        on_triangles_of(near_coplanar4()).boxed(),
        on_triangles_of(grid4()).boxed(),
        on_triangles_of(extreme4(&span_edges())).boxed(),
    ]
}

/// Checks every stage on `args`, whose inputs are finite, against the exact
/// sign, and that the chain decides wherever it is documented to.
fn check(args: &Args) -> Result<(), TestCaseError> {
    let expected = exact_sign(args);
    let results = every_stage(args);
    for (name, result) in &results {
        if let Some(sign) = result {
            prop_assert_eq!(*sign, expected, "{} on {:?}", name, args);
        }
    }
    let result = |name: &str| results.iter().find(|(n, _)| *n == name).unwrap().1;
    let chain = result("staged_in");
    for name in CHAIN {
        prop_assert_eq!(result(name), chain, "{} on {:?}", name, args);
    }
    // Every call decides; without the stage on big integers, at least
    // within the documented range.
    prop_assert_eq!(chain, Some(expected), "{:?}", args);
    if decidable(args) {
        prop_assert!(decided_on_expansions(&results), "{:?}", args);
    }
    if spans_decidable(args) {
        for name in OVER_SPANS {
            prop_assert!(result(name).is_some(), "{} on {:?}", name, args);
        }
    }
    // The filter gives a zero only on a flat axis.
    if result("filter_points") == Some(Sign::Zero) {
        let approx =
            common::args::any_points(args).map(|p| *implicit_predicates::Point::approx(&p));
        prop_assert!(
            (0..3).any(|k| approx.iter().all(|p| p[k] == approx[0][k])),
            "{:?}",
            args
        );
    }
    Ok(())
}

/// `args` with argument `i` replaced by `arg`.
fn replaced(args: &Args, i: usize, arg: Arg) -> Args {
    let mut args = *args;
    args[i] = arg;
    args
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn every_stage_is_exact_on_every_combination(args in any_args()) {
        check(&args)?;
    }

    #[test]
    fn the_ends_of_the_parameter_are_the_parents(
        args in any_args(),
        i in 0usize..4,
        (p, q) in (point3(), point3()),
    ) {
        // `t = 0` is `q` and `t = 1` is `p`, exactly.
        for (t, parent) in [(0.0, q), (-0.0, q), (1.0, p)] {
            let line = replaced(&args, i, Arg::Lnc(p, q, t));
            let point = replaced(&args, i, Arg::Explicit(parent));
            prop_assert_eq!(exact_sign(&line), exact_sign(&point));
            check(&line)?;
            let on_line = implicit_predicates::orient3d(
                &line[0].any(), &line[1].any(), &line[2].any(), &line[3].any(),
            );
            let explicit = implicit_predicates::orient3d(
                &point[0].any(), &point[1].any(), &point[2].any(), &point[3].any(),
            );
            prop_assert_eq!(on_line, explicit, "{:?} {:?}", line, point);
        }
    }

    #[test]
    fn a_point_on_a_line_through_two_other_arguments_is_coplanar(
        base in prop_oneof![random4(), grid4(), extreme4(&span_edges())],
        (i, j, k) in (0usize..4, 0usize..3, 0usize..3),
        t in parameter(),
        lines in 0u8..8,
    ) {
        // The other three arguments, explicit or on a line through
        // themselves; argument `i` on the line through two of them, its
        // parents shared with other arguments.
        let others: Vec<usize> = (0..4).filter(|&n| n != i).collect();
        let mut args: Args = base.map(Arg::Explicit);
        for (bit, &n) in others.iter().enumerate() {
            if lines & (1 << bit) != 0 {
                args[n] = Arg::Lnc(base[n], base[n], t);
            }
        }
        args[i] = Arg::Lnc(base[others[j]], base[others[k]], t);
        prop_assert_eq!(exact_sign(&args), Sign::Zero, "{:?}", args);
        check(&args)?;
    }

    #[test]
    fn four_points_on_lines_of_a_plane_are_coplanar(args in on_lines_of(coplanar4())) {
        prop_assert_eq!(exact_sign(&args), Sign::Zero, "{:?}", args);
        check(&args)?;
    }

    #[test]
    fn four_points_on_triangles_of_a_plane_are_coplanar(args in on_triangles_of(coplanar4())) {
        prop_assert_eq!(exact_sign(&args), Sign::Zero, "{:?}", args);
        check(&args)?;
    }

    #[test]
    fn a_point_on_a_triangle_of_three_other_arguments_is_coplanar(
        base in prop_oneof![random4(), grid4(), extreme4(&span_edges())],
        i in 0usize..4,
        corners in [0usize..3, 0usize..3, 0usize..3],
        (u, v) in (triangle_parameter(), triangle_parameter()),
        t in parameter(),
        lines in 0u8..8,
    ) {
        // The other three arguments, explicit or on a line through
        // themselves; argument `i` on the triangle with them as its corners,
        // in any order and any number of times, with any parameters, inside
        // the triangle or not: its parents are other arguments.
        let others: Vec<usize> = (0..4).filter(|&n| n != i).collect();
        let mut args: Args = base.map(Arg::Explicit);
        for (bit, &n) in others.iter().enumerate() {
            if lines & (1 << bit) != 0 {
                args[n] = Arg::Lnc(base[n], base[n], t);
            }
        }
        let [p, q, r] = corners.map(|c| base[others[c]]);
        args[i] = Arg::Bpt(p, q, r, u, v);
        prop_assert_eq!(exact_sign(&args), Sign::Zero, "{:?}", args);
        check(&args)?;
    }

    #[test]
    fn the_vertices_of_the_triangle_are_the_parents(
        args in any_args(),
        i in 0usize..4,
        (p, q, r) in (point3(), point3(), point3()),
    ) {
        // `(u, v) = (1, 0)` is `p`, `(0, 1)` is `q` and `(0, 0)` is `r`,
        // exactly.
        for (u, v, parent) in [(1.0, 0.0, p), (0.0, 1.0, q), (0.0, 0.0, r), (-0.0, -0.0, r)] {
            let on_triangle = replaced(&args, i, Arg::Bpt(p, q, r, u, v));
            let point = replaced(&args, i, Arg::Explicit(parent));
            prop_assert_eq!(exact_sign(&on_triangle), exact_sign(&point));
            check(&on_triangle)?;
            let on_triangle_sign = implicit_predicates::orient3d(
                &on_triangle[0].any(), &on_triangle[1].any(), &on_triangle[2].any(), &on_triangle[3].any(),
            );
            let explicit = implicit_predicates::orient3d(
                &point[0].any(), &point[1].any(), &point[2].any(), &point[3].any(),
            );
            prop_assert_eq!(on_triangle_sign, explicit, "{:?} {:?}", on_triangle, point);
        }
    }

    /// `p`, `q`, `r` and a point on their triangle are coplanar for any
    /// `u` and `v`, in any order of the arguments, whether the point is
    /// built with `Bpt::new` (when it accepts the parameters) or with
    /// `Bpt::new_unchecked`, and for every concrete type of the arguments.
    #[test]
    fn four_points_of_one_triangle_are_coplanar(
        (p, q, r, u, v) in bpt_params(),
        order in Just(vec![0usize, 1, 2, 3]).prop_shuffle(),
    ) {
        let args: Args = std::array::from_fn(|i| match order[i] {
            0 => Arg::Explicit(p),
            1 => Arg::Explicit(q),
            2 => Arg::Explicit(r),
            _ => Arg::Bpt(p, q, r, u, v),
        });
        prop_assert_eq!(exact_sign(&args), Sign::Zero, "{:?}", args);
        check(&args)?;
        // The point made by `new`, when it accepts the parameters.
        if let Ok(point) = Bpt::new(p, q, r, u, v) {
            let (a, b, c) = (&p, &q, &r);
            let signs = [
                implicit_predicates::orient3d(a, b, c, &point),
                implicit_predicates::orient3d(&point, a, b, c),
                implicit_predicates::orient3d(a, &point, c, b),
                implicit_predicates::orient3d(c, b, &point, a),
            ];
            prop_assert_eq!(signs, [Sign::Zero; 4], "{:?}", args);
        }
    }

    #[test]
    fn a_point_between_far_parents_in_the_plane_is_coplanar(
        base in coplanar4(),
        (i, j, k) in (0usize..4, 0usize..4, 1usize..4),
        far in 10i32..=30,
        t in parameter(),
    ) {
        // Parents `2^far` times as far from `base[j]` as `base[k]`, on the
        // line through both, so the point is near the others while the
        // error of its approximation is about `2^far` times theirs: the
        // filter must take that error into account.
        let k = (j + k) % 4;
        let scale = 2f64.powi(far);
        let p: [f64; 3] = std::array::from_fn(|n| base[j][n] + scale * (base[k][n] - base[j][n]));
        let q: [f64; 3] = std::array::from_fn(|n| base[j][n] - scale * (base[k][n] - base[j][n]));
        // The parents are exactly on the line: the coordinates of `base` are
        // small multiples of a power of two, so this holds unless their
        // magnitudes are far apart.
        for n in 0..3 {
            let [a, b, s] = [base[j][n], base[k][n], scale].map(Dyadic::from_f64);
            let step = &s * &(&b - &a);
            prop_assume!(Dyadic::from_f64(p[n]) == &a + &step);
            prop_assume!(Dyadic::from_f64(q[n]) == &a - &step);
        }
        let mut args: Args = base.map(Arg::Explicit);
        args[i] = Arg::Lnc(p, q, t);
        prop_assert_eq!(exact_sign(&args), Sign::Zero, "{:?}", args);
        check(&args)?;
    }

    #[test]
    fn negative_zero_and_repeated_arguments(
        args in any_args(),
        from in 0usize..4,
        to in 0usize..4,
    ) {
        // Negative zero is zero, for every stage.
        let flipped = args.map(|a| {
            a.with_inputs(&a.inputs().iter().map(|&x| if x == 0.0 { -x } else { x }).collect::<Vec<_>>())
        });
        prop_assert_eq!(every_stage(&flipped), every_stage(&args), "{:?}", args);
        // A repeated argument makes the points coplanar.
        if from != to {
            let repeated = replaced(&args, to, args[from]);
            prop_assert_eq!(exact_sign(&repeated), Sign::Zero);
            check(&repeated)?;
        }
    }

    // Excluded on wasm32: panics abort on wasm32, so they cannot be caught there.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn inputs_that_are_not_finite_panic(
        args in any_args(),
        bad in prop::sample::select(vec![f64::NAN, -f64::NAN, f64::INFINITY, f64::NEG_INFINITY]),
    ) {
        for i in 0..4 {
            let inputs = args[i].inputs();
            for slot in 0..inputs.len() {
                let mut changed = inputs.clone();
                changed[slot] = bad;
                let args = replaced(&args, i, args[i].with_inputs(&changed));
                let [a, b, c, d] = common::args::any_points(&args);
                prop_assert_eq!(stages::filter_points(&a, &b, &c, &d), None, "{:?}", args);
                for raw in [false, true] {
                    let messages = [
                        panic_message(|| {
                            with_concrete(&args, raw, &mut Orient3d);
                        }),
                        panic_message(|| {
                            with_concrete(&args, raw, &mut Orient3dIn);
                        }),
                    ];
                    for message in messages {
                        prop_assert!(
                            message.as_deref().is_some_and(|m| m.contains("not finite")),
                            "{:?} (raw: {}): {:?}", args, raw, message
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_point_whose_approximation_overflows_has_the_exact_sign(
        args in any_args(),
        i in 0usize..4,
        t in prop::sample::select(vec![0.0, 0.5, 1.0, 0.25]),
        big in prop::sample::select(vec![f64::MAX, 1e308, 2f64.powi(1023)]),
    ) {
        // The difference of the parents overflows, so the approximation is
        // not finite even where the point is, as at `t = 0` or `t = 1`; and
        // the parents are beyond the range of the exact arithmetic over
        // expansions, so only the stage on big integers decides.
        for (p, q) in [([big; 3], [-big; 3]), ([big, 0.0, 1.0], [-big, 0.0, 1.0])] {
            let args = replaced(&args, i, Arg::Lnc(p, q, t));
            let results = every_stage(&args);
            prop_assert!(!decided_on_expansions(&results), "{:?}", args);
            check(&args)?;
        }
        // The same on a triangle, whose corners differ by more than the
        // greatest `f64`.
        let args = replaced(&args, i, Arg::Bpt([big; 3], [-big; 3], [-big; 3], t, 0.25));
        let results = every_stage(&args);
        prop_assert!(!decided_on_expansions(&results), "{:?}", args);
        check(&args)?;
    }
}

/// The message of the panic of `f`, or `None` if it does not panic.
fn panic_message(f: impl FnOnce()) -> Option<String> {
    let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).err()?;
    Some(
        payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_string()))
            .unwrap_or_default(),
    )
}

/// The points of the triangle of three explicit points, `Bpt::new` with
/// parameters of a mesh, and `orient3d` of them with the corners in every
/// order: zero.
#[test]
fn a_point_made_by_new_is_coplanar_with_its_corners_in_every_order() {
    let (p, q, r) = ([1.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 4.0]);
    let parameters = [
        (0.0, 0.0),
        (1.0, 0.0),
        (0.0, 1.0),
        (0.5, 0.5),
        (0.25, 0.25),
        (1.0 / 256.0, 255.0 / 256.0),
        (0.1, 0.2),
    ];
    for (u, v) in parameters {
        let point = Bpt::new(p, q, r, u, v).expect("parameters in the triangle");
        for (a, b, c) in [(&p, &q, &r), (&q, &r, &p), (&r, &p, &q), (&q, &p, &r)] {
            assert_eq!(implicit_predicates::orient3d(a, b, c, &point), Sign::Zero);
            assert_eq!(implicit_predicates::orient3d(&point, a, b, c), Sign::Zero);
            assert_eq!(implicit_predicates::orient3d(a, &point, b, c), Sign::Zero);
            assert_eq!(implicit_predicates::orient3d(a, b, &point, c), Sign::Zero);
        }
    }
}

/// Every one of the 16 combinations on the same nearly degenerate and
/// coplanar bases, so that none is left to chance.
#[test]
fn every_combination_on_fixed_bases() {
    let bases: Vec<Points> = sample(coplanar4(), 20)
        .into_iter()
        .chain(sample(near_coplanar4(), 20))
        .chain(sample(grid4(), 20))
        .collect();
    let mut explicit_points = 0;
    for (n, base) in bases.iter().enumerate() {
        for mask in 0u8..16 {
            let lines = std::array::from_fn(|i| {
                let (j, k) = ((i + 1 + n) % 4, (i + 2) % 4);
                ([base[j], base[k], base[j], base[k]], 0.375)
            });
            let args = combine(mask, *base, lines);
            explicit_points += usize::from(all_explicit(&args));
            check(&args).unwrap();
        }
    }
    assert_eq!(explicit_points, bases.len());
}
