//! The command line tool reports the definitions that the generator cannot
//! emit as `file:line:column: message`, quickly, and writes nothing: none of
//! them reaches rustfmt or the compiler.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// A generous limit on the time to reject a definition, as in the tests of
/// the library.
const LIMIT: Duration = Duration::from_secs(30);

/// Runs the tool on a directory holding `a.pred` with `source`, in a fresh
/// temporary directory, and returns its output, the time it took and the
/// directory, which the caller removes.
fn run(source: &str) -> (Output, Duration, PathBuf) {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "implicit-predicates-gen-cli-limits-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("predicates")).unwrap();
    fs::write(root.join("predicates/a.pred"), source).unwrap();
    let start = Instant::now();
    let output = Command::new(env!("CARGO_BIN_EXE_implicit-predicates-gen"))
        .current_dir(&root)
        .args(["predicates", "--stages", "stages", "--checks", "checks"])
        .output()
        .unwrap();
    (output, start.elapsed(), root)
}

/// Asserts that the tool rejects `source` at `line:column` of `a.pred`
/// with a message containing `fragment`, within [`LIMIT`], writing
/// nothing.
fn rejected(source: &str, (line, column): (usize, usize), fragment: &str) {
    let (output, elapsed, root) = run(source);
    let message = String::from_utf8_lossy(&output.stderr).into_owned();
    let wrote = root.join("stages").exists() || root.join("checks").exists();
    let _ = fs::remove_dir_all(&root);
    assert!(!output.status.success(), "{message}");
    assert!(elapsed < LIMIT, "took {elapsed:?}: {message}");
    let at = format!(
        "{}:{line}:{column}: ",
        Path::new("predicates").join("a.pred").display()
    );
    assert!(message.starts_with(&at), "{message}");
    assert!(message.contains(fragment), "{message}");
    assert!(!message.contains("a bug in the generator"), "{message}");
    assert!(!wrote, "{message}");
}

#[test]
fn a_point_of_too_many_operations_is_reported_at_its_name() {
    for count in [10_001, 200_001] {
        rejected(
            &format!("point N(p: F64x3) = {}p;\n", "-".repeat(count)),
            (1, 7),
            "operations",
        );
    }
}

#[test]
fn a_parameter_named_like_a_variant_or_a_constant_is_reported() {
    for name in [
        "Ok",
        "Err",
        "Some",
        "None",
        "UNIT",
        "TINY",
        "SUM_OPERAND_LIMIT",
    ] {
        rejected(
            &format!("point L(p: F64x3, q: F64x3, {name}: f64) = q + {name}*(p - q);\n"),
            (1, 29),
            "must start with a lowercase letter",
        );
    }
}
