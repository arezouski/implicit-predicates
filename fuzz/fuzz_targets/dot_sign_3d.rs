//! Fuzz target; see `fuzz/README.md`.
#![no_main]

use implicit_predicates_fuzz::{decode, run_dot_sign_3d};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Some(case) = decode(data) {
        run_dot_sign_3d(&case);
    }
});
