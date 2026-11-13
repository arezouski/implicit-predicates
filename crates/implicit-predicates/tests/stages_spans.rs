//! The stages of `orient3d` over arena spans, against the exact sign and
//! the stages on the stack: on explicit points, on coordinates of many
//! components, at the ends of their ranges, with too many components, a
//! weight other than one or an arena too small.

mod common;

use common::boxes::determinant_at;
use common::generated::exact;
use common::{Points, all_finite, all_in_range, edges, pow2, refs};
use implicit_predicates::stages::orient3d as stages;
use implicit_predicates::{Point, Scratch, ScratchExhausted, Sign};
use implicit_predicates_core::{ExactCoords, Weight};
use implicit_predicates_oracle::Dyadic;
use implicit_predicates_test_support::{
    config, coplanar4, expansion_within, extreme4, grid4, near_coplanar4, random4, repeated4,
    signed_zeros4,
};
use proptest::prelude::*;
use stages::MAX_COORD_COMPONENTS;

fn any_family() -> impl Strategy<Value = Points> {
    prop_oneof![
        random4(),
        coplanar4(),
        near_coplanar4(),
        grid4(),
        repeated4(),
        signed_zeros4(),
        extreme4(&span_edges()),
    ]
}

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

/// The stages over spans, on the exact coordinates of `p` written to a
/// fresh arena: their results and the top of the arena after each call,
/// which must be where it was.
fn over_spans(p: &Points) -> (Option<Sign>, Option<Sign>) {
    let mut buf = vec![0.0; 1 << 16];
    let mut scratch = Scratch::borrowed(&mut buf);
    let q = p.map(|point| point.exact(&mut scratch).unwrap());
    let used = scratch.used();
    let q = [&q[0], &q[1], &q[2], &q[3]];
    let translated = stages::expansions_translated_in(&mut scratch, q).unwrap();
    assert_eq!(scratch.used(), used);
    let untranslated = stages::expansions_untranslated_in(&mut scratch, q).unwrap();
    assert_eq!(scratch.used(), used);
    (translated, untranslated)
}

/// Writes each coordinate, given as an expansion, to `scratch`.
fn write_points(scratch: &mut Scratch<'_>, p: &[[Vec<f64>; 3]; 4]) -> [ExactCoords; 4] {
    p.clone().map(|point| {
        let [x, y, z] = point.map(|e| scratch.push(&e).unwrap());
        ExactCoords {
            x,
            y,
            z,
            w: Weight::One,
        }
    })
}

/// The exact sign of orient3d at points whose coordinates are expansions.
fn exact_of_expansions(p: &[[Vec<f64>; 3]; 4]) -> Sign {
    let exact = p.clone().map(|point| point.map(|e| Dyadic::sum_of(&e)));
    match determinant_at(&exact).sign() {
        -1 => Sign::Negative,
        0 => Sign::Zero,
        _ => Sign::Positive,
    }
}

/// Both stages over spans at points whose coordinates are expansions.
fn spans_of_expansions(p: &[[Vec<f64>; 3]; 4]) -> (Option<Sign>, Option<Sign>) {
    let mut buf = vec![0.0; 1 << 20];
    let mut scratch = Scratch::borrowed(&mut buf);
    let q = write_points(&mut scratch, p);
    let q = [&q[0], &q[1], &q[2], &q[3]];
    (
        stages::expansions_translated_in(&mut scratch, q).unwrap(),
        stages::expansions_untranslated_in(&mut scratch, q).unwrap(),
    )
}

/// Four points whose coordinates are expansions of up to 16 components,
/// every one of them within both ranges over spans: the leaves they are
/// summed from have magnitudes in `2^-136..=2^136` and up to 53 bits, so
/// every component is a multiple of `2^-188` and below `2^141`.
fn expansion_points() -> impl Strategy<Value = [[Vec<f64>; 3]; 4]> {
    let point = || std::array::from_fn(|_| expansion_within(MAX_COORD_COMPONENTS, 136));
    let points: [[_; 3]; 4] = std::array::from_fn(|_| point());
    points
}

