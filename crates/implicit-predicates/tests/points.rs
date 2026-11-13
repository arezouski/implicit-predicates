//! The point types against the exact coordinates of their declarations:
//! the error bound of each coordinate of a point on a line, its exact
//! coordinates, the validation of the constructors, and `AnyPoint`.

#[path = "generated/points.rs"]
mod generated;

use implicit_predicates::{AnyPoint, Explicit, Lnc, Point, PointError, Scratch};
use implicit_predicates_core::{ExactCoords, ExactError, Weight, eft, expansion};
use implicit_predicates_oracle::Dyadic;
use implicit_predicates_test_support::{
    LncParams, Point3, assert_valid_expansion, config, lnc_params, point3, value,
};
use proptest::prelude::*;

/// The exact coordinates of `Lnc { p, q, t }`.
fn exact((p, q, t): LncParams) -> [Dyadic; 3] {
    generated::exact_lnc([p, q], [t])
}

/// `|a - b|`, exactly.
fn distance(a: f64, b: &Dyadic) -> Dyadic {
    let d = &Dyadic::from_f64(a) - b;
    if d.sign() < 0 { -d } else { d }
}

/// The exact coordinates of `point`, read from an arena, with each span
/// checked to be a valid expansion.
fn exact_of(point: &impl Point) -> Result<[Vec<f64>; 3], ExactError> {
    let mut buf = [0.0; 64];
    let mut scratch = Scratch::borrowed(&mut buf);
    let ExactCoords { x, y, z, w } = point.exact(&mut scratch)?;
    assert_eq!(w, Weight::One);
    Ok([x, y, z].map(|span| {
        let e = scratch.get(span).to_vec();
        assert_valid_expansion(&e);
        e
    }))
}

/// Whether the exact arithmetic of a point on a line can take `(p, q, t)`:
/// every parent coordinate is below `2^1020`, and `t` and every component
/// of the exact differences of the parents lie in the product domain. The
/// scaled differences are then below `2^1020` too.
fn in_exact_range((p, q, t): LncParams) -> bool {
    let limit = 2f64.powi(1020);
    let below = |x: f64| x.abs() < limit;
    eft::in_product_domain(t)
        && (0..3).all(|k| {
            let mut d = [0.0; 2];
            let n = if below(p[k]) && below(q[k]) {
                expansion::from_two_diff(p[k], q[k], &mut d)
            } else {
                return false;
            };
            d[..n].iter().all(|&c| eft::in_product_domain(c))
        })
}

/// `Lnc` with the parameters of `params`.
fn lnc((p, q, t): LncParams) -> Lnc {
    Lnc::new_unchecked(p, q, t)
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn lnc_error_bounds_hold(params in lnc_params()) {
        let point = lnc(params);
        for (k, exact) in exact(params).iter().enumerate() {
            let (a, e) = (point.approx()[k], point.err()[k]);
            prop_assert!(e >= 0.0, "{:?}: err {:?}", params, point.err());
            if !a.is_finite() {
                prop_assert_eq!(e, f64::INFINITY, "{:?}", params);
                continue;
            }
            if e < f64::INFINITY {
                prop_assert!(
                    distance(a, exact) <= Dyadic::from_f64(e),
                    "{:?}: axis {}, approx {:e}, err {:e}", params, k, a, e
                );
            }
        }
    }

    #[test]
    fn zero_error_means_exact(params in lnc_params()) {
        let point = lnc(params);
        for (k, exact) in exact(params).into_iter().enumerate() {
            if point.err()[k] == 0.0 {
                prop_assert_eq!(
                    Dyadic::from_f64(point.approx()[k]), exact, "{:?}: axis {}", params, k
                );
            }
        }
    }

    #[test]
    fn shared_parent_coordinates_are_exact(params in lnc_params(), axis in 0usize..3) {
        let (p, mut q, t) = params;
        q[axis] = p[axis];
        let point = Lnc::new_unchecked(p, q, t);
        prop_assert_eq!(point.err()[axis], 0.0, "{:?}", (p, q, t));
        prop_assert_eq!(point.approx()[axis], p[axis], "{:?}", (p, q, t));
        // With t = 0 the point is q, exactly, unless a difference of the
        // parents overflows.
        let point = Lnc::new_unchecked(p, q, 0.0);
        if (0..3).all(|k| (p[k] - q[k]).is_finite()) {
            prop_assert_eq!(point.err(), &[0.0; 3], "{:?}", (p, q));
            prop_assert_eq!(point.approx(), &q, "{:?}", (p, q));
        }
    }

    #[test]
    fn exact_coordinates_match_the_oracle(params in lnc_params()) {
        let expected = exact(params);
        match exact_of(&lnc(params)) {
            Ok(coordinates) => {
                prop_assert!(in_exact_range(params), "{:?}", params);
                for (k, e) in coordinates.iter().enumerate() {
                    prop_assert!(e.len() <= 5, "{:?}: {:?}", params, e);
                    prop_assert_eq!(value(e), expected[k].clone(), "{:?}: axis {}", params, k);
                }
            }
            Err(ExactError::OutOfRange) => prop_assert!(!in_exact_range(params), "{:?}", params),
            Err(e) => prop_assert!(false, "{:?}: {}", params, e),
        }
    }

    #[test]
    fn any_point_delegates(params in lnc_params(), e in point3()) {
        let point = lnc(params);
        let any = AnyPoint::from(point);
        let bits = |x: &[f64; 3]| x.map(f64::to_bits);
        prop_assert!(matches!(any, AnyPoint::Lnc(_)));
        prop_assert_eq!(bits(any.approx()), bits(point.approx()));
        prop_assert_eq!(bits(any.err()), bits(point.err()));
        prop_assert!(!any.is_explicit());
        prop_assert_eq!(exact_of(&any), exact_of(&point));

        let explicit = Explicit::new(e).unwrap();
        let any = AnyPoint::from(explicit);
        prop_assert_eq!(any.approx(), &e);
        prop_assert_eq!(any.err(), &[0.0; 3]);
        prop_assert!(any.is_explicit() && explicit.is_explicit());
        prop_assert_eq!(exact_of(&any), exact_of(&e));
        prop_assert_eq!(exact_of(&explicit), exact_of(&e));
    }
}

