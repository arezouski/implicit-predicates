//! `check` on the point types of the crate, against an oracle that computes
//! the distance between the approximation and the exact coordinates on
//! `Dyadic`, and on points made to break one obligation of the contract of
//! `Point` each.

// The tests use the growable arena, which needs the `std` feature.
#![cfg(feature = "std")]

mod common;

use common::odd_points::{LIAR_AT, Liar, Lie, Long, NoCoordinates, Weighted, WeightedOutOfRange};
use implicit_predicates::{
    AnyPoint, BigCoords, CheckError, ExactCoords, ExactError, Explicit, Lnc, Point, Scratch,
    Violation, check,
};
use implicit_predicates_oracle::Dyadic;
use implicit_predicates_test_support::{any_finite, config, lnc_params};
use proptest::prelude::*;

/// The exact coordinates of `Lnc::new_unchecked(p, q, t)`, `q + t (p - q)`,
/// written here rather than taken from the definition of the type.
fn lnc_exact(p: [f64; 3], q: [f64; 3], t: f64) -> [Dyadic; 3] {
    let t = Dyadic::from_f64(t);
    std::array::from_fn(|k| {
        let (p, q) = (Dyadic::from_f64(p[k]), Dyadic::from_f64(q[k]));
        &q + &(&t * &(&p - &q))
    })
}

/// Whether `|approx - exact| <= err`, exactly.
fn within(approx: f64, err: f64, exact: &Dyadic) -> bool {
    let d = &Dyadic::from_f64(approx) - exact;
    let err = Dyadic::from_f64(err);
    d <= err && -d <= err
}

/// `check` with an arena of its own.
fn checked<P: Point + ?Sized>(p: &P) -> Result<(), CheckError> {
    let mut scratch = Scratch::growable(64);
    let result = check(p, &mut scratch);
    assert_eq!(scratch.used(), 0, "check leaves the arena as it found it");
    result
}

/// The point `inner` with the error bound `err` instead of its own.
struct WithErr<'a, P> {
    inner: &'a P,
    err: [f64; 3],
}

impl<P: Point> Point for WithErr<'_, P> {
    fn approx(&self) -> &[f64; 3] {
        self.inner.approx()
    }

    fn err(&self) -> &[f64; 3] {
        &self.err
    }

    fn is_explicit(&self) -> bool {
        false
    }

    fn exact(&self, scratch: &mut Scratch<'_>) -> Result<ExactCoords, ExactError> {
        self.inner.exact(scratch)
    }

    fn exact_big(&self, scratch: &mut Scratch<'_>) -> Result<BigCoords, ExactError> {
        self.inner.exact_big(scratch)
    }
}

/// `check` accepts `p`, whose exact coordinates are `exact`, as the oracle
/// does; and on each axis with a finite, nonzero error, a smaller error
/// makes `check` reject the point with `ErrTooSmall` on that axis exactly
/// when the oracle says it no longer bounds the distance. The smaller errors
/// are one unit in the last place less and a few powers of two less.
fn agrees_with_the_oracle<P: Point>(p: &P, exact: &[Dyadic; 3]) -> Result<(), TestCaseError> {
    let (approx, err) = (*p.approx(), *p.err());
    for k in 0..3 {
        if err[k].is_finite() {
            prop_assert!(within(approx[k], err[k], &exact[k]), "axis {k}");
        }
    }
    prop_assert_eq!(checked(p), Ok(()));
    for k in 0..3 {
        if !(err[k].is_finite() && err[k] > 0.0) {
            continue;
        }
        let smaller = [
            err[k].next_down(),
            err[k] / 2.0,
            err[k] * 2f64.powi(-20),
            err[k] * 2f64.powi(-52),
            0.0,
        ];
        for e in smaller {
            let mut shrunk = err;
            shrunk[k] = e;
            let liar = WithErr {
                inner: p,
                err: shrunk,
            };
            let expected = if within(approx[k], e, &exact[k]) {
                Ok(())
            } else {
                Err(CheckError::Violation(Violation::ErrTooSmall { axis: k }))
            };
            prop_assert_eq!(checked(&liar), expected, "axis {} with err {:e}", k, e);
        }
    }
    Ok(())
}