/// An expansion of 16 components that spans the range over spans of the
/// translated form, with the signs of `signs`: from `2^234`, its top, down
/// to `2^-158` every 28 binades, then `(1 + 2^-52) 2^-188`, at its bottom,
/// whose last bit is `2^-240`.
fn spanning(signs: u16) -> Vec<f64> {
    let mut e: Vec<f64> = (0..15).map(|k| pow2(234 - 28 * k)).collect();
    e.push((1.0 + f64::EPSILON) * pow2(-188));
    e.reverse();
    for (k, c) in e.iter_mut().enumerate() {
        if signs >> k & 1 == 1 {
            *c = -*c;
        }
    }
    e
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn the_stages_over_spans_agree_with_the_oracle_and_the_stack(p in any_family()) {
        prop_assume!(all_finite(&p));
        let expected = exact(&p);
        let (translated, untranslated) = over_spans(&p);
        for (name, sign) in [("translated", translated), ("untranslated", untranslated)] {
            if let Some(sign) = sign {
                prop_assert_eq!(sign, expected, "{} on {:?}", name, p);
            }
        }
        if let (Some(t), Some(s)) = (translated, stages::expansions_translated(refs(&p))) {
            prop_assert_eq!(t, s, "{:?}", p);
        }
        if let (Some(u), Some(s)) = (untranslated, stages::expansions_untranslated(refs(&p))) {
            prop_assert_eq!(u, s, "{:?}", p);
        }
        // Within its range each decides.
        if all_in_range(&p, stages::SPANS_TRANSLATED_RANGE) {
            prop_assert_eq!(translated, Some(expected), "{:?}", p);
        }
        if all_in_range(&p, stages::SPANS_UNTRANSLATED_RANGE) {
            prop_assert_eq!(untranslated, Some(expected), "{:?}", p);
        }
    }

    #[test]
    fn coordinates_of_many_components_are_decided_exactly(p in expansion_points()) {
        let range = stages::SPANS_TRANSLATED_RANGE;
        let components = p.iter().flatten().flatten();
        prop_assert!(components.clone().all(|&c| (range.0..=range.1).contains(&c.abs())));
        let expected = exact_of_expansions(&p);
        prop_assert_eq!(spans_of_expansions(&p), (Some(expected), Some(expected)), "{:?}", p);
    }

    #[test]
    fn coordinates_that_span_the_range_are_decided_exactly(
        signs in prop::array::uniform12(any::<u16>()),
        zeros in any::<u16>(),
    ) {
        // Every product operand of the evaluation then reaches as far as the
        // components allow toward both ends of the product domain, which the
        // arithmetic core checks in debug builds.
        let p: [[Vec<f64>; 3]; 4] = std::array::from_fn(|i| {
            std::array::from_fn(|k| {
                if zeros >> (3 * i + k) & 1 == 1 { Vec::new() } else { spanning(signs[3 * i + k]) }
            })
        });
        let expected = exact_of_expansions(&p);
        prop_assert_eq!(spans_of_expansions(&p), (Some(expected), Some(expected)), "{:?}", p);
    }
}

#[test]
fn the_ranges_over_spans_lie_within_the_ranges_over_one_component() {
    let inside = |inner: (f64, f64), outer: (f64, f64)| outer.0 <= inner.0 && inner.1 <= outer.1;
    assert!(inside(
        stages::SPANS_TRANSLATED_RANGE,
        stages::TRANSLATED_RANGE
    ));
    assert!(inside(
        stages::SPANS_UNTRANSLATED_RANGE,
        stages::UNTRANSLATED_RANGE
    ));
    assert_eq!(stages::SPANS_TRANSLATED_RANGE, (pow2(-188), pow2(234)));
    assert_eq!(stages::SPANS_UNTRANSLATED_RANGE, (pow2(-188), pow2(235)));
    assert_eq!(stages::MAX_COORD_COMPONENTS, 16);
}

#[test]
fn a_component_outside_the_range_is_declined() {
    let (t, u) = (
        stages::SPANS_TRANSLATED_RANGE,
        stages::SPANS_UNTRANSLATED_RANGE,
    );
    let base = [
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.5, 0.5, 0.5],
    ];
    assert_eq!(over_spans(&base), (Some(exact(&base)), Some(exact(&base))));
    // Just inside and just outside each end of each range, as the second
    // component of a coordinate whose first is 1 (or, at the top, alone).
    let at = |value: f64| {
        let p = [
            [vec![value], vec![], vec![]],
            [vec![], vec![1.0], vec![]],
            [vec![], vec![], vec![1.0]],
            [vec![0.5], vec![0.5], vec![0.5]],
        ];
        spans_of_expansions(&p)
    };
    let small = |value: f64| {
        let p = [
            [vec![value, 1.0], vec![], vec![]],
            [vec![], vec![1.0], vec![]],
            [vec![], vec![], vec![1.0]],
            [vec![0.5], vec![0.5], vec![0.5]],
        ];
        spans_of_expansions(&p)
    };
    assert!(matches!(at(t.1), (Some(_), Some(_))));
    assert!(matches!(at(t.1.next_up()), (None, Some(_))));
    assert!(matches!(at(-u.1.next_up()), (None, None)));
    assert!(matches!(small(t.0), (Some(_), Some(_))));
    assert!(matches!(small(t.0.next_down()), (None, _)));
    assert!(matches!(small(-u.0.next_down()), (_, None)));
}

