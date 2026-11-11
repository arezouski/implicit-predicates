//! Misuse of the scratch arena that debug builds catch: spans released by a
//! reset, marks above the top, invalid expansions pushed, and operands
//! outside the product domain.

use implicit_predicates_core::Scratch;

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

/// Pushes `e`, which is not a valid expansion.
#[cfg(debug_assertions)]
fn push_invalid(e: &[f64]) {
    let mut buf = [0.0; 4];
    let _ = Scratch::borrowed(&mut buf).push(e);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "not a valid expansion")]
fn push_rejects_overlapping_components_in_debug_builds() {
    push_invalid(&[3.0, 5.0]);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "not a valid expansion")]
fn push_rejects_decreasing_magnitudes_in_debug_builds() {
    push_invalid(&[2.0, 1.0]);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "not a valid expansion")]
fn push_rejects_zero_components_in_debug_builds() {
    push_invalid(&[0.0, 1.0]);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "not a valid expansion")]
fn push_rejects_non_finite_components_in_debug_builds() {
    push_invalid(&[1.0, f64::INFINITY]);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "not a valid expansion")]
fn push_rejects_nan_in_debug_builds() {
    push_invalid(&[f64::NAN]);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "outside the product domain")]
fn scale_checks_the_domain_in_debug_builds() {
    let mut buf = [0.0; 8];
    let mut s = Scratch::borrowed(&mut buf);
    let a = s.push(&[1.0]).unwrap();
    let _ = s.scale(a, implicit_predicates_core::eft::PRODUCT_OPERAND_MIN / 2.0);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "outside the product domain")]
fn product_checks_the_domain_in_debug_builds() {
    let mut buf = [0.0; 64];
    let mut s = Scratch::borrowed(&mut buf);
    let a = s
        .push(&[
            1.0,
            implicit_predicates_core::eft::PRODUCT_OPERAND_MAX * 2.0,
        ])
        .unwrap();
    let b = s.push(&[0.5, 3.0]).unwrap();
    let _ = s.product(a, b);
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
