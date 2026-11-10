//! The scratch arena: its operations agree with the slice primitives, and
//! its accounting of used space, marks and exhaustion is exact.

mod common;

use common::{assert_valid_expansion, expansion, expansion_with, in_product_domain, leaf};
use indirect_predicates_core::{Scratch, ScratchExhausted, Sign, Span, expansion};
use proptest::prelude::*;

/// One arena operation in a random sequence. Operand indices select, modulo
/// their number, among the spans produced so far.
#[derive(Clone, Debug)]
enum Op {
    Push(Vec<f64>),
    Sum(usize, usize),
    Diff(usize, usize),
    Scale(usize, f64),
}

/// Operations whose operands keep a sequence of up to 10 of them inside the
/// domain of `scale`: leaves lie in `2^-40 ..= 2^41` and scale factors in
/// `2^-20 ..= 2^21` with at most 8 significant bits, as in the chained
/// test of the expansion primitives.
fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        expansion_with(8, leaf(-40, 40, 53)).prop_map(Op::Push),
        (any::<usize>(), any::<usize>()).prop_map(|(a, b)| Op::Sum(a, b)),
        (any::<usize>(), any::<usize>()).prop_map(|(a, b)| Op::Diff(a, b)),
        (any::<usize>(), leaf(-20, 20, 8)).prop_map(|(a, b)| Op::Scale(a, b)),
    ]
}

/// A run of operations on an arena, recording each span it produced next to
/// the components the slice primitives give for the same operands.
struct Run {
    spans: Vec<(Span, Vec<f64>)>,
    /// The largest `used()` plus worst-case capacity of any successful
    /// operation: the expected `high_water()`.
    peak: usize,
}

impl Run {
    fn new(scratch: &Scratch<'_>) -> Self {
        Run {
            spans: Vec::new(),
            peak: scratch.used(),
        }
    }

    /// Applies `op` and checks the arena's accounting: an operation succeeds
    /// exactly when its worst-case capacity fits, a success grows `used()` by
    /// the length of the result, and a failure leaves `used()` unchanged.
    /// Operations on spans are skipped while there are none.
    fn apply(&mut self, scratch: &mut Scratch<'_>, op: &Op) {
        let pick = |i: usize| self.spans[i % self.spans.len()].clone();
        if self.spans.is_empty() && !matches!(op, Op::Push(_)) {
            return;
        }
        let used_before = scratch.used();
        let (worst_case, result, expected) = match op {
            Op::Push(e) => (e.len(), scratch.push(e), e.clone()),
            Op::Sum(a, b) => {
                let ((a, ea), (b, eb)) = (pick(*a), pick(*b));
                let expected = slice_op(ea.len() + eb.len(), |out| expansion::sum(&ea, &eb, out));
                (a.len() + b.len(), scratch.sum(a, b), expected)
            }
            Op::Diff(a, b) => {
                let ((a, ea), (b, eb)) = (pick(*a), pick(*b));
                let expected = slice_op(ea.len() + eb.len(), |out| expansion::diff(&ea, &eb, out));
                (a.len() + b.len(), scratch.diff(a, b), expected)
            }
            Op::Scale(a, factor) => {
                let (a, ea) = pick(*a);
                let expected = slice_op(2 * ea.len(), |out| expansion::scale(&ea, *factor, out));
                (2 * a.len(), scratch.scale(a, *factor), expected)
            }
        };
        let fits = used_before + worst_case <= scratch.capacity();
        match result {
            Ok(span) => {
                assert!(fits, "{op:?} succeeded without room for its worst case");
                assert_eq!(scratch.used(), used_before + span.len(), "after {op:?}");
                assert_eq!(scratch.get(span), expected.as_slice(), "after {op:?}");
                self.peak = self.peak.max(used_before + worst_case);
                self.spans.push((span, expected));
            }
            Err(ScratchExhausted) => {
                assert!(!fits, "{op:?} failed with room for its worst case");
                assert_eq!(scratch.used(), used_before, "after failed {op:?}");
            }
        }
    }

    /// Checks that every span recorded still holds its result, so that no
    /// later operation overwrote it.
    fn check_spans(&self, scratch: &Scratch<'_>) {
        for (span, expected) in &self.spans {
            assert_eq!(scratch.get(*span), expected.as_slice());
        }
    }
}

