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
use implicit_predicates::stages::orient3d as stages;
use implicit_predicates_oracle::Dyadic;
use implicit_predicates_test_support::{config, leaf, sample};
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

/// Twelve magnitudes in the two binades `[2^(top - 2), 2^top)`, each with a
/// full 53-bit significand, from fixed-seed random bits.
fn magnitudes(top: i32, count: usize) -> Vec<[f64; 12]> {
    sample(prop::array::uniform12(any::<u64>()), count)
        .into_iter()
        .map(|bits| {
            bits.map(|b| {
                // The top bit picks the binade; the lowest significand bit
                // is set, so the significand has 53 bits.
                let exponent = top - 1 - i32::from(b >> 63 == 1);
                let biased = u64::try_from(exponent + 1023).unwrap();
                f64::from_bits((biased << 52) | (b & ((1 << 52) - 1)) | 1)
            })
        })
        .collect()
}

/// Every coordinate jointly at the top, or jointly at the bottom, of
/// `TRANSLATED_RANGE`, with full significands in its two outermost binades
/// and every one of the 4096 sign patterns of the twelve coordinates.
///
/// Opposite signs across two binades make most differences inexact, so
/// `two_diff` leaves a nonzero low part. Run in a debug build, no assertion
/// of the core may fire and the sign must be exact. This test establishes
/// that the range is safe, not that it is tight: the stage would also be
/// sound with a somewhat larger upper end.
#[test]
fn the_translated_stage_is_exact_with_every_coordinate_at_an_end_of_its_range() {
    let (min, max) = stages::TRANSLATED_RANGE;
    let ends = [(exponent(max), "top"), (exponent(min) + 2, "bottom")];
    for (top, end) in ends {
        let sets = magnitudes(top, 4096);
        let mut inexact = 0;
        for (pattern, magnitudes) in sets.iter().enumerate() {
            let p: Points = std::array::from_fn(|i| {
                std::array::from_fn(|axis| {
                    let k = 3 * i + axis;
                    let x = magnitudes[k];
                    if pattern >> k & 1 == 1 { -x } else { x }
                })
            });
            assert!(
                common::all_in_range(&p, stages::TRANSLATED_RANGE),
                "{end}: {p:?}"
            );
            inexact += (0..3)
                .flat_map(|i| (0..3).map(move |axis| (i, axis)))
                .filter(|&(i, axis)| {
                    let (a, b) = (p[i][axis], p[3][axis]);
                    Dyadic::from_f64(a - b) != &Dyadic::from_f64(a) - &Dyadic::from_f64(b)
                })
                .count();
            assert_eq!(
                stages::expansions_translated(refs(&p)),
                Some(exact(&p)),
                "{end}: {p:?}"
            );
        }
        // Most differences have a nonzero low part.
        assert!(
            inexact > 9 * sets.len() / 4,
            "{end}: {inexact} inexact differences"
        );
    }
}
