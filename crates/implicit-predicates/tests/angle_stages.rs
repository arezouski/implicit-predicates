//! The stages of `dot_sign_3d` and `dihedral_sign` against oracles that do
//! not come from their definitions, `dot_sign_3d` and `dihedral_sign` of
//! `implicit_predicates_corpus::exact`, on explicit points: their constants
//! against the derivation of the filter, the stages that are emitted, each
//! stage alone on random, grid, repeated, extreme, right and nearly right
//! configurations, the stages on expansions in their ranges, and the edge
//! cases of the generated checks.

mod common;

#[path = "generated/dot_sign_3d.rs"]
#[allow(dead_code)]
mod dot_checks;

#[path = "generated/dihedral_sign.rs"]
#[allow(dead_code)]
mod dihedral_checks;

use common::{in_range, is_derived, pow2, sign_of};
use implicit_predicates::stages::{dihedral_sign as dihedral, dot_sign_3d as dot};
use implicit_predicates::{BigCoords, ExactCoords, Point, Scratch, ScratchExhausted, Sign};
use implicit_predicates_corpus::exact;
use implicit_predicates_oracle::Dyadic;
use implicit_predicates_test_support::{
    Points4, config, extreme4, grid4, random4, repeated4, sample, signed_zeros4,
};
use proptest::prelude::*;

/// Three points in space.
type Points3 = [[f64; 3]; 3];

/// The slots of the arena of the stages that take one. Much more than the
/// stages need on explicit points.
const ARENA: usize = 1 << 16;

/// Ends a call of the tests whose arena runs out.
fn room<T>(e: ScratchExhausted) -> T {
    panic!("the arena is too small: {e:?}")
}

/// The sign of `dot_sign_3d` at the explicit points `p`, by the oracle.
fn dot_oracle(p: &Points3) -> Sign {
    let [p, q, r] = p.map(|x| x.map(Dyadic::from_f64));
    sign_of(exact::dot_sign_3d(&p, &q, &r))
}

/// The sign of `dihedral_sign` at the explicit points `p`, by the oracle.
fn dihedral_oracle(p: &Points4) -> Sign {
    let [a, b, c, d] = p.map(|x| x.map(Dyadic::from_f64));
    sign_of(exact::dihedral_sign(&a, &b, &c, &d))
}

/// The points as the stages take them.
fn refs<const N: usize>(p: &[[f64; 3]; N]) -> [&[f64; 3]; N] {
    std::array::from_fn(|i| &p[i])
}

/// Runs `f` in `scratch` and resets it to its mark on entry.
fn in_mark<T>(scratch: &mut Scratch<'_>, f: impl FnOnce(&mut Scratch<'_>) -> T) -> T {
    let mark = scratch.mark();
    let result = f(scratch);
    scratch.reset(mark);
    result
}