#[test]
fn the_ends_of_the_parameter_are_the_parents() {
    let (p, q) = ([3.0, -0.1, 1e-100], [0.7, 2.5, -4.0]);
    let exact = |t: f64| {
        exact_of(&Lnc::new(p, q, t).unwrap())
            .unwrap()
            .map(|e| value(&e))
    };
    assert_eq!(exact(0.0), q.map(Dyadic::from_f64));
    assert_eq!(exact(1.0), p.map(Dyadic::from_f64));
    let point = Lnc::new(p, q, 0.25).unwrap();
    assert_eq!((point.p(), point.q(), point.t()), (&p, &q, 0.25));
}

/// Every way of putting `bad` in one input of a point on a line.
fn with_one(bad: f64) -> Vec<LncParams> {
    let (p, q, t): LncParams = ([0.5, -1.0, 2.0], [1.5, 0.0, -3.0], 0.5);
    let mut out = vec![(p, q, bad)];
    for k in 0..3 {
        let mut p2 = p;
        p2[k] = bad;
        out.push((p2, q, t));
        let mut q2 = q;
        q2[k] = bad;
        out.push((p, q2, t));
    }
    out
}

#[test]
fn constructors_validate() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for (p, q, t) in with_one(bad) {
            assert_eq!(
                Lnc::new(p, q, t),
                Err(PointError::NonFinite),
                "{:?}",
                (p, q, t)
            );
            // Accepted without checking.
            let _ = Lnc::new_unchecked(p, q, t);
        }
        for k in 0..3 {
            let mut e: Point3 = [1.0, 2.0, 3.0];
            e[k] = bad;
            assert_eq!(Explicit::new(e), Err(PointError::NonFinite));
            assert_eq!(
                Explicit::new_unchecked(e).approx()[k].to_bits(),
                bad.to_bits()
            );
        }
    }
    let (p, q) = ([0.5, -1.0, 2.0], [1.5, 0.0, -3.0]);
    for t in [
        -f64::from_bits(1),
        -1.0,
        1.0f64.next_up(),
        2.0,
        f64::MAX,
        f64::MIN,
    ] {
        assert_eq!(Lnc::new(p, q, t), Err(PointError::ParamOutOfRange), "{t}");
        assert_eq!(Lnc::new_unchecked(p, q, t).t(), t);
    }
    for t in [0.0, -0.0, f64::from_bits(1), 0.5, 1.0f64.next_down(), 1.0] {
        assert_eq!(Lnc::new(p, q, t), Ok(Lnc::new_unchecked(p, q, t)), "{t}");
    }
    assert_eq!(
        Explicit::new([1.0, -0.0, 5e-324]).unwrap().approx(),
        &[1.0, -0.0, 5e-324]
    );
    assert_eq!(
        PointError::NonFinite.to_string(),
        "an input of the point is not finite"
    );
}