// Without debug assertions only: with them, the arena itself refuses to
// write an expansion that is not valid.
#[cfg(not(debug_assertions))]
#[test]
fn a_zero_component_is_declined() {
    // Zero is the empty expansion: a coordinate with a zero component is
    // not a valid expansion, wherever the zero is.
    for zero in [0.0, -0.0] {
        for coordinate in [vec![zero], vec![zero, 1.0], vec![1.0, zero]] {
            let p = [
                [coordinate.clone(), vec![], vec![]],
                [vec![], vec![1.0], vec![]],
                [vec![], vec![], vec![1.0]],
                [vec![0.5], vec![0.5], vec![0.5]],
            ];
            let mut buf = vec![0.0; 1 << 12];
            let mut scratch = Scratch::borrowed(&mut buf);
            let q = p.map(|point| {
                let [x, y, z] = point.map(|e| {
                    scratch
                        .write(e.len(), |out| {
                            out[..e.len()].copy_from_slice(&e);
                            e.len()
                        })
                        .unwrap()
                });
                ExactCoords {
                    x,
                    y,
                    z,
                    w: Weight::One,
                }
            });
            let q = [&q[0], &q[1], &q[2], &q[3]];
            let used = scratch.used();
            assert_eq!(
                stages::expansions_translated_in(&mut scratch, q),
                Ok(None),
                "{coordinate:?}"
            );
            assert_eq!(
                stages::expansions_untranslated_in(&mut scratch, q),
                Ok(None),
                "{coordinate:?}"
            );
            assert_eq!(scratch.used(), used);
        }
    }
}

#[test]
fn a_coordinate_of_17_components_is_declined() {
    // Powers of four: a valid expansion of any length.
    let long = |n: i32| (0..n).map(|k| pow2(2 * k)).collect::<Vec<f64>>();
    let points = |n: i32| {
        [
            [long(n), vec![], vec![]],
            [vec![], vec![1.0], vec![]],
            [vec![], vec![], vec![1.0]],
            [vec![0.5], vec![0.5], vec![0.5]],
        ]
    };
    let p16 = points(16);
    let expected = exact_of_expansions(&p16);
    assert_eq!(spans_of_expansions(&p16), (Some(expected), Some(expected)));
    assert_eq!(spans_of_expansions(&points(17)), (None, None));
}

#[test]
fn a_weight_other_than_one_is_declined() {
    let mut buf = vec![0.0; 1 << 12];
    let mut scratch = Scratch::borrowed(&mut buf);
    let p = [
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.5, 0.5, 0.5],
    ];
    let mut q = p.map(|point| point.exact(&mut scratch).unwrap());
    q[2].w = Weight::Expansion(scratch.push(&[2.0]).unwrap());
    let q = [&q[0], &q[1], &q[2], &q[3]];
    assert_eq!(stages::expansions_translated_in(&mut scratch, q), Ok(None));
    assert_eq!(
        stages::expansions_untranslated_in(&mut scratch, q),
        Ok(None)
    );
}

#[test]
fn an_arena_too_small_is_an_error_that_leaves_it_as_it_was() {
    let p = [
        [0.1, 0.3, 0.7],
        [1.1, -0.3, 0.2],
        [0.4, 0.9, -1.5],
        [0.25, 0.5, 0.125],
    ];
    let mut needed = None;
    // From an arena that holds only the coordinates up to one that is large
    // enough: every failure leaves the top where it was.
    for spare in 0..4096 {
        let mut buf = vec![0.0; 12 + spare];
        let mut scratch = Scratch::borrowed(&mut buf);
        let q = p.map(|point| point.exact(&mut scratch).unwrap());
        let used = scratch.used();
        let q = [&q[0], &q[1], &q[2], &q[3]];
        let results = [
            stages::expansions_translated_in(&mut scratch, q),
            stages::expansions_untranslated_in(&mut scratch, q),
        ];
        assert_eq!(scratch.used(), used, "{spare}");
        match results {
            [Ok(t), Ok(u)] => {
                assert_eq!((t, u), (Some(exact(&p)), Some(exact(&p))));
                needed = Some(spare);
                break;
            }
            [t, u] => {
                assert!([t, u].contains(&Err(ScratchExhausted)), "{spare}");
                assert!(
                    [t, u]
                        .iter()
                        .all(|r| r.is_err() || *r == Ok(Some(exact(&p))))
                );
            }
        }
    }
    assert!(needed.is_some_and(|spare| spare > 0), "{needed:?}");
}