/// A stage over spans on the exact coordinates of the points, written to
/// `scratch`; `None` when a point cannot give them or the stage declines.
fn over_spans<const N: usize>(
    scratch: &mut Scratch<'_>,
    p: [&[f64; 3]; N],
    stage: fn(&mut Scratch<'_>, [&ExactCoords; N]) -> Result<Option<Sign>, ScratchExhausted>,
) -> Option<Sign> {
    in_mark(scratch, |scratch| {
        let q: Vec<ExactCoords> = p
            .iter()
            .map(|x| x.exact(scratch).ok())
            .collect::<Option<_>>()?;
        stage(scratch, std::array::from_fn(|i| &q[i])).unwrap_or_else(room)
    })
}

/// A stage on big integers on the coordinates of the points as big
/// integers, written to `scratch`; `None` only when a point cannot give
/// them, when an input is not finite.
fn on_big_integers<const N: usize>(
    scratch: &mut Scratch<'_>,
    p: [&[f64; 3]; N],
    stage: fn(&mut Scratch<'_>, [&BigCoords; N]) -> Result<Sign, ScratchExhausted>,
) -> Option<Sign> {
    in_mark(scratch, |scratch| {
        let q: Vec<BigCoords> = p
            .iter()
            .map(|x| x.exact_big(scratch).ok())
            .collect::<Option<_>>()?;
        Some(stage(scratch, std::array::from_fn(|i| &q[i])).unwrap_or_else(room))
    })
}

/// A stage of a predicate on `N` points, as a function of an arena and the
/// points.
type Stage<const N: usize> = fn(&mut Scratch<'_>, [&[f64; 3]; N]) -> Option<Sign>;

/// A stage of `dot_sign_3d`.
type DotStage = Stage<3>;

/// Every emitted stage of `dot_sign_3d`, and every chain of them, by name.
const DOT_STAGES: [(&str, DotStage); 10] = [
    ("filter", |_, p| dot::filter(p, 0.0)),
    ("filter_axes", |_, p| dot::filter_axes(p, [0.0; 3])),
    ("exact_differences", |_, p| dot::exact_differences(p)),
    ("expansions_translated", |_, p| {
        dot::expansions_translated(p)
    }),
    ("expansions_translated_in", |s, p| {
        over_spans(s, p, dot::expansions_translated_in)
    }),
    ("big_integers_in", |s, p| {
        on_big_integers(s, p, dot::big_integers_in)
    }),
    ("staged", |_, p| dot::staged(p)),
    ("exact_stages", |_, p| dot::exact_stages(p)),
    ("exact_stages_explicit_in", |s, p| {
        Some(dot::exact_stages_explicit_in(s, p).unwrap_or_else(room))
    }),
    ("staged_in", |s, p| {
        Some(dot::staged_in(s, p[0], p[1], p[2]).unwrap_or_else(room))
    }),
];

/// A stage of `dihedral_sign`.
type DihedralStage = Stage<4>;

/// Every emitted stage of `dihedral_sign`, and every chain of them, by
/// name. It has no stage on expansions with translation on the stack.
const DIHEDRAL_STAGES: [(&str, DihedralStage); 9] = [
    ("filter", |_, p| dihedral::filter(p, 0.0)),
    ("filter_axes", |_, p| dihedral::filter_axes(p, [0.0; 3])),
    ("exact_differences", |_, p| dihedral::exact_differences(p)),
    ("expansions_translated_in", |s, p| {
        over_spans(s, p, dihedral::expansions_translated_in)
    }),
    ("big_integers_in", |s, p| {
        on_big_integers(s, p, dihedral::big_integers_in)
    }),
    ("staged", |s, p| dihedral::staged(s, p).unwrap_or_else(room)),
    ("exact_stages", |s, p| {
        dihedral::exact_stages(s, p).unwrap_or_else(room)
    }),
    ("exact_stages_explicit_in", |s, p| {
        Some(dihedral::exact_stages_explicit_in(s, p).unwrap_or_else(room))
    }),
    ("staged_in", |s, p| {
        Some(dihedral::staged_in(s, p[0], p[1], p[2], p[3]).unwrap_or_else(room))
    }),
];

/// The stages that always decide on finite explicit points.
const ALWAYS_DECIDE: [&str; 3] = ["big_integers_in", "exact_stages_explicit_in", "staged_in"];

/// Checks every stage of `stages` on `p` against `expected`: a stage that
/// declines is not a failure, one that decides must give `expected`.
fn check_stages<const N: usize>(
    stages: &[(&str, Stage<N>)],
    p: &[[f64; 3]; N],
    expected: Sign,
) -> Result<(), TestCaseError> {
    let mut buf = vec![0.0; ARENA];
    let mut scratch = Scratch::borrowed(&mut buf);
    for (name, stage) in stages {
        match stage(&mut scratch, refs(p)) {
            Some(sign) => prop_assert_eq!(sign, expected, "{} on {:?}", name, p),
            None => prop_assert!(!ALWAYS_DECIDE.contains(name), "{} on {:?}", name, p),
        }
    }
    Ok(())
}

/// `x` moved by `ulps` units in the last place, up or down.
fn step_ulps(mut x: f64, ulps: i32) -> f64 {
    for _ in 0..ulps.unsigned_abs() {
        x = if ulps > 0 { x.next_up() } else { x.next_down() };
    }
    x
}

/// The power-of-two exponents of the configurations built on integers: the
/// coordinates and their differences stay within the ranges of every stage
/// on expansions of both predicates.
const SCALES: std::ops::RangeInclusive<i32> = -150..=200;

/// `n * 2^k` for each coordinate, exactly, for integers of magnitude below
/// `2^11`.
fn scaled<const N: usize>(p: [[i32; 3]; N], k: i32) -> [[f64; 3]; N] {
    p.map(|x| x.map(|n| f64::from(n) * 2f64.powi(k)))
}

/// The cross product of two vectors of small integers.
fn cross(u: [i32; 3], v: [i32; 3]) -> [i32; 3] {
    [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ]
}

fn add(u: [i32; 3], v: [i32; 3]) -> [i32; 3] {
    std::array::from_fn(|i| u[i] + v[i])
}

/// Small integer vectors.
fn small() -> impl Strategy<Value = [i32; 3]> {
    [-8i32..=8, -8i32..=8, -8i32..=8]
}

/// Integer positions.
fn position() -> impl Strategy<Value = [i32; 3]> {
    [-64i32..=64, -64i32..=64, -64i32..=64]
}

/// Three points with a right angle at the middle one, exactly: `p = q + u`
/// and `r = q + u x w` on a grid of integers times a power of two.
fn right3() -> impl Strategy<Value = Points3> {
    (SCALES, position(), small(), small())
        .prop_map(|(k, q, u, w)| scaled([add(q, u), q, add(q, cross(u, w))], k))
}

/// Four points with a right dihedral angle at the edge `c, d`, exactly: with
/// `A = a - d` and `C = c - d`, `b - d = s C + t (A x C)`, whose cross
/// product with `C` is orthogonal to `A x C`.
fn right4() -> impl Strategy<Value = Points4> {
    (SCALES, position(), small(), small(), -2i32..=2, -2i32..=2).prop_map(|(k, d, a, c, s, t)| {
        let n = cross(a, c);
        let b: [i32; 3] = std::array::from_fn(|i| s * c[i] + t * n[i]);
        scaled([add(d, a), add(d, b), add(d, c), d], k)
    })
}

/// Points of `base` with one to three distinct coordinates each moved by
/// one to three units in the last place, up or down, when they are not
/// zero: a zero moved would be a subnormal, outside the range of every
/// exact stage.
fn nearly<const N: usize>(
    base: impl Strategy<Value = [[f64; 3]; N]>,
) -> impl Strategy<Value = [[f64; 3]; N]> {
    let coordinates = prop::sample::subsequence((0..3 * N).collect::<Vec<usize>>(), 1..=3);
    let ulps = prop_oneof![-3i32..=-1, 1i32..=3];
    (base, coordinates, [ulps.clone(), ulps.clone(), ulps]).prop_map(
        |(mut p, coordinates, ulps)| {
            for (index, ulps) in coordinates.into_iter().zip(ulps) {
                let (point, axis) = (index / 3, index % 3);
                if p[point][axis] != 0.0 {
                    p[point][axis] = step_ulps(p[point][axis], ulps);
                }
            }
            p
        },
    )
}

/// The ends of the ranges of the filter and of the stages on expansions of
/// `dot_sign_3d`.
fn dot_edges() -> Vec<f64> {
    let (d, t, s) = (
        dot::DIFFERENCES_RANGE,
        dot::TRANSLATED_RANGE,
        dot::SPANS_TRANSLATED_RANGE,
    );
    vec![d.0, d.1, t.0, t.1, s.0, s.1, dot::D_MIN, dot::D_MAX]
}

/// The ends of the ranges of the filter and of the stages on expansions of
/// `dihedral_sign`.
fn dihedral_edges() -> Vec<f64> {
    let (d, s) = (
        dihedral::DIFFERENCES_RANGE,
        dihedral::SPANS_TRANSLATED_RANGE,
    );
    vec![d.0, d.1, s.0, s.1, dihedral::D_MIN, dihedral::D_MAX]
}

/// The first three of four points.
fn first3(p: Points4) -> Points3 {
    [p[0], p[1], p[2]]
}

/// Every family of three points.
fn any3() -> impl Strategy<Value = Points3> {
    prop_oneof![
        random4().prop_map(first3),
        grid4().prop_map(first3),
        repeated4().prop_map(first3),
        signed_zeros4().prop_map(first3),
        extreme4(&dot_edges()).prop_map(first3),
        right3(),
        nearly(right3()),
    ]
}

/// Every family of four points.
fn any4() -> impl Strategy<Value = Points4> {
    prop_oneof![
        random4(),
        grid4(),
        repeated4(),
        signed_zeros4(),
        extreme4(&dihedral_edges()),
        right4(),
        nearly(right4()),
    ]
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn every_stage_of_dot_sign_3d_is_exact_when_it_decides(p in any3()) {
        check_stages(&DOT_STAGES, &p, dot_oracle(&p))?;
    }

    #[test]
    fn every_stage_of_dihedral_sign_is_exact_when_it_decides(p in any4()) {
        check_stages(&DIHEDRAL_STAGES, &p, dihedral_oracle(&p))?;
    }

    #[test]
    fn right_angles_are_zero_and_nearly_right_ones_are_decided(
        p3 in right3(),
        p4 in right4(),
        q3 in nearly(right3()),
        q4 in nearly(right4()),
    ) {
        prop_assert_eq!(dot_oracle(&p3), Sign::Zero);
        prop_assert_eq!(dihedral_oracle(&p4), Sign::Zero);
        // The exact differences, on the stack, decide the right ones; the
        // stages on expansions decide the moved ones, whose differences may
        // not be exact: on the stack for `dot_sign_3d`, in an arena for
        // `dihedral_sign`.
        prop_assert_eq!(dot::exact_differences(refs(&p3)), Some(Sign::Zero));
        prop_assert_eq!(dihedral::exact_differences(refs(&p4)), Some(Sign::Zero));
        prop_assert_eq!(dot::expansions_translated(refs(&q3)), Some(dot_oracle(&q3)));
        let mut buf = vec![0.0; ARENA];
        let mut scratch = Scratch::borrowed(&mut buf);
        prop_assert_eq!(
            over_spans(&mut scratch, refs(&q4), dihedral::expansions_translated_in),
            Some(dihedral_oracle(&q4))
        );
    }

    #[test]
    fn the_stages_on_expansions_decide_in_their_ranges(
        p3 in prop_oneof![grid4(), signed_zeros4()].prop_map(first3),
        p4 in prop_oneof![grid4(), signed_zeros4()],
    ) {
        // The differences of grid points are exact, and every coordinate
        // and difference is in the ranges.
        for (x, range) in [
            (p3.as_flattened(), dot::TRANSLATED_RANGE),
            (p3.as_flattened(), dot::SPANS_TRANSLATED_RANGE),
            (p4.as_flattened(), dihedral::SPANS_TRANSLATED_RANGE),
        ] {
            prop_assert!(x.iter().all(|&x| in_range(x, range)), "{:?}", x);
        }
        let (s3, s4) = (dot_oracle(&p3), dihedral_oracle(&p4));
        prop_assert_eq!(dot::exact_differences(refs(&p3)), Some(s3));
        prop_assert_eq!(dot::expansions_translated(refs(&p3)), Some(s3));
        prop_assert_eq!(dihedral::exact_differences(refs(&p4)), Some(s4));
        let mut buf = vec![0.0; ARENA];
        let mut scratch = Scratch::borrowed(&mut buf);
        prop_assert_eq!(
            over_spans(&mut scratch, refs(&p3), dot::expansions_translated_in),
            Some(s3)
        );
        prop_assert_eq!(
            over_spans(&mut scratch, refs(&p4), dihedral::expansions_translated_in),
            Some(s4)
        );
    }
}

/// How many of `cases` each stage of `stages` decides.
fn decided<const N: usize>(stages: &[(&str, Stage<N>)], cases: &[[[f64; 3]; N]]) -> Vec<usize> {
    let mut buf = vec![0.0; ARENA];
    let mut scratch = Scratch::borrowed(&mut buf);
    stages
        .iter()
        .map(|(_, stage)| {
            cases
                .iter()
                .filter(|p| stage(&mut scratch, refs(p)).is_some())
                .count()
        })
        .collect()
}

/// The families of points reach every stage: on a fixed sample, each stage
/// decides some calls, and every stage but those that always decide
/// declines some, so the properties above check every stage on both
/// outcomes.
#[test]
fn the_families_reach_every_stage() {
    let count = 2000;
    let dot_counts = decided(&DOT_STAGES, &sample(any3(), count));
    let dihedral_counts = decided(&DIHEDRAL_STAGES, &sample(any4(), count));
    let dot_names = DOT_STAGES.iter().map(|(name, _)| *name);
    let dihedral_names = DIHEDRAL_STAGES.iter().map(|(name, _)| *name);
    let all = dot_names
        .zip(dot_counts)
        .chain(dihedral_names.zip(dihedral_counts));
    for (name, n) in all {
        println!("{name}: {n} of {count}");
        assert!(n > 0, "{name}");
        if ALWAYS_DECIDE.contains(&name) {
            assert_eq!(n, count, "{name}");
        } else {
            assert!(n < count, "{name}");
        }
    }
}

const DOT_SOURCE: &str = include_str!("../src/generated/dot_sign_3d.rs");
const DIHEDRAL_SOURCE: &str = include_str!("../src/generated/dihedral_sign.rs");

#[test]
fn the_emitted_constants_of_dot_sign_3d_are_those_of_the_derivation() {
    // The rules of the global bound give epsilon = 14 u and kappa = 6, plus
    // terms of order u, each inflated by `1 + 2^-30` and rounded up; those
    // of the bound by parts, the terms of epsilon 4 u D^(0, 0, 2) and
    // 5 u D^(0, 2, 0) and D^(2, 0, 0), and a term of kappa of 2 on each axis.
    let u = Dyadic::from_f64(pow2(-53));
    let times_u = |n: f64| &Dyadic::from_f64(n) * &u;
    assert_eq!(dot::DEGREE, 2);
    assert!(is_derived(dot::EPSILON, &times_u(14.0)));
    assert!(is_derived(dot::KAPPA, &Dyadic::from_f64(6.0)));
    assert_eq!(dot::EPSILON, 1.5543122359227854e-15);
    assert_eq!(dot::KAPPA, 6.000000005587936);
    let vectors = dot::AXES_EPSILON.map(|(g, _)| g);
    assert_eq!(vectors, [[0, 0, 2], [0, 2, 0], [2, 0, 0]]);
    for ((g, value), n) in dot::AXES_EPSILON.into_iter().zip([4.0, 5.0, 5.0]) {
        assert!(is_derived(value, &times_u(n)), "{g:?}");
    }
    let kappa = [(0, [1, 0, 0]), (1, [0, 1, 0]), (2, [0, 0, 1])];
    assert_eq!(dot::AXES_KAPPA.map(|(k, h, _)| (k, h)), kappa);
    for (k, h, value) in dot::AXES_KAPPA {
        assert!(is_derived(value, &Dyadic::from_f64(2.0)), "{k} {h:?}");
    }
    assert_eq!((dot::D_MIN, dot::D_MAX), (pow2(-485), pow2(510)));
    assert_eq!((dot::AXES_D_MIN, dot::AXES_D_MAX), (pow2(-485), pow2(510)));
    assert_eq!(dot::DIFFERENCES_RANGE, (pow2(-428), pow2(480)));
    assert_eq!(dot::TRANSLATED_RANGE, (pow2(-428), pow2(478)));
    assert_eq!(dot::SPANS_TRANSLATED_RANGE, (pow2(-428), pow2(474)));
    assert_eq!(dot::MAX_COORD_COMPONENTS, 16);
    assert_eq!(dot::BIG_INTEGERS_EXPLICIT_ARENA, 568);
}

#[test]
fn the_emitted_constants_of_dihedral_sign_are_those_of_the_derivation() {
    // The rules of the global bound give epsilon = 128 u and kappa = 48,
    // plus terms of order u, each inflated by `1 + 2^-30` and rounded up;
    // those of the bound by parts, the terms of epsilon 44 u D^(0, 2, 2) and
    // D^(2, 0, 2) and 40 u D^(2, 2, 0), and terms of kappa of 8.
    let u = Dyadic::from_f64(pow2(-53));
    let times_u = |n: f64| &Dyadic::from_f64(n) * &u;
    assert_eq!(dihedral::DEGREE, 4);
    assert!(is_derived(dihedral::EPSILON, &times_u(128.0)));
    assert!(is_derived(dihedral::KAPPA, &Dyadic::from_f64(48.0)));
    assert_eq!(dihedral::EPSILON, 1.42108547284369e-14);
    assert_eq!(dihedral::KAPPA, 48.0000000447035);
    let vectors = dihedral::AXES_EPSILON.map(|(g, _)| g);
    assert_eq!(vectors, [[0, 2, 2], [2, 0, 2], [2, 2, 0]]);
    for ((g, value), n) in dihedral::AXES_EPSILON.into_iter().zip([44.0, 44.0, 40.0]) {
        assert!(is_derived(value, &times_u(n)), "{g:?}");
    }
    let kappa = [
        (0, [1, 0, 2]),
        (0, [1, 2, 0]),
        (1, [0, 1, 2]),
        (1, [2, 1, 0]),
        (2, [0, 2, 1]),
        (2, [2, 0, 1]),
    ];
    assert_eq!(dihedral::AXES_KAPPA.map(|(k, h, _)| (k, h)), kappa);
    for (k, h, value) in dihedral::AXES_KAPPA {
        assert!(is_derived(value, &Dyadic::from_f64(8.0)), "{k} {h:?}");
    }
    assert_eq!((dihedral::D_MIN, dihedral::D_MAX), (pow2(-243), pow2(254)));
    let axes = (dihedral::AXES_D_MIN, dihedral::AXES_D_MAX);
    assert_eq!(axes, (pow2(-243), pow2(254)));
    assert_eq!(dihedral::DIFFERENCES_RANGE, (pow2(-188), pow2(239)));
    assert_eq!(dihedral::SPANS_TRANSLATED_RANGE, (pow2(-188), pow2(234)));
    assert_eq!(dihedral::MAX_COORD_COMPONENTS, 16);
    assert_eq!(dihedral::BIG_INTEGERS_EXPLICIT_ARENA, 2233);
}

#[test]
fn the_emitted_stages_are_those_of_the_tests() {
    // Both are of degree below 5, so they have stages on expansions; the
    // stage with translation of `dihedral_sign` does not fit on the stack,
    // so it exists only over spans, in an arena.
    let dot_names: Vec<&str> = DOT_STAGES[..4].iter().map(|(name, _)| *name).collect();
    let dihedral_names: Vec<&str> = DIHEDRAL_STAGES[..3].iter().map(|(name, _)| *name).collect();
    for (source, present, absent) in [
        (DOT_SOURCE, dot_names, None),
        (
            DIHEDRAL_SOURCE,
            dihedral_names,
            Some("pub fn expansions_translated("),
        ),
    ] {
        for name in present {
            assert!(source.contains(&format!("pub fn {name}(")), "{name}");
        }
        for name in ["expansions_translated_in", "big_integers_in"] {
            assert!(source.contains(&format!("pub fn {name}(")), "{name}");
        }
        if let Some(absent) = absent {
            assert!(!source.contains(absent), "{absent}");
            assert!(!source.contains("pub const TRANSLATED_RANGE"));
        }
    }
    let staged = |source: &str| {
        let start = source.find("pub fn staged(").expect("staged");
        let body = &source[start..];
        body[..body.find("\n}").expect("its end")].to_string()
    };
    let calls = |body: &str, names: &[&str]| -> Vec<usize> {
        names
            .iter()
            .map(|name| body.find(name).unwrap_or_else(|| panic!("{name}: {body}")))
            .collect()
    };
    let dot_staged = staged(DOT_SOURCE);
    let order = [
        "filter(p, 0.0)",
        "filter_axes(p, [0.0; 3])",
        "exact_differences(p)",
        "expansions_translated(p)",
    ];
    assert!(calls(&dot_staged, &order).is_sorted(), "{dot_staged}");
    let dihedral_staged = staged(DIHEDRAL_SOURCE);
    let order = [
        "filter(p, 0.0)",
        "filter_axes(p, [0.0; 3])",
        "exact_differences(p)",
        "staged_in_arena(scratch, p)",
    ];
    assert!(
        calls(&dihedral_staged, &order).is_sorted(),
        "{dihedral_staged}"
    );
}

#[test]
fn the_emitted_edge_cases_of_dot_sign_3d_pass() {
    let cases = dot_checks::bound_edge_cases();
    assert!(cases.len() >= 64, "{} cases", cases.len());
    for p in &cases {
        assert_eq!(dot_checks::exact(p), dot_oracle(p), "{p:?}");
    }
    let [decided, declined] = dot_checks::check_bound_edge_cases(|p| dot::filter(refs(p), 0.0));
    assert!(decided > 0 && declined > 0, "{decided} {declined}");
    let mut buf = vec![0.0; ARENA];
    let mut scratch = Scratch::borrowed(&mut buf);
    for (name, stage) in DOT_STAGES {
        let [decided, declined] =
            dot_checks::check_bound_edge_cases(|p| stage(&mut scratch, refs(p)));
        // The other stages may decline any case: their counts are not
        // checked, only the signs they give, inside `check_bound_edge_cases`.
        if ALWAYS_DECIDE.contains(&name) {
            assert_eq!((decided, declined), (cases.len(), 0), "{name}");
        }
    }
    let axes = dot_checks::axes_edge_cases();
    assert!(axes.len() >= 128, "{} cases", axes.len());
    for (p, _) in &axes {
        assert_eq!(dot_checks::exact(p), dot_oracle(p), "{p:?}");
    }
    let counts = dot_checks::check_axes_edge_cases(|p, e| dot::filter_axes(refs(p), e));
    for [decided, declined] in counts {
        assert!(decided > 0 && declined > 0, "{counts:?}");
    }
}

#[test]
fn the_emitted_edge_cases_of_dihedral_sign_pass() {
    let cases = dihedral_checks::bound_edge_cases();
    assert!(cases.len() >= 64, "{} cases", cases.len());
    for p in &cases {
        assert_eq!(dihedral_checks::exact(p), dihedral_oracle(p), "{p:?}");
    }
    let [decided, declined] =
        dihedral_checks::check_bound_edge_cases(|p| dihedral::filter(refs(p), 0.0));
    assert!(decided > 0 && declined > 0, "{decided} {declined}");
    let mut buf = vec![0.0; ARENA];
    let mut scratch = Scratch::borrowed(&mut buf);
    for (name, stage) in DIHEDRAL_STAGES {
        let [decided, declined] =
            dihedral_checks::check_bound_edge_cases(|p| stage(&mut scratch, refs(p)));
        // The other stages may decline any case: their counts are not
        // checked, only the signs they give, inside `check_bound_edge_cases`.
        if ALWAYS_DECIDE.contains(&name) {
            assert_eq!((decided, declined), (cases.len(), 0), "{name}");
        }
    }
    let axes = dihedral_checks::axes_edge_cases();
    assert!(axes.len() >= 128, "{} cases", axes.len());
    for (p, _) in &axes {
        assert_eq!(dihedral_checks::exact(p), dihedral_oracle(p), "{p:?}");
    }
    let counts = dihedral_checks::check_axes_edge_cases(|p, e| dihedral::filter_axes(refs(p), e));
    for [decided, declined] in counts {
        assert!(decided > 0 && declined > 0, "{counts:?}");
    }
}

/// A few calls whose sign is known, through the chain for points of any
/// type: the convention of each predicate.
#[test]
fn the_sign_conventions_are_those_of_the_definitions() {
    let mut buf = vec![0.0; ARENA];
    let mut scratch = Scratch::borrowed(&mut buf);
    let mut dot_sign = |p: Points3| dot::staged_in(&mut scratch, &p[0], &p[1], &p[2]).unwrap();
    let (x, o) = ([1.0, 0.0, 0.0], [0.0; 3]);
    assert_eq!(dot_sign([x, o, [0.0, 1.0, 0.0]]), Sign::Zero);
    assert_eq!(dot_sign([x, o, [1.0, 1.0, 0.0]]), Sign::Positive);
    assert_eq!(dot_sign([x, o, [-1.0, 1.0, 0.0]]), Sign::Negative);
    // The apex is the middle argument.
    assert_eq!(dot_sign([o, x, [-1.0, 1.0, 0.0]]), Sign::Positive);
    let mut buf = vec![0.0; ARENA];
    let mut scratch = Scratch::borrowed(&mut buf);
    let mut dihedral_sign =
        |p: Points4| dihedral::staged_in(&mut scratch, &p[0], &p[1], &p[2], &p[3]).unwrap();
    let (c, d) = ([0.0; 3], [0.0, 0.0, 1.0]);
    assert_eq!(dihedral_sign([x, [0.0, 1.0, 0.0], c, d]), Sign::Zero);
    assert_eq!(dihedral_sign([x, [1.0, 1.0, 0.0], c, d]), Sign::Positive);
    assert_eq!(dihedral_sign([x, [-1.0, 1.0, 0.0], c, d]), Sign::Negative);
}
