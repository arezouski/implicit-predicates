//! The documented condition on the inputs of an `Lnc` under which
//! `orient3d` decides: `t` zero or at least `2^-k` for some `k >= 0`, and
//! every nonzero coordinate of `p` and `q` of a magnitude between
//! `2^(k - 84)` and `2^233`, with every nonzero coordinate of the explicit
//! points between `2^-188` and `2^235`. Inside it the exact coordinates of
//! the point are computed and every component lies in the range of the
//! exact stage over spans, so the call is decided, also when only an exact
//! stage can decide it; outside it a call can give `None`, as the two
//! documented examples do.

// `orient3d` needs the `std` feature.
#![cfg(feature = "std")]

mod common;

use common::args::{Arg, exact_sign};
use implicit_predicates::stages::orient3d as stages;
use implicit_predicates::{Lnc, Point, Scratch, Sign, orient3d, orient3d_in};
use implicit_predicates_test_support::{config, leaf};
use proptest::prelude::*;

/// The least and the greatest magnitude of a nonzero component that the
/// exact stage over spans accepts, `2^-188` and `2^235`.
fn spans_range() -> (f64, f64) {
    let range = stages::SPANS_UNTRANSLATED_RANGE;
    assert_eq!(range, (2f64.powi(-188), 2f64.powi(235)));
    range
}

/// Asserts that the exact coordinates of the `Lnc` of `p`, `q` and `t` are
/// computed and that every nonzero component lies in [`spans_range`].
fn assert_components_in_range(p: [f64; 3], q: [f64; 3], t: f64) {
    let (min, max) = spans_range();
    let m = Lnc::new(p, q, t).unwrap();
    let mut buf = vec![0.0; 256];
    let mut scratch = Scratch::borrowed(&mut buf);
    let exact = m.exact(&mut scratch).expect("the exact coordinates");
    for span in [exact.x, exact.y, exact.z] {
        let components = scratch.get(span);
        assert!(components.len() <= stages::MAX_COORD_COMPONENTS);
        for &x in components {
            assert!(
                x == 0.0 || (min..=max).contains(&x.abs()),
                "{p:?} {q:?} {t:e}: component {x:e}"
            );
        }
    }
}

/// Asserts that `orient3d` and `orient3d_in` decide on `args` with the
/// exact sign, and returns it.
fn assert_decided(args: &[Arg; 4]) -> Sign {
    let [a, b, c, d] = args.each_ref().map(Arg::any);
    let sign = exact_sign(args);
    assert_eq!(orient3d(&a, &b, &c, &d), Some(sign), "{args:?}");
    let mut buf = vec![0.0; 1 << 14];
    let mut scratch = Scratch::borrowed(&mut buf);
    assert_eq!(
        orient3d_in(&mut scratch, &a, &b, &c, &d),
        Ok(Some(sign)),
        "{args:?}"
    );
    sign
}

/// Checks one `Lnc` of the condition and the calls built from it: with
/// `a`, `b` and `c` explicit, the `Lnc` of `p`, `q` and `t` as the fourth
/// point, and the points of the segments from `p` to `q`, which are
/// coplanar with `p`, `q` and any third point, so that only an exact stage
/// decides.
fn check_condition(p: [f64; 3], q: [f64; 3], t: f64, others: [[f64; 3]; 2]) {
    assert_components_in_range(p, q, t);
    let [c, e] = others;
    let lnc = Arg::Lnc(p, q, t);
    assert_decided(&[Arg::Explicit(c), Arg::Explicit(e), Arg::Explicit(p), lnc]);
    let zero = [Arg::Explicit(p), Arg::Explicit(q), Arg::Explicit(c), lnc];
    assert_eq!(assert_decided(&zero), Sign::Zero);
    let two = [lnc, Arg::Explicit(c), Arg::Lnc(q, p, t), Arg::Explicit(p)];
    assert_eq!(assert_decided(&two), Sign::Zero);
}

/// A nonzero magnitude in `2^min..=2^max` with a random significand and
/// sign, or zero; at most `2^max` exactly.
fn coordinate(min: i32, max: i32) -> impl Strategy<Value = f64> {
    prop_oneof![
        8 => leaf(min, max - 1, 53),
        1 => prop::sample::select(vec![0.0, 2f64.powi(min), -2f64.powi(max), 2f64.powi(max)]),
    ]
}

/// A point whose coordinates are [`coordinate`]s.
fn point(min: i32, max: i32) -> impl Strategy<Value = [f64; 3]> {
    [
        coordinate(min, max),
        coordinate(min, max),
        coordinate(min, max),
    ]
}

