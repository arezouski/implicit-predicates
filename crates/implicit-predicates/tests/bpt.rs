//! The point type `Bpt`, a point on a triangle, against its definition,
//! `r + u (p - r) + v (q - r)`, computed by hand on `Dyadic`: the error
//! bound of each coordinate, the validation of the constructor and the size.

mod common;

use common::bpt::exact_bpt_by_hand;
use implicit_predicates::{AnyPoint, Bpt, Lnc, Point, PointError};
use implicit_predicates_oracle::Dyadic;
use implicit_predicates_test_support::{bpt_params, config};
use proptest::prelude::*;

/// `|a - b|`, exactly.
fn distance(a: f64, b: &Dyadic) -> Dyadic {
    let d = &Dyadic::from_f64(a) - b;
    if d.sign() < 0 { -d } else { d }
}

/// Whether `0 <= u`, `0 <= v` and `u + v <= 1`, exactly.
fn inside_the_triangle(u: f64, v: f64) -> bool {
    let (zero, one) = (Dyadic::from_f64(0.0), Dyadic::from_f64(1.0));
    let (du, dv) = (Dyadic::from_f64(u), Dyadic::from_f64(v));
    du >= zero && dv >= zero && &du + &dv <= one
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn a_coordinate_shared_by_the_parents_is_exact(
        params in bpt_params(),
        axis in 0usize..3,
        value in -1.0e3..1.0e3f64,
    ) {
        let (mut p, mut q, mut r, u, v) = params;
        (p[axis], q[axis], r[axis]) = (value, value, value);
        let point = Bpt::new_unchecked(p, q, r, u, v);
        prop_assert_eq!(point.err()[axis], 0.0, "{:?}", (p, q, r, u, v));
        prop_assert_eq!(point.approx()[axis], value, "{:?}", (p, q, r, u, v));
    }

    #[test]
    fn the_corner_r_is_exact(params in bpt_params()) {
        let (p, q, r, ..) = params;
        // With u = v = 0 the point is r, unless a difference of parents
        // overflows.
        if (0..3).all(|k| (p[k] - r[k]).is_finite() && (q[k] - r[k]).is_finite()) {
            let point = Bpt::new(p, q, r, 0.0, 0.0).unwrap();
            prop_assert_eq!(point.err(), &[0.0; 3], "{:?}", (p, q, r));
            prop_assert_eq!(point.approx(), &r, "{:?}", (p, q, r));
        }
    }

    #[test]
    fn an_inexact_coordinate_is_not_declared_exact(params in bpt_params()) {
        let (p, q, r, u, v) = params;
        let point = Bpt::new_unchecked(p, q, r, u, v);
        let exact = exact_bpt_by_hand(&p, &q, &r, u, v);
        for (k, exact) in exact.into_iter().enumerate() {
            if point.err()[k] == 0.0 {
                prop_assert_eq!(
                    Dyadic::from_f64(point.approx()[k]), exact, "{:?}: axis {}", params, k
                );
            }
        }
    }

    #[test]
    fn the_error_bound_holds(params in bpt_params()) {
        let (p, q, r, u, v) = params;
        let point = Bpt::new_unchecked(p, q, r, u, v);
        let exact = exact_bpt_by_hand(&p, &q, &r, u, v);
        for (k, exact) in exact.iter().enumerate() {
            let (a, e) = (point.approx()[k], point.err()[k]);
            prop_assert!(e >= 0.0, "{:?}: err {:?}", params, point.err());
            if !a.is_finite() {
                prop_assert_eq!(e, f64::INFINITY, "{:?}", params);
            } else if e < f64::INFINITY {
                prop_assert!(
                    distance(a, exact) <= Dyadic::from_f64(e),
                    "{:?}: axis {}, approx {:e}, err {:e}", params, k, a, e
                );
            }
        }
    }

    #[test]
    fn new_accepts_the_parameters_inside_the_triangle_and_nothing_else(params in bpt_params()) {
        let (p, q, r, u, v) = params;
        let expected = if inside_the_triangle(u, v) {
            Ok(())
        } else {
            Err(PointError::ParamOutOfRange)
        };
        let made = Bpt::new(p, q, r, u, v);
        prop_assert_eq!(made.map(|_| ()), expected, "{:?}", params);
        if let Ok(point) = made {
            prop_assert_eq!(
                (*point.p(), *point.q(), *point.r(), point.u(), point.v()),
                params
            );
        }
    }

    #[test]
    fn any_point_delegates(params in bpt_params()) {
        let (p, q, r, u, v) = params;
        let point = Bpt::new_unchecked(p, q, r, u, v);
        let any = AnyPoint::from(point);
        let bits = |x: &[f64; 3]| x.map(f64::to_bits);
        prop_assert!(matches!(any, AnyPoint::Bpt(_)));
        prop_assert_eq!(bits(any.approx()), bits(point.approx()));
        prop_assert_eq!(bits(any.err()), bits(point.err()));
        prop_assert!(!any.is_explicit());
    }
}

