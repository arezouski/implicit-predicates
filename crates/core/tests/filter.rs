//! The uncertainty of the filter: the sum of the two largest errors, per
//! axis or overall, rounded up, with infinities and NaN kept.

use implicit_predicates_core::filter::{uncertainty, uncertainty_max};
use implicit_predicates_oracle::Dyadic;
use implicit_predicates_test_support::{any_finite, config, leaf, value};
use proptest::prelude::*;

/// A per-coordinate error: zero, a moderate value, or any finite magnitude.
fn err() -> impl Strategy<Value = f64> {
    prop_oneof![
        Just(0.0),
        leaf(-60, 0, 53).prop_map(f64::abs),
        any_finite().prop_map(f64::abs),
    ]
}

/// The sum of the two largest of `values`, computed by sorting: the
/// smallest `f64` not below the exact sum, or `+∞` if that exceeds
/// `f64::MAX`.
fn reference(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| b.total_cmp(a));
    let top_two = &sorted[..2.min(sorted.len())];
    let exact = value(top_two);
    if exact > Dyadic::from_f64(f64::MAX) {
        return f64::INFINITY;
    }
    let mut up = top_two.iter().sum::<f64>();
    while Dyadic::from_f64(up) < exact {
        up = up.next_up();
    }
    while up > 0.0 && Dyadic::from_f64(up.next_down()) >= exact {
        up = up.next_down();
    }
    up
}

/// Per axis, the reference sum of the two largest errors.
fn reference_per_axis(errs: &[[f64; 3]]) -> [f64; 3] {
    [0, 1, 2].map(|k| reference(&errs.iter().map(|e| e[k]).collect::<Vec<_>>()))
}

/// The reference sum of the two largest per-point maxima.
fn reference_max(errs: &[[f64; 3]]) -> f64 {
    let maxima: Vec<f64> = errs.iter().map(|e| e[0].max(e[1]).max(e[2])).collect();
    reference(&maxima)
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn uncertainty_of_four_points_matches_sorting(
        errs in prop::array::uniform4(prop::array::uniform3(err())),
    ) {
        let refs = errs.each_ref();
        prop_assert_eq!(uncertainty(refs), reference_per_axis(&errs));
        prop_assert_eq!(uncertainty_max(refs), reference_max(&errs));
    }

    #[test]
    fn uncertainty_of_five_points_matches_sorting(
        errs in prop::array::uniform5(prop::array::uniform3(err())),
    ) {
        let refs = errs.each_ref();
        prop_assert_eq!(uncertainty(refs), reference_per_axis(&errs));
        prop_assert_eq!(uncertainty_max(refs), reference_max(&errs));
    }
}

#[test]
fn uncertainty_sums_the_two_largest_errors_per_axis() {
    let errs = [
        [1.0, 0.0, 0.25],
        [4.0, 0.5, 0.0],
        [2.0, 0.0, 0.0],
        [0.0, 0.0, 8.0],
    ];
    assert_eq!(uncertainty(errs.each_ref()), [6.0, 0.5, 8.25]);
    let five = [
        [1.0, 3.0, 0.0],
        [4.0, 0.5, 0.0],
        [2.0, 3.0, 0.0],
        [0.0, 1.0, 8.0],
        [4.0, 2.0, 16.0],
    ];
    assert_eq!(uncertainty(five.each_ref()), [8.0, 6.0, 24.0]);
}

#[test]
fn uncertainty_max_sums_the_two_largest_maxima() {
    let errs = [
        [1.0, 0.0, 0.25],
        [0.0, 0.5, 4.0],
        [2.0, 0.0, 0.0],
        [0.0, 0.0, 0.125],
    ];
    assert_eq!(uncertainty_max(errs.each_ref()), 6.0);
    let five = [
        [0.0; 3],
        [1.0; 3],
        [0.0, 0.0, 3.0],
        [2.0, 0.0, 0.0],
        [0.0; 3],
    ];
    assert_eq!(uncertainty_max(five.each_ref()), 5.0);
}

#[test]
fn explicit_points_give_exactly_zero() {
    let zero = [0.0; 3];
    let errs = [&zero; 4];
    let e = uncertainty(errs);
    assert_eq!(e.map(f64::to_bits), [0.0f64.to_bits(); 3]);
    assert_eq!(uncertainty_max(errs).to_bits(), 0.0f64.to_bits());
}

#[test]
fn inexact_sums_round_up() {
    let small = 2f64.powi(-60);
    let errs = [[1.0, 0.0, 0.0], [small, 0.0, 0.0], [0.0; 3], [0.0; 3]];
    assert_eq!(uncertainty(errs.each_ref())[0], 1f64.next_up());
    assert_eq!(uncertainty_max(errs.each_ref()), 1f64.next_up());
    let huge = [
        [f64::MAX, 0.0, 0.0],
        [f64::MAX, 0.0, 0.0],
        [0.0; 3],
        [0.0; 3],
    ];
    assert_eq!(uncertainty(huge.each_ref())[0], f64::INFINITY);
}

#[test]
fn an_infinite_error_stays_on_its_axis() {
    let errs = [
        [1.0, 2.0, 3.0],
        [0.0, f64::INFINITY, 0.0],
        [1.0; 3],
        [0.0; 3],
    ];
    assert_eq!(uncertainty(errs.each_ref()), [2.0, f64::INFINITY, 4.0]);
    assert_eq!(uncertainty_max(errs.each_ref()), f64::INFINITY);
}

#[test]
fn a_nan_error_makes_its_axis_not_finite() {
    for point in 0..5 {
        for axis in 0..3 {
            let mut errs = [[1.0, 2.0, 3.0]; 5];
            errs[point][axis] = f64::NAN;
            let e = uncertainty(errs.each_ref());
            for k in 0..3 {
                if k == axis {
                    assert!(!e[k].is_finite(), "NaN at {point}, {axis} gave {e:?}");
                } else {
                    assert_eq!(e[k], [2.0, 4.0, 6.0][k], "NaN at {point}, {axis}");
                }
            }
            let m = uncertainty_max(errs.each_ref());
            assert!(!m.is_finite(), "NaN at {point}, {axis} gave {m}");
        }
    }
}
