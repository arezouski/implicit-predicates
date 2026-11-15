//! Fuzz target; see `fuzz/README.md`.
#![no_main]

use implicit_predicates_fuzz::{decode, run_point_check};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Some(case) = decode(data) {
        run_point_check(&case);
    }
});