/// The result of a slice primitive, written into a buffer of `capacity`
/// components and checked to be a valid expansion.
fn slice_op(capacity: usize, operation: impl FnOnce(&mut [f64]) -> usize) -> Vec<f64> {
    let mut out = vec![f64::NAN; capacity];
    let len = operation(&mut out);
    out.truncate(len);
    assert_valid_expansion(&out);
    out
}

proptest! {
    #![proptest_config(common::config())]

    #[test]
    fn arena_ops_match_slice_ops(
        e in expansion(16),
        f in expansion(16),
        b in in_product_domain(),
    ) {
        let mut buf = vec![f64::NAN; 128];
        let mut s = Scratch::borrowed(&mut buf);
        let (se, sf) = (s.push(&e).unwrap(), s.push(&f).unwrap());
        prop_assert_eq!(s.get(se), e.as_slice());
        prop_assert_eq!(s.get(sf), f.as_slice());

        let sum = s.sum(se, sf).unwrap();
        let expected = slice_op(e.len() + f.len(), |out| expansion::sum(&e, &f, out));
        prop_assert_eq!(s.get(sum), expected.as_slice());

        let diff = s.diff(se, sf).unwrap();
        let expected = slice_op(e.len() + f.len(), |out| expansion::diff(&e, &f, out));
        prop_assert_eq!(s.get(diff), expected.as_slice());

        let scaled = s.scale(se, b).unwrap();
        let expected = slice_op(2 * e.len(), |out| expansion::scale(&e, b, out));
        prop_assert_eq!(s.get(scaled), expected.as_slice());

        prop_assert_eq!(s.sign(diff), expansion::sign(s.get(diff)));
        prop_assert_eq!(s.get(se), e.as_slice());
        prop_assert_eq!(s.get(sf), f.as_slice());
    }

    #[test]
    fn used_grows_by_result_length(
        capacity in 0usize..=96,
        ops in prop::collection::vec(op(), 0..=10),
    ) {
        let mut buf = vec![f64::NAN; capacity];
        let mut s = Scratch::borrowed(&mut buf);
        prop_assert_eq!(s.capacity(), capacity);
        let mut run = Run::new(&s);
        for op in &ops {
            run.apply(&mut s, op);
        }
        run.check_spans(&s);
        prop_assert_eq!(s.high_water(), run.peak);
    }

    #[test]
    fn mark_reset_restores_used(
        capacity in 0usize..=96,
        before in prop::collection::vec(op(), 0..=5),
        after in prop::collection::vec(op(), 0..=5),
    ) {
        let mut buf = vec![f64::NAN; capacity];
        let mut s = Scratch::borrowed(&mut buf);
        let mut run = Run::new(&s);
        for op in &before {
            run.apply(&mut s, op);
        }
        let mark = s.mark();
        let used_at_mark = s.used();
        let spans_at_mark = run.spans.len();
        for op in &after {
            run.apply(&mut s, op);
        }
        s.reset(mark);
        prop_assert_eq!(s.used(), used_at_mark);
        prop_assert_eq!(s.high_water(), run.peak);
        run.spans.truncate(spans_at_mark);
        run.check_spans(&s);
    }
}

#[test]
fn exhaustion_returns_error_and_keeps_state() {
    let mut buf = [0.0; 5];
    let mut s = Scratch::borrowed(&mut buf);
    let a = s.push(&[1e-20, 1.0]).unwrap();
    let b = s.push(&[1e-25, 3.0]).unwrap();
    assert_eq!(s.used(), 4);
    assert_eq!(s.sum(a, b), Err(ScratchExhausted)); // needs 4 more, only 1 left
    assert_eq!(s.used(), 4);
    assert_eq!(s.get(a), &[1e-20, 1.0]);
}

#[test]
fn scratch_debug_shows_accounting_only() {
    let mut buf = [0.0; 8];
    let mut s = Scratch::borrowed(&mut buf);
    let a = s.push(&[1e-20, 1.0]).unwrap();
    let _ = s.scale(a, 3.0).unwrap();
    let mark = s.mark();
    let _ = s.push(&[1.0]).unwrap();
    s.reset(mark);
    let used = s.used();
    assert_eq!(
        format!("{s:?}"),
        format!("Scratch {{ used: {used}, capacity: 8, high_water: 6 }}")
    );
}