/// Finite values of every kind, the extremes more often than `any_finite`
/// alone gives them.
fn coordinate() -> impl Strategy<Value = f64> {
    prop_oneof![
        any_finite(),
        prop::sample::select(vec![
            0.0,
            -0.0,
            f64::MAX,
            -f64::MAX,
            f64::MIN_POSITIVE,
            f64::from_bits(1),
            -f64::from_bits(1),
        ]),
    ]
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn explicit_points_keep_the_contract(p in prop::array::uniform3(coordinate())) {
        let exact = p.map(Dyadic::from_f64);
        agrees_with_the_oracle(&Explicit::new_unchecked(p), &exact)?;
        agrees_with_the_oracle(&AnyPoint::Explicit(Explicit::new_unchecked(p)), &exact)?;
        agrees_with_the_oracle(&p, &exact)?;
    }

    #[test]
    fn lnc_points_keep_the_contract((p, q, t) in lnc_params()) {
        let exact = lnc_exact(p, q, t);
        agrees_with_the_oracle(&Lnc::new_unchecked(p, q, t), &exact)?;
        agrees_with_the_oracle(&AnyPoint::Lnc(Lnc::new_unchecked(p, q, t)), &exact)?;
    }
}

#[test]
fn an_lnc_whose_approximation_overflows_keeps_the_contract() {
    let (p, q, t) = ([1e308, 0.0, 0.0], [-1e308, 0.0, 0.0], 0.5);
    let m = Lnc::new(p, q, t).unwrap();
    assert_eq!(m.err()[0], f64::INFINITY);
    assert_eq!(checked(&m), Ok(()));
    assert_eq!(checked(&AnyPoint::Lnc(m)), Ok(()));
    agrees_with_the_oracle(&m, &lnc_exact(p, q, t)).unwrap();
}

/// The explicit point `exact`, seen through `approx` and `err`.
struct Seen {
    exact: [f64; 3],
    approx: [f64; 3],
    err: [f64; 3],
}

impl Point for Seen {
    fn approx(&self) -> &[f64; 3] {
        &self.approx
    }

    fn err(&self) -> &[f64; 3] {
        &self.err
    }

    fn is_explicit(&self) -> bool {
        false
    }

    fn exact(&self, scratch: &mut Scratch<'_>) -> Result<ExactCoords, ExactError> {
        self.exact.exact(scratch)
    }

    fn exact_big(&self, scratch: &mut Scratch<'_>) -> Result<BigCoords, ExactError> {
        self.exact.exact_big(scratch)
    }
}

#[test]
fn an_infinite_err_on_one_axis_still_checks_the_others() {
    let exact = [1.0, 2.0, 3.0];
    let inf = f64::INFINITY;
    let seen = |approx, err| Seen { exact, approx, err };
    // An axis with an infinite error passes whatever its approximation.
    for x in [f64::NAN, inf, -inf, 1e300, 1.0] {
        assert_eq!(checked(&seen([x, 2.0, 3.0], [inf, 0.0, 0.0])), Ok(()));
    }
    assert_eq!(
        checked(&seen([f64::NAN, 2.5, 3.0], [inf, 0.25, 0.0])),
        Err(CheckError::Violation(Violation::ErrTooSmall { axis: 1 }))
    );
    assert_eq!(
        checked(&seen([f64::NAN, 2.5, 3.0], [inf, 0.5, 0.0])),
        Ok(())
    );
    assert_eq!(
        checked(&seen([0.0, 7.0, 3.0f64.next_up()], [inf, inf, 0.0])),
        Err(CheckError::Violation(Violation::ErrTooSmall { axis: 2 }))
    );
    assert_eq!(
        checked(&seen([0.0, 7.0, f64::NAN], [inf, inf, 1.0])),
        Err(CheckError::Violation(Violation::ApproxNotFinite {
            axis: 2
        }))
    );
}

