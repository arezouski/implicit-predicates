//! Self-tests of the shared test infrastructure: the expansion validity check
//! must reject each kind of invalid input, and the generators must produce
//! the kinds of inputs they promise.

mod common;

use common::{assert_valid_expansion, correlated_pair, expansion};
use indirect_predicates_core::eft;
use proptest::strategy::{Strategy, ValueTree};
use proptest::test_runner::TestRunner;

#[test]
#[should_panic(expected = "overlap or are out of order")]
fn rejects_equal_components() {
    assert_valid_expansion(&[1.0, 1.0]);
}

#[test]
#[should_panic(expected = "overlap or are out of order")]
fn rejects_decreasing_magnitudes() {
    assert_valid_expansion(&[2.0, 1.0]);
}

#[test]
#[should_panic(expected = "is zero or not finite")]
fn rejects_zero_components() {
    assert_valid_expansion(&[0.0, 1.0]);
}

#[test]
#[should_panic(expected = "is zero or not finite")]
fn rejects_non_finite_components() {
    assert_valid_expansion(&[1.0, f64::INFINITY]);
}

#[test]
#[should_panic(expected = "overlap or are out of order")]
fn rejects_overlap_when_the_smaller_has_several_bits() {
    assert_valid_expansion(&[3.0, 5.0]);
}

#[test]
#[should_panic(expected = "overlap or are out of order")]
fn rejects_overlap_with_fractional_parts() {
    assert_valid_expansion(&[1.5, 2.5]);
}

#[test]
fn accepts_valid_expansions() {
    assert_valid_expansion(&[]);
    assert_valid_expansion(&[1.0, 2.0]);
    // Adjacent components (one bit position apart) do not overlap.
    assert_valid_expansion(&[3.0, 4.0]);
    assert_valid_expansion(&[1.5, 2.0]);
    assert_valid_expansion(&[5e-324, 1.0]);
    assert_valid_expansion(&[-1.0, 2.0, -8.0]);
}

/// `count` values drawn from `strategy` with a fixed seed.
fn sample<S: Strategy>(strategy: S, count: usize) -> Vec<S::Value> {
    let mut runner = TestRunner::deterministic();
    (0..count)
        .map(|_| {
            strategy
                .new_tree(&mut runner)
                .expect("the strategy has no filters that could reject every value")
                .current()
        })
        .collect()
}

#[test]
fn generated_expansions_often_have_several_components() {
    // Components whose bits all fit in 53 consecutive positions merge into
    // one, so leaves within `2^-200 ..= 2^200` give at most about
    // 400 / 53 + 1 = 8 components.
    let samples = sample(expansion(16), 1000);
    let several = samples.iter().filter(|e| e.len() >= 3).count();
    let many = samples.iter().filter(|e| e.len() >= 6).count();
    assert!(
        several >= 700,
        "only {several} of 1000 have 3 or more components"
    );
    assert!(many >= 150, "only {many} of 1000 have 6 or more components");
}

#[test]
fn correlated_pairs_usually_round() {
    let samples = sample(correlated_pair(), 1000);
    let rounded = samples
        .iter()
        .filter(|&&(a, b)| eft::two_sum(a, b).1 != 0.0)
        .count();
    assert!(rounded >= 850, "only {rounded} of 1000 sums round");
}
