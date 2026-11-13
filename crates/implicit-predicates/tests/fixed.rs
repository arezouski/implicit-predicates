//! Fixed cases of `orient3d` with implicit points, so that what they check
//! does not depend on the seed of the random tests: the error of every
//! point, in every position and on every axis, enters the filter, even when
//! the other points are explicit; and the untranslated stage over spans
//! decides near the top of its range.

#![cfg(feature = "std")]

mod common;

use common::args::{Arg, Args, decidable, decided_on_expansions, every_stage, exact_sign};
use implicit_predicates::{Lnc, Scratch, Sign, orient3d, orient3d_in};

/// Checks every stage on `args` against the exact sign, that the chain
/// decides, and that it does so before the stage on big integers when it is
/// documented to.
fn check(args: &Args) {
    let expected = exact_sign(args);
    let results = every_stage(args);
    for (name, result) in &results {
        if let Some(sign) = result {
            assert_eq!(*sign, expected, "{name} on {args:?}");
        }
    }
    // Every call decides; without the stage on big integers, at least
    // within the documented range.
    for (name, result) in &results {
        if !name.starts_with("filter") && !name.starts_with("expansions") && *name != "exact_stages"
        {
            assert_eq!(*result, Some(expected), "{name} on {args:?}");
        }
    }
    if decidable(args) {
        assert!(decided_on_expansions(&results), "{args:?}");
    }
}

/// `point` with its coordinates rotated so that axis 2 becomes `axis`.
fn on_axis(point: [f64; 3], axis: usize) -> [f64; 3] {
    std::array::from_fn(|k| point[(k + 2 - axis) % 3])
}

/// Three explicit points on the plane where coordinate `axis` is 1, and a
/// point on a line from that plane to the plane where it is 2, at the
/// parameter `t`: its approximation rounds onto the plane, so every
/// difference of coordinates on `axis` is zero there, while the exact point
/// is off the plane. A filter that leaves out its error sees a flat axis
/// and gives a zero, which is wrong.
fn off_the_plane_by_rounding(axis: usize, t: f64) -> ([[f64; 3]; 3], Arg) {
    let plane = [[0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [0.0, 1.0, 1.0]].map(|p| on_axis(p, axis));
    let p = on_axis([0.25, 0.375, 2.0], axis);
    let q = on_axis([0.25, 0.375, 1.0], axis);
    (plane, Arg::Lnc(p, q, t))
}

/// The ways the other three points are given: explicit, or as points on a
/// line with both parents at the point, whose coordinates are exact but
/// which are not explicit.
fn others(point: [f64; 3], explicit: bool) -> Arg {
    if explicit {
        Arg::Explicit(point)
    } else {
        Arg::Lnc(point, point, 0.5)
    }
}

#[test]
fn the_error_of_every_point_enters_the_filter() {
    // `2^-60` keeps the point within the range of the exact stages; the
    // smallest subnormal does not.
    for t in [2f64.powi(-60), 5e-324] {
        for axis in 0..3 {
            for i in 0..4 {
                for explicit in [true, false] {
                    let (plane, line) = off_the_plane_by_rounding(axis, t);
                    let mut plane = plane.into_iter();
                    let args: Args = std::array::from_fn(|n| {
                        if n == i {
                            line
                        } else {
                            others(plane.next().unwrap(), explicit)
                        }
                    });
                    // The approximation is on the plane; the point is not.
                    let approx = args[i].any();
                    assert_eq!(implicit_predicates::Point::approx(&approx)[axis], 1.0);
                    assert_ne!(exact_sign(&args), Sign::Zero, "{args:?}");
                    let filter = every_stage(&args)[0];
                    assert_eq!(filter, ("filter_points", None), "{args:?}");
                    check(&args);
                    if t > 2f64.powi(-100) {
                        assert!(decidable(&args), "{args:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn the_error_of_every_point_enters_the_filter_with_two_implicit_points() {
    // Two points on lines, one with an exact approximation, in every pair
    // of positions: the error of the other must still be counted.
    let t = 2f64.powi(-60);
    for axis in 0..3 {
        for i in 0..4 {
            for j in (0..4).filter(|&j| j != i) {
                let (plane, line) = off_the_plane_by_rounding(axis, t);
                let mut plane = plane.into_iter();
                let args: Args = std::array::from_fn(|n| {
                    if n == i {
                        line
                    } else {
                        let point = plane.next().unwrap();
                        others(point, n != j)
                    }
                });
                assert_eq!(every_stage(&args)[0], ("filter_points", None), "{args:?}");
                check(&args);
            }
        }
    }
}

#[test]
fn regressions_of_lost_errors() {
    let l = |p, q, t| Arg::Lnc(p, q, t);
    let e = Arg::Explicit;
    let cases: [Args; 2] = [
        [
            l([0.0, 3.0, -0.0], [-1.0, 0.1, 1.0], 5e-324),
            e([-1.0, 0.0, 0.5]),
            e([-1.0, -1.0, 1.0]),
            e([-1.0, -0.0, -0.0]),
        ],
        [
            l([-1.0, 3.0, 0.0], [1.0, 0.0, -0.0], 0.4381637269356644),
            l([0.0, 1.0, 0.1], [0.1, 1.0, 0.0], 5e-324),
            l([3.0, 0.5, 3.0], [0.5, 1.0, -0.0], 0.0),
            l([1.0, 0.1, -0.0], [0.0, 0.5, 0.0], 0.14743676396194894),
        ],
    ];
    for args in cases {
        check(&args);
    }
}

#[test]
fn the_untranslated_stage_over_spans_decides_near_the_top_of_its_range() {
    // Coordinates above the range of the translated stage over spans,
    // `2^234`, and within that of the untranslated one, `2^235`.
    let s = 1.5 * 2f64.powi(234);
    let (a, b, c) = ([s, 0.0, 0.0], [0.0, s, 0.0], [0.0, 0.0, s]);
    let m = Lnc::new(a, b, 0.3).unwrap();
    assert_eq!(orient3d(&a, &b, &c, &m), Sign::Zero);
    let mut buf = vec![0.0; 1 << 12];
    let mut scratch = Scratch::borrowed(&mut buf);
    assert_eq!(orient3d_in(&mut scratch, &a, &b, &c, &m), Ok(Sign::Zero));
    let m = Lnc::new(a, [0.0, 0.0, -s], 0.3).unwrap();
    for args in [
        [
            Arg::Explicit(a),
            Arg::Explicit(b),
            Arg::Explicit(c),
            Arg::Lnc(a, b, 0.3),
        ],
        [
            Arg::Explicit(a),
            Arg::Explicit(b),
            Arg::Explicit(c),
            Arg::Lnc(a, [0.0, 0.0, -s], 0.3),
        ],
    ] {
        check(&args);
        // The untranslated stage over spans decides, not the translated
        // one, nor the stage on big integers after them.
        let results = every_stage(&args);
        let result = |name: &str| results.iter().find(|(n, _)| *n == name).unwrap().1;
        assert_eq!(result("expansions_translated_in"), None, "{args:?}");
        assert_eq!(
            result("expansions_untranslated_in"),
            Some(exact_sign(&args)),
            "{args:?}"
        );
    }
    let args = [
        Arg::Explicit(a),
        Arg::Explicit(b),
        Arg::Explicit(c),
        Arg::Lnc(a, [0.0, 0.0, -s], 0.3),
    ];
    assert_eq!(orient3d(&a, &b, &c, &m), exact_sign(&args));
}