#[test]
fn zero_capacity_arena_only_fails() {
    let mut buf: [f64; 0] = [];
    let mut s = Scratch::borrowed(&mut buf);
    assert_eq!(s.push(&[1.0]).map(|_| ()), Err(ScratchExhausted));
    let empty = s.push(&[]).unwrap();
    assert_eq!(s.sign(empty), Sign::Zero);
}

#[test]
fn empty_results_are_empty_spans() {
    let mut buf = [0.0; 8];
    let mut s = Scratch::borrowed(&mut buf);
    let a = s.push(&[1e-20, 1.0]).unwrap();
    let zero = s.diff(a, a).unwrap();
    assert!(zero.is_empty());
    assert_eq!(zero.len(), 0);
    assert_eq!(s.get(zero), &[] as &[f64]);
    assert_eq!(s.sign(zero), Sign::Zero);
    assert_eq!(s.used(), 2);
    let scaled = s.scale(zero, 3.0).unwrap();
    assert!(scaled.is_empty());
    assert_eq!(s.used(), 2);
}

#[test]
fn high_water_counts_the_worst_case_reservation() {
    let mut buf = [0.0; 8];
    let mut s = Scratch::borrowed(&mut buf);
    let a = s.push(&[1e-20, 1.0]).unwrap();
    let doubled = s.scale(a, 2.0).unwrap();
    assert_eq!(s.get(doubled), &[2e-20, 2.0]);
    // The scale reserved 4 components above the 2 in use.
    assert_eq!(s.used(), 4);
    assert_eq!(s.high_water(), 6);
    let mark = s.mark();
    let _ = s.sum(a, doubled).unwrap();
    s.reset(mark);
    assert_eq!(s.used(), 4);
    assert_eq!(s.high_water(), 8);
    // A failed operation reserves nothing.
    let long = [
        2f64.powi(-120),
        2f64.powi(-60),
        1.0,
        2f64.powi(60),
        2f64.powi(120),
    ];
    assert_eq!(s.push(&long), Err(ScratchExhausted));
    assert_eq!(s.high_water(), 8);
}

#[test]
fn spans_compare_by_position_and_length() {
    let mut buf = [0.0; 4];
    let mut s = Scratch::borrowed(&mut buf);
    let mark = s.mark();
    let first = s.push(&[1.0]).unwrap();
    s.reset(mark);
    let again = s.push(&[2.0]).unwrap();
    assert_eq!(first, again);
    assert_eq!(s.get(again), &[2.0]);
    let other = s.push(&[3.0]).unwrap();
    assert_ne!(first, other);
}

#[test]
fn exhausted_error_describes_itself() {
    use core::error::Error;
    assert_eq!(ScratchExhausted.to_string(), "scratch arena exhausted");
    assert!(ScratchExhausted.source().is_none());
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "past the top")]
fn using_a_span_released_by_reset_is_caught() {
    let mut buf = [0.0; 4];
    let mut s = Scratch::borrowed(&mut buf);
    let mark = s.mark();
    let a = s.push(&[1.0, 4.0]).unwrap();
    s.reset(mark);
    let _ = s.get(a);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "past the top")]
fn operating_on_a_span_released_by_reset_is_caught() {
    let mut buf = [0.0; 8];
    let mut s = Scratch::borrowed(&mut buf);
    let mark = s.mark();
    let a = s.push(&[1.0, 4.0]).unwrap();
    s.reset(mark);
    let _ = s.scale(a, 2.0);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "above the top")]
fn reset_to_a_mark_above_the_top_is_caught() {
    let mut buf = [0.0; 4];
    let mut s = Scratch::borrowed(&mut buf);
    let base = s.mark();
    let _ = s.push(&[1.0]).unwrap();
    let high = s.mark();
    s.reset(base);
    s.reset(high);
}

#[cfg(not(debug_assertions))]
#[test]
fn reset_to_a_mark_above_the_top_never_raises_used() {
    let mut buf = [0.0; 4];
    let mut s = Scratch::borrowed(&mut buf);
    let base = s.mark();
    let _ = s.push(&[1.0]).unwrap();
    let high = s.mark();
    s.reset(base);
    s.reset(high);
    assert_eq!(s.used(), 0);
}
