//! The growable arena (the `alloc` feature): it grows as operations need
//! room, keeps its spans valid, and agrees with a borrowed arena.
#![cfg(feature = "alloc")]

use implicit_predicates_core::{Scratch, Span};
use implicit_predicates_test_support::{ArenaOp, arena_op, config, value};
use proptest::prelude::*;

/// Applies `op` to `scratch`, picking operands among `spans`, and records
/// the span it produced.
fn apply(scratch: &mut Scratch<'_>, spans: &mut Vec<Span>, op: &ArenaOp) -> Option<Span> {
    let span = op.apply(scratch, spans)?.result.ok();
    spans.extend(span);
    span
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn growable_matches_borrowed(ops in prop::collection::vec(arena_op(), 0..=10)) {
        let mut buf = vec![f64::NAN; 1 << 16];
        let mut borrowed = Scratch::borrowed(&mut buf);
        let mut owned = Scratch::growable(1);
        let (mut borrowed_spans, mut owned_spans) = (Vec::new(), Vec::new());
        for op in &ops {
            let expected = apply(&mut borrowed, &mut borrowed_spans, op);
            let actual = apply(&mut owned, &mut owned_spans, op);
            prop_assert_eq!(actual, expected, "{:?}", op);
            prop_assert_eq!(owned.used(), borrowed.used());
            prop_assert_eq!(owned.high_water(), borrowed.high_water());
            prop_assert!(owned.capacity() >= owned.high_water());
        }
        for (a, b) in owned_spans.iter().zip(&borrowed_spans) {
            prop_assert_eq!(owned.get(*a), borrowed.get(*b));
        }
    }
}

#[test]
fn growable_arena_grows_and_keeps_spans_valid() {
    let mut s = Scratch::growable(2);
    let a = s.push(&[1e-20, 1.0]).unwrap();
    let b = s.push(&[1e-25, 3.0]).unwrap(); // forces growth
    let p = s.product(a, b).unwrap(); // forces more growth
    assert_eq!(s.get(a), &[1e-20, 1.0]);
    assert_eq!(s.get(b), &[1e-25, 3.0]);
    assert_eq!(
        value(s.get(p)),
        &value(&[1e-20, 1.0]) * &value(&[1e-25, 3.0])
    );
    assert!(s.capacity() >= s.used());
}

#[test]
fn growable_arena_with_zero_initial_capacity_works() {
    let mut s = Scratch::growable(0);
    assert_eq!(s.push(&[2.0]).map(|sp| sp.len()), Ok(1));
}

/// A valid expansion of `N` components, each 2^3 times the previous.
fn spread<const N: usize>() -> [f64; N] {
    core::array::from_fn(|i| 8f64.powi(i32::try_from(i).unwrap()))
}

#[test]
fn growable_arena_grows_at_least_to_double_or_to_what_is_needed() {
    let mut s = Scratch::growable(4);
    assert_eq!(s.capacity(), 4);
    let _ = s.push(&[1.0]).unwrap();
    let _ = s.push(&spread::<4>()).unwrap(); // needs 5, doubling gives 8
    assert_eq!(s.capacity(), 8);
    let _ = s.push(&spread::<20>()).unwrap(); // needs 25, doubling gives 16
    assert_eq!(s.capacity(), 25);
}
