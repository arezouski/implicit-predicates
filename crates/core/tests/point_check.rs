//! `check` on explicit points: it accepts every finite `[f64; 3]`, rejects
//! one that is not finite, reports an arena that runs out, and leaves the
//! arena as it found it.

use implicit_predicates_core::Scratch;
use implicit_predicates_core::point::{CheckError, Violation, check};
use implicit_predicates_test_support::{any_finite, config};
use proptest::prelude::*;

/// Enough room for `check` on any `[f64; 3]`.
const ROOM: usize = 256;

/// Finite values of every kind, the extremes more often than
/// `any_finite` alone gives them.
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
            f64::MIN_POSITIVE.next_down(),
        ]),
    ]
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn accepts_every_finite_array(p in prop::array::uniform3(coordinate())) {
        let mut buf = [0.0; ROOM];
        let mut scratch = Scratch::borrowed(&mut buf);
        prop_assert_eq!(check(&p, &mut scratch), Ok(()));
        prop_assert_eq!(scratch.used(), 0);
    }
}

#[test]
fn an_array_that_is_not_finite_is_reported() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for axis in 0..3 {
            let mut p = [1.0, -2.0, 0.5];
            p[axis] = bad;
            let mut buf = [0.0; ROOM];
            let mut scratch = Scratch::borrowed(&mut buf);
            assert_eq!(
                check(&p, &mut scratch),
                Err(CheckError::Violation(Violation::NotFinite)),
                "{p:?}"
            );
            assert_eq!(scratch.used(), 0);
        }
    }
}

#[test]
fn an_arena_too_small_is_reported() {
    let mut buf = [0.0; 2];
    let mut scratch = Scratch::borrowed(&mut buf);
    let p = [1.5, -3.0, 0.25];
    assert_eq!(check(&p, &mut scratch), Err(CheckError::Exhausted));
    assert_eq!(scratch.used(), 0);
}

#[test]
fn check_leaves_what_was_below_the_top() {
    let below = [2f64.powi(-70), 1.0, 2f64.powi(80)];
    let points = [
        [1.5, -3.0, 0.25],
        [f64::MAX, f64::from_bits(1), -0.0],
        [f64::NAN, 0.0, 0.0],
    ];
    // From an arena with no free room to one with plenty, so that each
    // point either passes, breaks the contract or runs out of room.
    for room in 0..=ROOM {
        for p in points {
            let mut buf = vec![f64::NAN; below.len() + room];
            let mut scratch = Scratch::borrowed(&mut buf);
            let span = scratch.push(&below).unwrap();
            let used = scratch.used();
            let result = check(&p, &mut scratch);
            if room == ROOM {
                assert_ne!(result, Err(CheckError::Exhausted), "{p:?}");
            }
            assert_eq!(scratch.used(), used, "{p:?} with {room} free");
            assert_eq!(scratch.get(span), &below, "{p:?} with {room} free");
        }
    }
}

#[test]
fn the_errors_explain_themselves() {
    let exhausted = CheckError::Exhausted.to_string();
    assert!(exhausted.contains("arena"), "{exhausted}");
    let violation = CheckError::Violation(Violation::ErrTooSmall { axis: 2 }).to_string();
    assert!(violation.contains("axis 2"), "{violation}");
    let source = core::error::Error::source(&CheckError::Violation(Violation::NotFinite));
    assert_eq!(
        source.map(ToString::to_string),
        Some(Violation::NotFinite.to_string())
    );
    assert!(core::error::Error::source(&CheckError::Exhausted).is_none());
}
