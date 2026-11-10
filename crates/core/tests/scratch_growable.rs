//! The growable arena (the `alloc` feature): it grows as operations need
//! room, keeps its spans valid, and agrees with a borrowed arena.
#![cfg(feature = "alloc")]

mod common;

use common::{expansion_with, leaf, value};
use indirect_predicates_core::{Scratch, Span};
use proptest::prelude::*;

/// One arena operation in a random sequence. Operand indices select, modulo
/// their number, among the spans produced so far.
#[derive(Clone, Debug)]
enum Op {
    Push(Vec<f64>),
    Sum(usize, usize),
    Diff(usize, usize),
    Scale(usize, f64),
    Product(usize, usize),
}

/// Operations on leaves in `2^-40 ..= 2^41`, with scale factors of at most 8
/// significant bits in `2^-20 ..= 2^21`.
fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        expansion_with(8, leaf(-40, 40, 53)).prop_map(Op::Push),
        (any::<usize>(), any::<usize>()).prop_map(|(a, b)| Op::Sum(a, b)),
        (any::<usize>(), any::<usize>()).prop_map(|(a, b)| Op::Diff(a, b)),
        (any::<usize>(), leaf(-20, 20, 8)).prop_map(|(a, b)| Op::Scale(a, b)),
        (any::<usize>(), any::<usize>()).prop_map(|(a, b)| Op::Product(a, b)),
    ]
}

/// Whether `product` may take `span`: short, with components well inside the
/// domain of `two_product`, so that a sequence of operations stays in domain
/// and its working space stays modest.
fn fits_product(scratch: &Scratch<'_>, span: Span) -> bool {
    span.len() <= 4
        && scratch
            .get(span)
            .iter()
            .all(|c| (2f64.powi(-200)..=2f64.powi(200)).contains(&c.abs()))
}

/// Applies `op` to `scratch`, picking operands among `spans`, and records
/// the span it produced. Operations on spans are skipped while there are
/// none.
fn apply_plain(scratch: &mut Scratch<'_>, spans: &mut Vec<Span>, op: &Op) -> Option<Span> {
    let pick = |i: usize| spans[i % spans.len()];
    if spans.is_empty() && !matches!(op, Op::Push(_)) {
        return None;
    }
    let result = match op {
        Op::Push(e) => scratch.push(e),
        Op::Sum(a, b) => scratch.sum(pick(*a), pick(*b)),
        Op::Diff(a, b) => scratch.diff(pick(*a), pick(*b)),
        Op::Scale(a, f) => scratch.scale(pick(*a), *f),
        Op::Product(a, b) => {
            let (a, b) = (pick(*a), pick(*b));
            if !(fits_product(scratch, a) && fits_product(scratch, b)) {
                return None;
            }
            scratch.product(a, b)
        }
    };
    let span = result.ok();
    spans.extend(span);
    span
}

proptest! {
    #![proptest_config(common::config())]

    #[test]
    fn growable_matches_borrowed(ops in prop::collection::vec(op(), 0..=10)) {
        let mut buf = vec![f64::NAN; 1 << 16];
        let mut borrowed = Scratch::borrowed(&mut buf);
        let mut owned = Scratch::growable(1);
        let (mut borrowed_spans, mut owned_spans) = (Vec::new(), Vec::new());
        for op in &ops {
            let expected = apply_plain(&mut borrowed, &mut borrowed_spans, op);
            let actual = apply_plain(&mut owned, &mut owned_spans, op);
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