/// `k` in `0..=316`, and `p`, `q` and `t` that meet the condition for it;
/// `k = 317`, where every nonzero coordinate is `2^233`, is a fixed case.
fn condition() -> impl Strategy<Value = ([f64; 3], [f64; 3], f64)> {
    (0i32..=316).prop_flat_map(|k| {
        let t = prop_oneof![
            1 => prop::sample::select(vec![0.0, 1.0, 1f64.next_down(), 2f64.powi(-k)]),
            4 => (-k..=0, any::<u64>()).prop_map(|(exp, bits)| {
                (2f64.powi(exp) * (1.0 + (bits >> 12) as f64 / 2f64.powi(52))).min(1.0)
            }),
        ];
        (point(k - 84, 233), point(k - 84, 233), t)
    })
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn inputs_meeting_the_condition_are_decided(
        (p, q, t) in condition(),
        others in [point(-188, 235), point(-188, 235)],
    ) {
        check_condition(p, q, t, others);
    }
}

#[test]
fn inputs_at_the_edges_of_the_condition_are_decided() {
    let top = 2f64.powi(233);
    let others = [[1.0, -0.75, 3.0], [2f64.powi(-188), 0.0, 2f64.powi(235)]];
    for k in [0, 1, 53, 54, 84, 140, 317] {
        let low = 2f64.powi(k - 84);
        let ts = [0.0, 1.0, 2f64.powi(-k), 1f64.next_down().max(2f64.powi(-k))];
        let mantissa = 1.0 + 3.0 * f64::EPSILON;
        let parents = [
            ([low, -top, (low * mantissa).min(top)], [top, low, -low]),
            ([-top, top, low], [top, -top, top]),
            ([(low * mantissa).min(top), 0.0, top], [low, top, 0.0]),
        ];
        for t in ts {
            for (p, q) in parents {
                check_condition(p, q, t, others);
                check_condition(q, p, t, others);
            }
        }
    }
}

#[test]
fn the_documented_examples_outside_the_condition_give_none() {
    let (min, _) = spans_range();
    let a = [1.0, 0.25, 0.5];
    let c = [0.3, 0.7, 5.0];
    // A coordinate of `q` below `2^(k - 84)` for `t = 0.1`, and a `t` below
    // `2^-k` for the coordinates of `p` and `q`, which are above `2^-4`.
    for (b, t) in [
        ([2f64.powi(-140), 3.0, 0.75], 0.1),
        ([0.1, 3.0, 0.75], 2f64.powi(-200)),
    ] {
        let m = Lnc::new(a, b, t).unwrap();
        assert_eq!(orient3d(&a, &b, &c, &m), None, "{b:?} {t:e}");
        // The exact coordinates are computed, but a component of one of
        // them is below the range of the exact stage over spans.
        let mut buf = vec![0.0; 256];
        let mut scratch = Scratch::borrowed(&mut buf);
        let exact = m.exact(&mut scratch).unwrap();
        let smallest = [exact.x, exact.y, exact.z]
            .into_iter()
            .flat_map(|s| scratch.get(s).to_vec())
            .filter(|&x| x != 0.0)
            .map(f64::abs)
            .fold(f64::INFINITY, f64::min);
        assert!(smallest < min, "{b:?} {t:e}: {smallest:e}");
    }
}

/// The smallest nonzero magnitude of a component of the exact coordinates
/// of the `Lnc` of `p`, `q` and `t`.
fn smallest_component(p: [f64; 3], q: [f64; 3], t: f64) -> f64 {
    let m = Lnc::new(p, q, t).unwrap();
    let mut buf = vec![0.0; 256];
    let mut scratch = Scratch::borrowed(&mut buf);
    let exact = m.exact(&mut scratch).unwrap();
    [exact.x, exact.y, exact.z]
        .into_iter()
        .flat_map(|s| scratch.get(s).to_vec())
        .filter(|&x| x != 0.0)
        .map(f64::abs)
        .fold(f64::INFINITY, f64::min)
}

#[test]
fn the_lower_ends_of_the_condition_are_reached() {
    // `p.x` is below the last bit of `q.x`, so `p.x - q.x` has `p.x` as a
    // component, and its product by `t` has a component of the product of
    // their last bits: `2^(k - 136) * 2^(-k - 52) = 2^-188`.
    let (min, _) = spans_range();
    let next = 1.0 + f64::EPSILON;
    let c = [0.3, 0.7, 5.0];
    for k in [1, 54, 100, 200] {
        let t = 2f64.powi(-k) * next;
        let low = 2f64.powi(k - 84) * next;
        let q = [2f64.powi(233), 1.0, 1.0];
        let p = [low, 1.0, 1.0];
        assert_eq!(smallest_component(p, q, t), min, "k = {k}");
        check_condition(p, q, t, [c, [1.0, 2.0, 3.0]]);
        // One bit lower in `p.x`, or in `t`: a component below the range,
        // and `None` for points that only an exact stage can decide.
        for (p, t) in [([low / 2.0, 1.0, 1.0], t), (p, t / 2.0)] {
            assert!(smallest_component(p, q, t) < min, "k = {k}");
            let m = Lnc::new(p, q, t).unwrap();
            assert_eq!(orient3d(&p, &q, &c, &m), None, "k = {k}");
        }
    }
}
