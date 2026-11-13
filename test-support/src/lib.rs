//! Shared test infrastructure for the `implicit-predicates` workspace: the
//! proptest configuration, input generators (values, expansions and sets of
//! points), structural checks of expansions, and exact values from the
//! oracle.
//!
//! This crate is not published. The crates of the workspace use it as a
//! dev-dependency only.
//!
//! The structural check ([`assert_valid_expansion`]) and the bit-pattern
//! generators ([`bit_pattern_expansion`], [`bit_pattern_pair`]) read and
//! write bit patterns directly and never call the arithmetic of
//! `implicit-predicates-core`, since they are what that arithmetic is judged
//! against. The other expansion generators are built with
//! `implicit-predicates-core` and check every intermediate result.

mod arena;
mod big;
mod bit_patterns;
mod checks;
mod expansions;
mod points;
mod values;

pub use arena::{Applied, ArenaOp, PRODUCT_OPERAND_LIMIT, arena_op, product_working_space};
pub use big::{BigSpec, any_big, assert_valid_big, big_to_dyadic};
pub use bit_patterns::{bit_pattern_expansion, bit_pattern_pair};
pub use checks::{assert_valid_expansion, has_adjacent_components};
pub use expansions::{expansion, expansion_with, expansion_within, near_equal};
pub use points::{
    LncParams, Point3, Points4, Points5, coplanar4, cospherical5, extreme4, grid4, grid5,
    lnc_params, near_coplanar4, near_cospherical5, point3, random4, random5, repeated4,
    signed_zeros4,
};
pub use values::{
    any_finite, cancellation_that_rounds, correlated_pair, extreme_value, leaf, lnc_parameter,
    near_cancelling, product_domain_bits, product_operand,
};

use implicit_predicates_oracle::Dyadic;
use proptest::strategy::{Strategy, ValueTree};
use proptest::test_runner::{Config as ProptestConfig, TestRunner};

/// Number of cases per property when `PROPTEST_CASES` is not set.
const DEFAULT_CASES: u32 = 2000;

/// Proptest configuration shared by every property test.
///
/// The number of cases comes from the `PROPTEST_CASES` environment variable,
/// defaulting to 2000.
///
/// # Panics
///
/// Panics if `PROPTEST_CASES` is set to something other than a
/// non-negative integer.
pub fn config() -> ProptestConfig {
    let cases = match std::env::var("PROPTEST_CASES") {
        Ok(text) => text
            .parse()
            .expect("PROPTEST_CASES must be a non-negative integer"),
        Err(_) => DEFAULT_CASES,
    };
    ProptestConfig::with_cases(cases)
}

/// The exact value of the sum of `xs`.
pub fn value(xs: &[f64]) -> Dyadic {
    Dyadic::sum_of(xs)
}

/// `count` values drawn from `strategy` with a fixed seed, to measure what a
/// generator produces.
///
/// # Panics
///
/// Panics if `strategy` rejects every value it tries.
pub fn sample<S: Strategy>(strategy: S, count: usize) -> Vec<S::Value> {
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
