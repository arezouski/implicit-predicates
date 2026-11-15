//! Writes into the arena through a closure: `Scratch::write` keeps what the
//! closure wrote, accounts for the space it lent, and fails or panics as
//! documented.

use implicit_predicates_core::{Scratch, ScratchExhausted};
use implicit_predicates_test_support::{config, expansion};
use proptest::prelude::*;

/// Writes `e` into `out` and returns its length, as a closure given to
/// `Scratch::write` does.
fn copy(e: &[f64]) -> impl FnOnce(&mut [f64]) -> usize + '_ {
    move |out| {
        out[..e.len()].copy_from_slice(e);
        e.len()
    }
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn write_keeps_what_the_closure_wrote(
        before in expansion(8),
        e in expansion(16),
        extra in 0usize..=4,
    ) {
        let mut buf = vec![f64::NAN; 32];
        let mut s = Scratch::borrowed(&mut buf);
        let first = s.push(&before).unwrap();
        let used_before = s.used();
        let lent = e.len() + extra;
        let span = s.write(lent, |out| {
            assert_eq!(out.len(), lent, "the closure is lent exactly the capacity");
            copy(&e)(out)
        }).unwrap();
        prop_assert_eq!(span.len(), e.len());
        prop_assert_eq!(s.get(span), e.as_slice());
        prop_assert_eq!(s.used(), used_before + e.len());
        prop_assert_eq!(s.high_water(), used_before + lent);
        prop_assert_eq!(s.get(first), before.as_slice());
    }
}

#[test]
fn write_of_nothing_gives_an_empty_span() {
    let mut buf: [f64; 0] = [];
    let mut s = Scratch::borrowed(&mut buf);
    let span = s.write(0, |_| 0).unwrap();
    assert!(span.is_empty());
    assert_eq!((s.used(), s.high_water()), (0, 0));
}

#[test]
fn write_that_does_not_fit_fails_and_keeps_the_arena() {
    let mut buf = [0.0; 4];
    let mut s = Scratch::borrowed(&mut buf);
    let a = s.push(&[1e-20, 1.0]).unwrap();
    let mut called = false;
    let result = s.write(3, |_| {
        called = true;
        0
    });
    assert_eq!(result, Err(ScratchExhausted));
    assert!(!called, "the closure runs only once the space is reserved");
    assert_eq!((s.used(), s.high_water()), (2, 2));
    assert_eq!(s.get(a), &[1e-20, 1.0]);
    assert_eq!(s.write(usize::MAX, |_| 0), Err(ScratchExhausted));
    assert_eq!((s.used(), s.high_water()), (2, 2));
}

#[cfg(feature = "alloc")]
#[test]
fn write_grows_a_growable_arena() {
    let mut s = Scratch::growable(1);
    let a = s.push(&[2.0]).unwrap();
    let span = s.write(3, copy(&[1e-20, 1.0, 1e20])).unwrap();
    assert!(s.capacity() >= 4);
    assert_eq!(s.get(a), &[2.0]);
    assert_eq!(s.get(span), &[1e-20, 1.0, 1e20]);
    assert_eq!((s.used(), s.high_water()), (4, 4));
}

#[test]
#[should_panic(expected = "returned a length of 3, more than the capacity of 2")]
fn write_panics_when_the_closure_returns_more_than_the_capacity() {
    let mut buf = [0.0; 8];
    let _ = Scratch::borrowed(&mut buf).write(2, |_| 3);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "not a valid expansion")]
fn write_of_an_invalid_expansion_is_caught_in_debug_builds() {
    let mut buf = [0.0; 4];
    let _ = Scratch::borrowed(&mut buf).write(2, copy(&[3.0, 5.0]));
}

// Panics abort on wasm32, so a panic cannot be caught there.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn after_the_panic_the_top_is_back_but_the_high_water_mark_has_risen() {
    let mut buf = [0.0; 8];
    let mut s = Scratch::borrowed(&mut buf);
    let kept = s.push(&[1.0]).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = s.write(5, |_| 6);
    }));
    assert!(result.is_err());
    assert_eq!((s.used(), s.high_water()), (1, 6));
    assert_eq!(s.get(kept), &[1.0]);
}
