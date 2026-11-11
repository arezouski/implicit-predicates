//! The generator returns an error, quickly, on definitions that parse but are
//! too large to process: it never panics, hangs or runs out of memory on
//! them, and it still accepts the real predicates with a wide margin.

use std::time::{Duration, Instant};

use implicit_predicates_gen::{
    Error, canonical, difference_graph, filter_bound, generate, parse, parse_points,
};

/// How the tests say to regenerate the code.
const REGENERATE: &str = "run the tests.";

const ORIENT3D: &str = include_str!("fixtures/orient3d.pred");
const INSPHERE: &str = include_str!("fixtures/insphere.pred");

/// A generous limit on the time to reject a definition, even in a debug
/// build on a shared machine several times slower than a desktop: it tells
/// a bounded rejection from a hang, not a fast one from a slow one.
const LIMIT: Duration = Duration::from_secs(30);

/// The most subexpressions of a main form for which the checks are
/// generated.
const MAX_NODES: usize = 256;

/// Runs `generate` on `source` and returns its result with the time it took.
fn timed(source: &str) -> (Result<usize, Error>, Duration) {
    let start = Instant::now();
    let result = generate(source, "t.pred", REGENERATE).map(|all| all.len());
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

/// A predicate whose main form is a balanced sum of `terms` products of two
/// differences, cycling through the products of the differences of four
/// points, written with `let` so that brackets stay shallow and the text
/// stays short.
fn balanced_sum(terms: usize) -> String {
    let points = ["a", "b", "c", "d"];
    let mut lines = String::new();
    let mut leaves = 0;
    for p in points {
        for q in points.iter().filter(|&&q| q != p) {
            for axis in ["x", "y", "z"] {
                lines.push_str(&format!("    let l{leaves} = {p}.{axis} - {q}.{axis};\n"));
                leaves += 1;
            }
        }
    }
    let mut level: Vec<String> = (0..terms)
        .map(|i| format!("l{} * l{}", i % leaves, (i / leaves + i) % leaves))
        .collect();
    let mut next_name = 0;
    while level.len() > 1 {
        let mut next = Vec::new();
        for pair in level.chunks(2) {
            if let [x, y] = pair {
                lines.push_str(&format!("let s{next_name} = {x} + {y};\n"));
                next.push(format!("s{next_name}"));
                next_name += 1;
            } else {
                next.push(pair[0].clone());
            }
        }
        level = next;
    }
    format!(
        "fn p(a: Point3, b: Point3, c: Point3, d: Point3) -> Sign {{\n{lines}    {}\n}}\n",
        level[0]
    )
}

#[test]
fn a_wide_balanced_sum_is_rejected_quickly() {
    // Shallow enough for every other budget, but too many subexpressions
    // for the search of the edge cases of the checks.
    rejected_quickly(&balanced_sum(250_000), (1, 4), "subexpressions");
}

#[test]
fn the_largest_accepted_chain_generates_in_a_few_seconds() {
    // The worst shape found for the search of the edge cases: a sum of one
    // difference, whose edge cases are hard to find, at the largest number
    // of subexpressions accepted.
    let start = Instant::now();
    assert_eq!(timed(&long_sum(MAX_NODES)).0, Ok(1));
    let elapsed = start.elapsed();
    assert!(elapsed < Duration::from_secs(90), "took {elapsed:?}");
    rejected_quickly(&long_sum(MAX_NODES + 1), (1, 4), "subexpressions");
}

#[test]
fn a_point_of_250000_terms_is_read_in_time() {
    // A flat chain: read in a loop, so no stack depth and no limit but time.
    let terms = "p + t * q + ".repeat(125_000);
    let source = format!("point A(p: F64x3, q: F64x3, t: f64) = {terms}p;");
    let start = Instant::now();
    let points = parse_points(&source, "t.pred").unwrap();
    assert!(start.elapsed() < LIMIT, "took {:?}", start.elapsed());
    assert_eq!(points[0].graph.roots.len(), 3);
}

#[test]
fn a_point_with_a_huge_where_side_is_read_in_time() {
    let side = "t + 1 + ".repeat(125_000);
    let source = format!("point A(p: F64x3, t: f64) = p * t where {side}t <= 0;");
    let start = Instant::now();
    let points = parse_points(&source, "t.pred").unwrap();
    assert!(start.elapsed() < LIMIT, "took {:?}", start.elapsed());
    assert_eq!(points[0].constraints[0].lhs.params.len(), 125_001);
    assert_eq!(points[0].constraints[0].lhs.constant, 125_000);
}

#[test]
fn a_long_run_of_unary_minus_in_a_point_is_read_in_time() {
    let source = format!("point A(p: F64x3) = {}p;", "-".repeat(250_000));
    let start = Instant::now();
    let points = parse_points(&source, "t.pred").unwrap();
    assert!(start.elapsed() < LIMIT, "took {:?}", start.elapsed());
    assert_eq!(points[0].graph.nodes.len(), 3 + 3 * 250_000);
}

#[test]
fn points_nested_32_deep_are_read_and_one_more_is_rejected() {
    let nested = |depth: usize| {
        format!(
            "point A(p: F64x3, t: f64) = {}p * t{};",
            "(".repeat(depth),
            ")".repeat(depth)
        )
    };
    let start = Instant::now();
    assert!(parse_points(&nested(32), "t.pred").is_ok());
    let e = parse_points(&nested(33), "t.pred").expect_err("33 levels");
    assert!(start.elapsed() < LIMIT, "took {:?}", start.elapsed());
    assert_eq!((e.line, e.column), (1, 28 + 33), "{e}");
    assert!(e.message.contains("nested more than 32 deep"), "{e}");
    // Far deeper, and unbalanced: an error, not a stack overflow.
    for source in [
        nested(100_000),
        format!("point A(p: F64x3) = {}", "(".repeat(100_000)),
    ] {
        assert!(parse_points(&source, "t.pred").is_err());
    }
}
