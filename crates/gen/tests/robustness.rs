//! The generator returns an error, quickly, on definitions that parse but are
//! too large to process: it never panics, hangs or runs out of memory on
//! them, and it still accepts the real predicates with a wide margin.

use std::time::{Duration, Instant};

use indirect_predicates_gen::{Error, canonical, difference_graph, filter_bound, generate, parse};

const ORIENT3D: &str = include_str!("fixtures/orient3d.pred");
const INSPHERE: &str = include_str!("fixtures/insphere.pred");

/// A generous limit on the time to reject a definition, even in a debug
/// build.
const LIMIT: Duration = Duration::from_secs(5);

/// Runs `generate` on `source` and returns its result with the time it took.
fn timed(source: &str) -> (Result<usize, Error>, Duration) {
    let start = Instant::now();
    let result = generate(source, "t.pred").map(|all| all.len());
    (result, start.elapsed())
}

/// Asserts that `source` is rejected within [`LIMIT`] at `(line, column)`
/// of `t.pred`, with a message containing `fragment`.
fn rejected_quickly(source: &str, (line, column): (usize, usize), fragment: &str) {
    let (result, elapsed) = timed(source);
    let e = result.expect_err("the definition must be rejected");
    assert!(elapsed < LIMIT, "took {elapsed:?}: {e}");
    assert_eq!(e.file, "t.pred");
    assert_eq!((e.line, e.column), (line, column), "{e}");
    assert!(e.message.contains(fragment), "{e}");
}

/// `let` lines that square `base` `times` times, the last named `t{times}`.
fn squarings(base: &str, times: usize) -> String {
    let mut lines = format!("    let t0 = {base};\n");
    for i in 1..=times {
        lines.push_str(&format!("    let t{i} = t{} * t{};\n", i - 1, i - 1));
    }
    lines
}

#[test]
fn a_form_squared_32_times_is_an_error_not_a_panic() {
    // Squaring `a.x` 32 times makes its exponent 2^32, one more than a
    // `u32` holds.
    let source = format!(
        "fn p(a: Point3, d: Point3) -> Sign {{\n    a.x - d.x\n}}\n\
         #[exact]\nfn p(a: Point3, d: Point3) -> Sign {{\n{}    t32 - d.x\n}}\n",
        squarings("a.x", 32)
    );
    rejected_quickly(&source, (5, 4), "degree");
}

#[test]
fn canonical_returns_an_error_on_a_form_too_large_to_expand() {
    // The public expansion itself, without the cheaper checks before it.
    let exponent = format!(
        "fn p(a: Point3, d: Point3) -> Sign {{\n{}    t32 - d.x\n}}\n",
        squarings("a.x", 32)
    );
    let wide = format!(
        "fn p(a: Point3, d: Point3) -> Sign {{\n{}    t5\n}}\n",
        squarings("(a.x - d.x) + (a.y - d.y) + (a.z - d.z)", 5)
    );
    let deep = format!(
        "fn p(a: Point3, d: Point3) -> Sign {{\n{}    t14\n}}\n",
        squarings("a.x - d.x", 14)
    );
    for (source, fragment) in [
        (exponent, "a degree above 4294967295"),
        (wide, "operations on its terms"),
        (deep, "operations on its terms"),
    ] {
        let p = parse(&source, "t.pred").unwrap().remove(0);
        let start = Instant::now();
        let e = canonical(&p.main, "t.pred", p.position).expect_err("too large");
        assert!(start.elapsed() < LIMIT, "took {:?}", start.elapsed());
        assert_eq!((e.file.as_str(), e.line, e.column), ("t.pred", 1, 4), "{e}");
        assert!(e.message.contains(fragment), "{e}");
    }
}

#[test]
fn a_high_degree_main_form_is_rejected_before_expansion() {
    // Degree 2^14 in the differences: expanding it would take minutes.
    let source = format!(
        "fn p(a: Point3, d: Point3) -> Sign {{\n{}    t14\n}}\n",
        squarings("a.x - d.x", 14)
    );
    rejected_quickly(&source, (1, 4), "the degree 16384 is too high");
}

#[test]
fn a_wide_base_squared_repeatedly_is_rejected_quickly() {
    // Degree 32 only, which the filter accepts, but hundreds of thousands
    // of terms in six variables once expanded.
    let source = format!(
        "fn p(a: Point3, d: Point3) -> Sign {{\n{}    t5\n}}\n",
        squarings("(a.x - d.x) + (a.y - d.y) + (a.z - d.z)", 5)
    );
    rejected_quickly(&source, (1, 4), "too large to expand");
}

/// A predicate on one line whose main form is the sum of `terms` copies of
/// one difference.
fn long_sum(terms: usize) -> String {
    let sum = vec!["(a.x - b.x)"; terms].join(" + ");
    format!("fn p(a: Point3, b: Point3) -> Sign {{ {sum} }}")
}

#[test]
fn a_very_long_sum_is_rejected_quickly() {
    for terms in [2_000, 8_000, 30_000, 100_000] {
        rejected_quickly(&long_sum(terms), (1, 4), "error bound of the filter");
    }
}

#[test]
fn a_sum_of_a_few_hundred_terms_is_accepted() {
    // The budget of the filter bound admits a chain of about 1,560 terms.
    for terms in [400, 1_500] {
        let p = parse(&long_sum(terms), "t.pred").unwrap().remove(0);
        let g = difference_graph(&p.main, "t.pred", p.position).unwrap();
        let start = Instant::now();
        assert!(filter_bound(&g, "t.pred", p.position).is_ok(), "{terms}");
        assert!(start.elapsed() < LIMIT, "took {:?}", start.elapsed());
    }
    // The whole generation, whose search for the edge cases of the checks
    // takes time in proportion to the number of terms.
    assert_eq!(timed(&long_sum(100)).0, Ok(1));
}

#[test]
fn the_real_predicates_are_accepted() {
    for source in [ORIENT3D, INSPHERE] {
        let (result, elapsed) = timed(source);
        assert_eq!(result, Ok(1));
        assert!(elapsed < LIMIT, "took {elapsed:?}");
    }
}