#[test]
fn each_liar_gets_its_violation() {
    use Violation as V;
    let cases = [
        (Lie::InvalidErr(0, f64::NAN), V::InvalidErr { axis: 0 }),
        (Lie::InvalidErr(2, -1.0), V::InvalidErr { axis: 2 }),
        (
            Lie::InvalidErr(1, -f64::INFINITY),
            V::InvalidErr { axis: 1 },
        ),
        (Lie::ApproxNotFinite(1), V::ApproxNotFinite { axis: 1 }),
        (Lie::NotFinite, V::NotFinite),
        (Lie::WeightedBig, V::Weighted),
        (Lie::WeightedExpansions, V::Weighted),
        (Lie::ErrTooSmall(0), V::ErrTooSmall { axis: 0 }),
        (Lie::ErrTooSmall(2), V::ErrTooSmall { axis: 2 }),
        (Lie::ExplicitWithErr(1), V::ExplicitWithErr { axis: 1 }),
        (Lie::InvalidExpansion(0), V::InvalidExpansion { axis: 0 }),
        (Lie::InvalidExpansion(2), V::InvalidExpansion { axis: 2 }),
        (Lie::ExactMismatch(1), V::ExactMismatch { axis: 1 }),
        (Lie::UnstableApprox, V::Unstable),
        (Lie::UnstableErr, V::Unstable),
        (Lie::UnstableExplicit, V::Unstable),
        (Lie::UnstableExactBig, V::Unstable),
    ];
    for (lie, violation) in cases {
        assert_eq!(
            checked(&Liar::new(lie)),
            Err(CheckError::Violation(violation)),
            "{lie:?}"
        );
    }
}

#[test]
fn the_other_odd_points() {
    let violation = |v| Err(CheckError::Violation(v));
    assert_eq!(checked(&NoCoordinates), violation(Violation::NotFinite));
    assert_eq!(checked(&Weighted), violation(Violation::Weighted));
    assert_eq!(checked(&WeightedOutOfRange), violation(Violation::Weighted));
    let long = Long::new(vec![2f64.powi(-900), 1.0, 2f64.powi(900)], LIAR_AT[1], 0.0);
    assert_eq!(checked(&long), Ok(()));
}

#[test]
fn a_liar_leaves_the_arena_as_it_found_it() {
    let below = [0.5, 4.0];
    for lie in [
        Lie::InvalidExpansion(1),
        Lie::ExactMismatch(0),
        Lie::UnstableExactBig,
    ] {
        let mut scratch = Scratch::growable(4);
        let span = scratch.push(&below).unwrap();
        assert!(check(&Liar::new(lie), &mut scratch).is_err());
        assert_eq!(scratch.used(), below.len(), "{lie:?}");
        assert_eq!(scratch.get(span), &below, "{lie:?}");
    }
}

#[test]
fn a_point_that_lowers_the_top_is_reported() {
    for lie in [Lie::LowersTopInExact, Lie::LowersTopInSecondBig] {
        assert_eq!(
            checked(&Liar::new(lie)),
            Err(CheckError::Violation(Violation::ArenaBelowTop)),
            "{lie:?}"
        );
    }
    // Below the top that `check` was given: the top stays where the point
    // left it.
    // More components than the coordinates the point writes there.
    let below: Vec<f64> = (0..16).map(|i| 2f64.powi(60 * i - 400)).collect();
    let mut scratch = Scratch::growable(16);
    let mark = scratch.mark();
    scratch.push(&below).unwrap();
    assert_eq!(
        check(&Liar::lowering_to(mark), &mut scratch),
        Err(CheckError::Violation(Violation::ArenaBelowTop))
    );
    assert!(scratch.used() < below.len(), "{}", scratch.used());
}

#[test]
fn a_span_or_a_big_integer_above_the_top_is_reported_without_panicking() {
    for axis in 0..3 {
        assert_eq!(
            checked(&Liar::new(Lie::SpanAboveTop(axis))),
            Err(CheckError::Violation(Violation::InvalidExpansion { axis }))
        );
    }
    assert_eq!(
        checked(&Liar::new(Lie::BigAboveTop)),
        Err(CheckError::Violation(Violation::NonCanonicalBig {
            axis: 0
        }))
    );
}

#[test]
fn a_second_call_to_exact_big_above_the_top_is_reported() {
    assert_eq!(
        checked(&Liar::new(Lie::StaleSecondBig)),
        Err(CheckError::Violation(Violation::NonCanonicalBig {
            axis: 0
        }))
    );
}
