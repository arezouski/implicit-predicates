//! Every stage of `orient3d` against the exact sign, with the real
//! arithmetic of the core: in a debug build the core checks that every
//! exact product stays in its domain, so inputs at both ends of each range
//! of the exact stages, including nearly coplanar ones, also check that the
//! ranges are sound.

use indirect_predicates::Sign;
use indirect_predicates::stages::orient3d as stages;
use indirect_predicates_oracle::Dyadic;
use indirect_predicates_test_support::{config, leaf};
use proptest::prelude::*;

type Points = [[f64; 3]; 4];

/// The exact sign of det[a - d; b - d; c - d].
fn exact_sign(p: &Points) -> Sign {
    let row = |i: usize| -> [Dyadic; 3] {
        core::array::from_fn(|axis| &Dyadic::from_f64(p[i][axis]) - &Dyadic::from_f64(p[3][axis]))
    };
    let (a, b, c) = (row(0), row(1), row(2));
    let minor = |x: usize, y: usize| &(&b[x] * &c[y]) - &(&b[y] * &c[x]);
    let det = &(&(&a[0] * &minor(1, 2)) - &(&a[1] * &minor(0, 2))) + &(&a[2] * &minor(0, 1));
    match det.sign() {
        1 => Sign::Positive,
        -1 => Sign::Negative,
        _ => Sign::Zero,
    }
}

fn refs(p: &Points) -> [&[f64; 3]; 4] {
    [&p[0], &p[1], &p[2], &p[3]]
}

/// The binary exponent of a power of two.
fn exponent(x: f64) -> i32 {
    let e = x.log2();
    assert_eq!(e.fract(), 0.0, "{x} is not a power of two");
    e as i32
}

/// Zero, or a value of either sign with magnitude in `[min, max]`, with a
/// full significand, at the lower end, at the upper end or anywhere between.
fn coordinate((min, max): (f64, f64)) -> BoxedStrategy<f64> {
    let (low, high) = (exponent(min), exponent(max));
    prop_oneof![
        1 => Just(0.0),
        1 => prop_oneof![Just(min), Just(-min), Just(max), Just(-max)],
        3 => leaf(low, low, 53),
        3 => leaf(high - 1, high - 1, 53),
        2 => leaf(low, high - 1, 53),
    ]
    .boxed()
}

fn points(range: (f64, f64)) -> impl Strategy<Value = Points> {
    let c = coordinate(range);
    let point = [c.clone(), c.clone(), c];
    [point.clone(), point.clone(), point.clone(), point]
}

/// Four points one unit in the last place apart around `a`, nearly coplanar,
/// and the same with the last one moved back onto a coordinate of `a`.
fn nearly_coplanar(range: (f64, f64)) -> impl Strategy<Value = Points> {
    let c = coordinate(range);
    ([c.clone(), c.clone(), c], 0usize..4, 0usize..3).prop_map(move |(a, which, axis)| {
        let step = |x: f64, up: bool| {
            let next = if up { x.next_up() } else { x.next_down() };
            if next.abs() <= range.1 && (next == 0.0 || next.abs() >= range.0) {
                next
            } else {
                x
            }
        };
        let mut p = [a; 4];
        for (i, point) in p.iter_mut().enumerate().take(3) {
            point[i] = step(point[i], true);
        }
        p[which][axis] = step(p[which][axis], false);
        p
    })
}

/// Checks that every stage that decides gives the exact sign, and that a
/// stage whose range holds the input decides.
fn check(p: &Points, must_decide: &[&str]) -> Result<(), TestCaseError> {
    let exact = exact_sign(p);
    let results = [
        ("filter", stages::filter(refs(p), 0.0)),
        ("exact_differences", stages::exact_differences(refs(p))),
        (
            "expansions_translated",
            stages::expansions_translated(refs(p)),
        ),
        (
            "expansions_untranslated",
            stages::expansions_untranslated(refs(p)),
        ),
        ("staged", stages::staged(refs(p))),
    ];
    for (name, result) in results {
        if let Some(sign) = result {
            prop_assert_eq!(sign, exact, "{} on {:?}", name, p);
        } else {
            prop_assert!(
                !must_decide.contains(&name),
                "{} abstained on {:?}",
                name,
                p
            );
        }
    }
    prop_assert_eq!(
        indirect_predicates::orient3d(&p[0], &p[1], &p[2], &p[3]),
        Some(exact)
    );
    Ok(())
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn stages_are_exact_across_the_untranslated_range(
        p in points(stages::UNTRANSLATED_RANGE),
    ) {
        check(&p, &["expansions_untranslated", "staged"])?;
    }

    #[test]
    fn stages_are_exact_across_the_translated_range(p in points(stages::TRANSLATED_RANGE)) {
        check(&p, &["expansions_translated", "expansions_untranslated", "staged"])?;
    }

    #[test]
    fn stages_are_exact_on_nearly_coplanar_points(
        p in nearly_coplanar(stages::TRANSLATED_RANGE),
    ) {
        check(&p, &["expansions_translated", "expansions_untranslated", "staged"])?;
    }

    #[test]
    fn exact_differences_decides_across_its_range(
        a in points(stages::DIFFERENCES_RANGE),
    ) {
        // With d at the origin the differences are the coordinates of a, b
        // and c, computed exactly.
        let mut p = a;
        p[3] = [0.0; 3];
        check(&p, &["exact_differences", "staged"])?;
    }

    #[test]
    fn the_filter_is_exact_near_the_ends_of_its_range(
        scale in prop_oneof![Just(stages::D_MIN), Just(stages::D_MAX / 4.0)],
        p in points((0.5, 2.0)),
    ) {
        let scaled = p.map(|point| point.map(|x| x * scale));
        if let Some(sign) = stages::filter(refs(&scaled), 0.0) {
            prop_assert_eq!(sign, exact_sign(&scaled), "{:?}", scaled);
        }
    }
}
