//! The exact stages of `orient3d` at both ends of their ranges, with the
//! real arithmetic of the core.
//!
//! In a debug build, the default for tests, the core asserts that the
//! operands of every exact product are in its domain. Each stage is driven
//! here with leaves at the ends of its range, at their inward neighbours and
//! in near-cancellation, so these tests also check that the ranges derived
//! for the stages keep every product in that domain: no assertion may fire.
//! The leaves are the coordinates for the two expansion forms, and the
//! computed differences for `exact_differences`.

mod common;

use common::generated::exact;
use common::{Points, Stage, refs};
use indirect_predicates::stages::orient3d as stages;
use indirect_predicates_test_support::{config, leaf};
use proptest::prelude::*;

/// The binary exponent of a power of two.
fn exponent(x: f64) -> i32 {
    let e = x.log2();
    assert_eq!(e.fract(), 0.0, "{x} is not a power of two");
    e as i32
}

/// Zero, or a value of either sign with magnitude in `[min, max]`: an end
/// of the range, its inward neighbour, or a full significand in the lowest
/// binade, in the highest one or anywhere between.
fn coordinate((min, max): (f64, f64)) -> BoxedStrategy<f64> {
    let (low, high) = (exponent(min), exponent(max));
    let ends = prop::sample::select(vec![min, min.next_up(), max, max.next_down()]);
    prop_oneof![
        1 => prop_oneof![Just(0.0), Just(-0.0)],
        2 => (ends, any::<bool>()).prop_map(|(x, negative)| if negative { -x } else { x }),
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

/// `x` moved one unit in the last place up or down, unless that leaves the
/// range.
fn step(x: f64, up: bool, (min, max): (f64, f64)) -> f64 {
    let next = if up { x.next_up() } else { x.next_down() };
    if next.abs() <= max && (next == 0.0 || next.abs() >= min) {
        next
    } else {
        x
    }
}

/// Four points one unit in the last place apart around a point of the
/// range: nearly coplanar, so that the determinant cancels deeply.
fn nearly_coplanar(range: (f64, f64)) -> impl Strategy<Value = Points> {
    let c = coordinate(range);
    ([c.clone(), c.clone(), c], 0usize..4, 0usize..3).prop_map(move |(a, which, axis)| {
        let mut p = [a; 4];
        for (i, point) in p.iter_mut().enumerate().take(3) {
            point[i] = step(point[i], true, range);
        }
        p[which][axis] = step(p[which][axis], false, range);
        p
    })
}

/// With the last point at the origin, the differences are the coordinates
/// of the other three, computed exactly.
fn at_origin(mut p: Points) -> Points {
    p[3] = [0.0; 3];
    p
}

/// A nonzero magnitude just outside the range, with either sign.
fn outside((min, max): (f64, f64)) -> impl Strategy<Value = f64> {
    (
        prop::sample::select(vec![min.next_down(), max.next_up()]),
        any::<bool>(),
    )
        .prop_map(|(x, negative)| if negative { -x } else { x })
}

/// The stages with leaves on coordinates, with their ranges.
fn coordinate_stages() -> [(&'static str, Stage, (f64, f64)); 2] {
    [
        (
            "expansions_translated",
            stages::expansions_translated,
            stages::TRANSLATED_RANGE,
        ),
        (
            "expansions_untranslated",
            stages::expansions_untranslated,
            stages::UNTRANSLATED_RANGE,
        ),
    ]
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn leaves_just_inside_the_range_are_exact(
        differences in prop_oneof![
            points(stages::DIFFERENCES_RANGE),
            nearly_coplanar(stages::DIFFERENCES_RANGE),
        ],
        translated in prop_oneof![
            points(stages::TRANSLATED_RANGE),
            nearly_coplanar(stages::TRANSLATED_RANGE),
        ],
        untranslated in prop_oneof![
            points(stages::UNTRANSLATED_RANGE),
            nearly_coplanar(stages::UNTRANSLATED_RANGE),
        ],
    ) {
        let p = at_origin(differences);
        prop_assert_eq!(stages::exact_differences(refs(&p)), Some(exact(&p)), "{:?}", p);
        let [(_, t, _), (_, u, _)] = coordinate_stages();
        prop_assert_eq!(t(refs(&translated)), Some(exact(&translated)), "{:?}", translated);
        prop_assert_eq!(u(refs(&untranslated)), Some(exact(&untranslated)), "{:?}", untranslated);
    }

    #[test]
    fn leaves_just_outside_the_range_do_not_decide(
        differences in points(stages::DIFFERENCES_RANGE),
        translated in points(stages::TRANSLATED_RANGE),
        untranslated in points(stages::UNTRANSLATED_RANGE),
        out_d in outside(stages::DIFFERENCES_RANGE),
        out_t in outside(stages::TRANSLATED_RANGE),
        out_u in outside(stages::UNTRANSLATED_RANGE),
        point in 0usize..3,
        axis in 0usize..3,
    ) {
        // One difference just outside the range.
        let mut p = at_origin(differences);
        p[point][axis] = out_d;
        prop_assert_eq!(stages::exact_differences(refs(&p)), None, "{:?}", p);
        // A difference out of the range from two coordinates in it.
        let (_, max) = stages::DIFFERENCES_RANGE;
        let mut p = differences;
        p[point][axis] = max;
        p[3][axis] = -max;
        prop_assert_eq!(stages::exact_differences(refs(&p)), None, "{:?}", p);
        // One coordinate just outside the range, at any of the four points.
        for ((name, stage, _), (base, out)) in coordinate_stages()
            .into_iter()
            .zip([(translated, out_t), (untranslated, out_u)])
        {
            for which in 0..4 {
                let mut p = base;
                p[which][axis] = out;
                prop_assert_eq!(stage(refs(&p)), None, "{} on {:?}", name, p);
            }
        }
    }
}
