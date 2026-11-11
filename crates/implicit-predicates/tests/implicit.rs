//! `orient3d` on every combination of explicit points and points on a line,
//! against the exact sign: every stage alone and the chain, on `AnyPoint`s
//! and on the concrete types; the degenerate cases; and inputs that are not
//! finite.

// `orient3d` needs the `std` feature.
#![cfg(feature = "std")]

mod common;

use common::args::{Arg, Args, all_explicit, decidable, every_stage, exact_sign, spans_decidable};
use common::{Points, edges};
use implicit_predicates::Sign;
use implicit_predicates::stages::orient3d as stages;
use implicit_predicates_oracle::Dyadic;
use implicit_predicates_test_support::{
    config, coplanar4, extreme4, grid4, lnc_params, near_coplanar4, point3, random4, sample,
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

/// The names of [`every_stage`] that run every stage over spans.
const OVER_SPANS: [&str; 1] = ["exact_stages_in"];

/// The ends of the ranges of the stages on the stack, of the filter and of
/// the stages over spans.
fn span_edges() -> Vec<f64> {
    let (t, u) = (
        stages::SPANS_TRANSLATED_RANGE,
        stages::SPANS_UNTRANSLATED_RANGE,
    );
    let mut edges = edges();
    edges.extend([t.0, t.1, u.0, u.1]);
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

/// Every family of arguments.
fn any_args() -> impl Strategy<Value = Args> {
    prop_oneof![
        random_args().boxed(),
        on_lines_of(random4()).boxed(),
        on_lines_of(coplanar4()).boxed(),
        on_lines_of(near_coplanar4()).boxed(),
        on_lines_of(grid4()).boxed(),
        on_lines_of(extreme4(&span_edges())).boxed(),
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
    // `None` only outside the documented range.
    if decidable(args) {
        prop_assert!(chain.is_some(), "no sign for {:?}", args);
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
            if let (Some(a), Some(b)) = (on_line, explicit) {
                prop_assert_eq!(a, b, "{:?} {:?}", line, point);
            }
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

    #[test]
    fn inputs_that_are_not_finite_give_none(
        args in any_args(),
        bad in prop::sample::select(vec![f64::NAN, -f64::NAN, f64::INFINITY, f64::NEG_INFINITY]),
    ) {
        for i in 0..4 {
            let inputs = args[i].inputs();
            for slot in 0..inputs.len() {
                let mut changed = inputs.clone();
                changed[slot] = bad;
                let args = replaced(&args, i, args[i].with_inputs(&changed));
                for (name, result) in every_stage(&args) {
                    prop_assert_eq!(result, None, "{} on {:?}", name, args);
                }
            }
        }
    }

    #[test]
    fn a_point_whose_approximation_overflows_gives_none(
        args in any_args(),
        i in 0usize..4,
        t in prop::sample::select(vec![0.0, 0.5, 1.0, 0.25]),
        big in prop::sample::select(vec![f64::MAX, 1e308, 2f64.powi(1023)]),
    ) {
        // The difference of the parents overflows, so the approximation is
        // not finite even where the point is, as at `t = 0` or `t = 1`; and
        // the parents are beyond the range of the exact arithmetic.
        for (p, q) in [([big; 3], [-big; 3]), ([big, 0.0, 1.0], [-big, 0.0, 1.0])] {
            let args = replaced(&args, i, Arg::Lnc(p, q, t));
            for (name, result) in every_stage(&args) {
                prop_assert_eq!(result, None, "{} on {:?}", name, args);
            }
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