#[test]
fn non_finite_points_have_infinite_error() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for params in with_one(bad) {
            let point = lnc(params);
            for k in 0..3 {
                if !point.approx()[k].is_finite() {
                    assert_eq!(point.err()[k], f64::INFINITY, "{params:?}");
                }
            }
            assert!(point.approx().iter().any(|a| !a.is_finite()), "{params:?}");
            assert_eq!(exact_of(&point), Err(ExactError::OutOfRange), "{params:?}");
        }
    }
    // A NaN parameter spoils every coordinate.
    assert_eq!(
        lnc(([1.0; 3], [2.0; 3], f64::NAN)).err(),
        &[f64::INFINITY; 3]
    );
    // With t = 0 the point is q, but the difference of the parents
    // overflows and the approximation is 0 * inf, NaN.
    let point = lnc(([1e308; 3], [-1e308; 3], 0.0));
    assert!(point.approx().iter().all(|a| a.is_nan()));
    assert_eq!(point.err(), &[f64::INFINITY; 3]);
    // Its exact coordinates are outside the range of the exact arithmetic.
    assert_eq!(exact_of(&point), Err(ExactError::OutOfRange));
    // Every operation of x has a zero operand, so its computed error is
    // zero, but its approximation, 0 * inf, is NaN: the error is infinite
    // because of the approximation alone.
    let point = Lnc::new_unchecked([f64::INFINITY, 0.0, 0.0], [0.0; 3], 0.0);
    assert!(point.approx()[0].is_nan());
    assert_eq!(point.err()[0], f64::INFINITY);
    assert_eq!(&point.err()[1..], &[0.0, 0.0]);
    // For a point on a line the error bound cannot overflow while the
    // approximation is finite: every term is about 2^-53 times finite
    // values. Near the top of the range both stay finite, and the bound
    // holds.
    let params = ([f64::MAX, 0.0, 0.0], [f64::MAX / 2.0, 0.0, 0.0], 1.0);
    let point = lnc(params);
    assert!(point.approx()[0].is_finite() && point.err()[0].is_finite());
    assert!(distance(point.approx()[0], &exact(params)[0]) <= Dyadic::from_f64(point.err()[0]));
}

#[test]
fn a_small_arena_is_reported_and_left_as_it_was() {
    let point = Lnc::new([0.1, 0.2, 0.3], [0.7, -0.6, 0.5], 0.3).unwrap();
    for size in 0..15 {
        let mut buf = vec![0.0; size];
        let mut scratch = Scratch::borrowed(&mut buf);
        match point.exact(&mut scratch) {
            Err(ExactError::Exhausted(_)) => assert_eq!(scratch.used(), 0, "{size}"),
            Ok(_) => assert!(size >= 5, "{size}"),
            Err(e) => panic!("{size}: {e}"),
        }
    }
    let mut buf = vec![0.0; 4];
    let mut scratch = Scratch::borrowed(&mut buf);
    assert!(matches!(
        point.exact(&mut scratch),
        Err(ExactError::Exhausted(_))
    ));
    let mut buf = vec![0.0; 15];
    let mut scratch = Scratch::borrowed(&mut buf);
    assert!(point.exact(&mut scratch).is_ok());
}

/// A point type written by hand needs every type of the contract, and of
/// the arena it writes to, from this crate alone.
#[test]
fn the_types_of_the_point_contract_come_from_this_crate() {
    use implicit_predicates::{ExactCoords, ExactError, Mark, Point, Scratch, Span, Weight};

    struct Origin;

    impl Point for Origin {
        fn approx(&self) -> &[f64; 3] {
            &[0.0; 3]
        }

        fn err(&self) -> &[f64; 3] {
            &[0.0; 3]
        }

        fn is_explicit(&self) -> bool {
            false
        }

        fn exact(&self, scratch: &mut Scratch<'_>) -> Result<ExactCoords, ExactError> {
            let mark: Mark = scratch.mark();
            // Zero is the empty expansion.
            let zero: Result<Span, _> = scratch.push(&[]);
            let Ok(zero) = zero else {
                scratch.reset(mark);
                return Err(ExactError::OutOfRange);
            };
            Ok(ExactCoords {
                x: zero,
                y: zero,
                z: zero,
                w: Weight::One,
            })
        }
    }

    let (a, b, c) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
    let mut buf = [0.0; 4096];
    let mut scratch = Scratch::borrowed(&mut buf);
    let origin = implicit_predicates::orient3d_in(&mut scratch, &a, &b, &c, &Origin);
    let explicit = implicit_predicates::orient3d_in(&mut scratch, &a, &b, &c, &[0.0; 3]);
    assert_eq!(origin, explicit);
    assert!(matches!(origin, Ok(Some(_))), "{origin:?}");
}
