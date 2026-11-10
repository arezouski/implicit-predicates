//! Shared infrastructure for the property tests of this crate: proptest
//! configuration, input generators and exact checks against the oracle.

// Each integration-test crate uses only part of this module.
#![allow(dead_code, unused_imports)]

mod checks;
mod strategies;

pub use checks::assert_valid_expansion;
pub use strategies::{
    any_finite, cancellation_that_rounds, correlated_pair, expansion, expansion_with,
    in_product_domain, leaf, near_cancelling,
};

use indirect_predicates_oracle::Dyadic;
use proptest::test_runner::Config as ProptestConfig;

/// Number of cases per property when `PROPTEST_CASES` is not set.
const DEFAULT_CASES: u32 = 2000;

/// Proptest configuration shared by every property test.
///
/// The number of cases comes from the `PROPTEST_CASES` environment variable,
/// defaulting to [`DEFAULT_CASES`].
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
