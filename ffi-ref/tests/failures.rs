//! How the C interface ends the process when a reference library leaves
//! the rounding mode changed or calls its error routine.
//!
//! Each case runs a test hook in a child process: this test executable,
//! running only the ignored `child` test with core dumps disabled.

use implicit_predicates_ffi_ref::test_hooks::{self, TestHook};
use std::process::{Command, Output};

/// Names the version and hook the child runs.
const CHILD_CASE: &str = "IPREF_CHILD_CASE";

const CASES: [(&str, TestHook); 4] = [
    ("error-routine", TestHook::ErrorRoutine),
    ("return-rounding-upward", TestHook::ReturnRoundingUpward),
    ("enter-rounding-upward", TestHook::EnterRoundingUpward),
    (
        "enter-sse-rounding-upward",
        TestHook::EnterSseRoundingUpward,
    ),
];

#[test]
#[ignore = "run in a child process by the other tests of this file"]
fn child() {
    let Ok(case) = std::env::var(CHILD_CASE) else {
        return;
    };
    let (version, name) = case.split_once(':').unwrap();
    let hook = CASES.iter().find(|(n, _)| *n == name).unwrap().1;
    match version {
        "legacy" => test_hooks::legacy(hook),
        "current" => test_hooks::current(hook),
        other => panic!("unknown version {other}"),
    }
    // Reaching this point means the hook returned: the interface did not
    // end the process. Exit with a status the parent does not expect.
    std::process::exit(42);
}

fn run_child(version: &str, case: &str) -> Output {
    Command::new("sh")
        .args(["-c", "ulimit -c 0; exec \"$0\" \"$@\""])
        .arg(std::env::current_exe().unwrap())
        .args([
            "child",
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_CASE, format!("{version}:{case}"))
        .output()
        .unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn the_error_routine_fails_the_process() {
    for version in ["legacy", "current"] {
        let output = run_child(version, "error-routine");
        let err = stderr(&output);
        assert_eq!(output.status.code(), Some(1), "{version}: {err}");
        assert!(
            err.contains("ipref: the reference library ended the process from inside a call"),
            "{version}: {err}"
        );
    }
}

#[test]
fn returning_with_directed_rounding_aborts() {
    for version in ["legacy", "current"] {
        let output = run_child(version, "return-rounding-upward");
        let err = stderr(&output);
        // Killed by a signal (the abort), not exited.
        assert_eq!(output.status.code(), None, "{version}: {err}");
        assert!(
            err.contains(
                "ipref: the reference library returned with the rounding mode not to nearest"
            ),
            "{version}: {err}"
        );
    }
}

#[test]
fn entering_with_directed_rounding_aborts() {
    for version in ["legacy", "current"] {
        let output = run_child(version, "enter-rounding-upward");
        let err = stderr(&output);
        assert_eq!(output.status.code(), None, "{version}: {err}");
        assert!(
            err.contains("ipref: called with the rounding mode not to nearest"),
            "{version}: {err}"
        );
    }
}

#[test]
fn entering_with_directed_rounding_in_the_sse_control_register_only_aborts() {
    // The arithmetic of Rust code on x86_64 follows the rounding mode of the
    // SSE control register, which can differ from the x87 one.
    for version in ["legacy", "current"] {
        let output = run_child(version, "enter-sse-rounding-upward");
        let err = stderr(&output);
        assert_eq!(output.status.code(), None, "{version}: {err}");
        assert!(
            err.contains("ipref: called with the rounding mode not to nearest"),
            "{version}: {err}"
        );
    }
}