#[test]
fn bpt_new_rejects_what_is_outside_the_triangle() {
    let (p, q, r) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
    let tiny = f64::from_bits(1);
    let half_ulp = 2f64.powi(-53);
    let new = |u: f64, v: f64| Bpt::new(p, q, r, u, v).map(|_| ());
    let out = Err(PointError::ParamOutOfRange);
    // The corners and the edges are inside.
    for (u, v) in [
        (0.0, 0.0),
        (1.0, 0.0),
        (0.0, 1.0),
        (0.5, 0.5),
        (-0.0, 0.0),
        (0.0, -0.0),
    ] {
        assert_eq!(new(u, v), Ok(()), "{u} {v}");
    }
    // A negative parameter, however small.
    assert_eq!(new(-tiny, 0.0), out);
    assert_eq!(new(0.0, -tiny), out);
    assert_eq!(new(-1.0, 2.0), out);
    // A sum above one, however little: in `f64` it is one.
    assert_eq!(new(1.0, tiny), out);
    assert_eq!(new(tiny, 1.0), out);
    assert_eq!(new(0.5, 0.5 + half_ulp), out);
    assert_eq!(new(0.5 + half_ulp, 0.5), out);
    assert_eq!(new(1.0, 1.0), out);
    assert_eq!(new(f64::MAX, f64::MAX), out);
    assert_eq!(new(f64::MAX, -f64::MAX), out);
    // A sum below one is inside.
    assert_eq!(new(1.0f64.next_down(), tiny), Ok(()));
    assert_eq!(new(0.25, 0.75f64.next_down()), Ok(()));
    // Inputs that are not finite.
    let nan = f64::NAN;
    let inf = f64::INFINITY;
    for bad in [nan, inf, -inf] {
        assert_eq!(new(bad, 0.0), Err(PointError::NonFinite));
        assert_eq!(new(0.0, bad), Err(PointError::NonFinite));
        assert_eq!(
            Bpt::new(bad_point(bad), q, r, 0.0, 0.0),
            Err(PointError::NonFinite)
        );
        assert_eq!(
            Bpt::new(p, bad_point(bad), r, 0.0, 0.0),
            Err(PointError::NonFinite)
        );
        assert_eq!(
            Bpt::new(p, q, bad_point(bad), 0.0, 0.0),
            Err(PointError::NonFinite)
        );
    }
    // A coordinate that is not finite, on any axis.
    for k in 0..3 {
        let mut bad = p;
        bad[k] = nan;
        assert_eq!(Bpt::new(bad, q, r, 0.0, 0.0), Err(PointError::NonFinite));
    }
}

/// A point with a coordinate that is not finite.
fn bad_point(x: f64) -> [f64; 3] {
    [0.0, x, 0.0]
}

#[test]
fn a_bpt_with_extreme_magnitudes_keeps_the_contract() {
    let tiny = f64::from_bits(1);
    let values = [
        0.0,
        1e-300,
        -1e-300,
        1e300,
        -1e300,
        tiny,
        -tiny,
        f64::MIN_POSITIVE,
        f64::MAX,
        -f64::MAX,
        1.0,
        -1.0,
    ];
    let weights = [0.0, 1e-300, tiny, 0.25, 0.5, 1.0];
    let mut cases = 0;
    for &a in &values {
        for &b in &values {
            for &c in &values {
                for &u in &weights {
                    for &v in &weights {
                        let (p, q, r) = ([a, b, c], [b, c, a], [c, a, b]);
                        let point = Bpt::new_unchecked(p, q, r, u, v);
                        let exact = exact_bpt_by_hand(&p, &q, &r, u, v);
                        for (k, exact) in exact.iter().enumerate() {
                            let (x, e) = (point.approx()[k], point.err()[k]);
                            assert!(e >= 0.0, "{p:?} {q:?} {r:?} {u:e} {v:e}");
                            if !x.is_finite() {
                                assert_eq!(e, f64::INFINITY);
                            } else if e < f64::INFINITY {
                                assert!(
                                    distance(x, exact) <= Dyadic::from_f64(e),
                                    "{p:?} {q:?} {r:?} {u:e} {v:e}: axis {k}"
                                );
                            }
                            if e == 0.0 {
                                assert_eq!(&Dyadic::from_f64(x), exact);
                            }
                        }
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cases, 12 * 12 * 12 * 36);
}

#[test]
fn the_size_of_a_bpt_is_136_bytes() {
    // Three parents and two parameters, the approximation and its error:
    // 9 + 2 + 3 + 3 = 17 words of 8 bytes.
    assert_eq!(size_of::<Bpt>(), 136);
    // Two parents and one parameter: 6 + 1 + 3 + 3 = 13 words.
    assert_eq!(size_of::<Lnc>(), 104);
}
