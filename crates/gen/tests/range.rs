//! Tests for the ranges and buffer sizes of the exact stages.

use indirect_predicates_gen::{
    Error, ExactPlan, Leaves, canonical, difference_graph, exact_plan_differences, exact_plan_raw,
    parse,
};

const ORIENT3D: &str = include_str!("fixtures/orient3d.pred");
const INSPHERE: &str = include_str!("fixtures/insphere.pred");
const INSPHERE_RAW: &str = include_str!("fixtures/insphere_raw.pred");
const DIFFERENCE: &str = "fn p(a: Point3, b: Point3) -> Sign { a.x - b.x }";

/// The product domain of the arithmetic core, `[2^-480, 2^480]`; the
/// generator passes the core's own constants.
fn domain() -> (f64, f64) {
    (2f64.powi(-480), 2f64.powi(480))
}

/// The plan of the raw form, the `#[exact]` form if the file has one and the
/// main form otherwise, with the id of its root.
fn try_raw(source: &str) -> Result<(ExactPlan, usize), Error> {
    let p = parse(source, "t.pred").unwrap().remove(0);
    let position = p.exact_position.unwrap_or(p.position);
    let g = p.exact.unwrap_or(p.main);
    let (min, max) = domain();
    exact_plan_raw(&g, "t.pred", position, min, max).map(|plan| (plan, g.root))
}

/// The plan of the difference form of the main form, with the id of its
/// root.
fn try_differences(source: &str, leaves: Leaves) -> Result<(ExactPlan, usize), Error> {
    let p = parse(source, "t.pred").unwrap().remove(0);
    let g = difference_graph(&p.main, "t.pred", p.position).unwrap();
    let (min, max) = domain();
    exact_plan_differences(&g, leaves, "t.pred", p.position, min, max).map(|plan| (plan, g.root))
}

fn raw(source: &str) -> (ExactPlan, usize) {
    try_raw(source).unwrap()
}

fn differences(source: &str, leaves: Leaves) -> (ExactPlan, usize) {
    try_differences(source, leaves).unwrap()
}

/// Asserts that a definition on one line is rejected at the name of the
/// predicate with a message containing `fragment`.
fn rejected(result: Result<(ExactPlan, usize), Error>, fragment: &str) {
    let e = result.expect_err("the definition must be rejected");
    assert!(e.message.contains(fragment), "{e}");
    assert_eq!(e.file, "t.pred");
    assert_eq!((e.line, e.column), (1, 4), "{e}");
}

/// A predicate on one line that squares `(a.x - b.x)` `levels` times, then
/// multiplies the result by `last`.
fn squarings(levels: usize, last: &str) -> String {
    let mut body = String::from("let t0 = a.x - b.x; ");
    for i in 1..=levels {
        body += &format!("let t{i} = t{} * t{}; ", i - 1, i - 1);
    }
    format!("fn p(a: Point3, b: Point3) -> Sign {{ {body}t{levels} * {last} }}")
}

fn root_length((plan, root): &(ExactPlan, usize)) -> usize {
    assert_eq!(plan.total, plan.lengths.iter().sum::<usize>());
    plan.lengths[*root]
}

/// The range as the exponents of its two powers of two.
fn exponents((plan, _): &(ExactPlan, usize)) -> (i32, i32) {
    let exponent = |x: f64| {
        assert!(
            x.is_normal() && x > 0.0 && x.to_bits() & ((1 << 52) - 1) == 0,
            "{x}"
        );
        x.log2() as i32
    };
    (exponent(plan.range.min), exponent(plan.range.max))
}

#[test]
fn orient3d_lengths_match_the_known_worst_cases() {
    assert_eq!(root_length(&raw(ORIENT3D)), 96);
    assert_eq!(root_length(&differences(ORIENT3D, Leaves::Two)), 192);
    assert_eq!(root_length(&differences(ORIENT3D, Leaves::One)), 24);
}

#[test]
fn insphere_lengths_match_the_known_worst_cases() {
    assert_eq!(root_length(&raw(INSPHERE_RAW)), 5_760);
    assert_eq!(root_length(&differences(INSPHERE, Leaves::Two)), 36_864);
    assert_eq!(root_length(&differences(INSPHERE, Leaves::One)), 1_152);
}

#[test]
fn the_two_insphere_forms_are_the_same_polynomial() {
    let raw = parse(INSPHERE_RAW, "t.pred").unwrap().remove(0);
    let differences = parse(INSPHERE, "t.pred").unwrap().remove(0);
    let polynomial = canonical(&raw.main, "t.pred", raw.position).unwrap();
    assert!(!polynomial.is_zero());
    let expected = canonical(&differences.main, "t.pred", differences.position).unwrap();
    assert_eq!(polynomial, expected);
}

#[test]
fn orient3d_range_follows_from_the_operand_domain() {
    let (plan, _) = raw(ORIENT3D);
    assert_eq!(plan.range.min, 2f64.powi(-188));
    assert!(2f64.powi(238) <= plan.range.max && plan.range.max <= 2f64.powi(240));
}

/// The independent check of the range rule: these exponents were derived by
/// hand from the binding product operand of each form (the 2x2 minors of
/// orient3d and the 3x3 minors of in-sphere), not from the code under test.
#[test]
fn every_shape_has_the_derived_range() {
    assert_eq!(exponents(&raw(ORIENT3D)), (-188, 239));
    assert_eq!(exponents(&differences(ORIENT3D, Leaves::One)), (-188, 239));
    assert_eq!(exponents(&differences(ORIENT3D, Leaves::Two)), (-188, 238));
    assert_eq!(exponents(&raw(INSPHERE_RAW)), (-108, 159));
    assert_eq!(exponents(&differences(INSPHERE, Leaves::One)), (-108, 159));
    assert_eq!(exponents(&differences(INSPHERE, Leaves::Two)), (-108, 158));
}

#[test]
fn a_degree_one_predicate_has_the_full_domain_as_range() {
    let (min, max) = domain();
    for (plan, length) in [
        (raw(DIFFERENCE), 2),
        (differences(DIFFERENCE, Leaves::One), 1),
        (differences(DIFFERENCE, Leaves::Two), 2),
    ] {
        assert_eq!((plan.0.range.min, plan.0.range.max), (min, max));
        assert_eq!(root_length(&plan), length);
    }
}

#[test]
fn shared_nodes_are_one_temporary_each() {
    // `x * x` reuses the leaf `x`, which is one node.
    let source = "fn p(a: Point3, b: Point3) -> Sign { (a.x - b.x) * (a.x - b.x) }";
    let (plan, _) = differences(source, Leaves::Two);
    assert_eq!(plan.lengths, [2, 8]);
    assert_eq!(plan.total, 10);
    let (plan, _) = raw(source);
    assert_eq!(plan.lengths, [1, 1, 2, 8]);
    assert_eq!(plan.total, 12);
}

#[test]
fn negation_keeps_the_length() {
    let source = "fn p(a: Point3, b: Point3) -> Sign { -((a.x - b.x) * (a.y - b.y)) }";
    let (plan, root) = differences(source, Leaves::Two);
    assert_eq!(plan.lengths[root], 8);
}

#[test]
fn a_degree_too_high_for_any_range_is_an_error_not_a_panic() {
    // Twenty-one equal factors: an operand of degree 20 needs
    // `(m 2^-52)^20 >= 2^-480` and `M^20 <= 2^480`, so `m >= 2^28` and
    // `M < 2^24`.
    let product = vec!["(a.x - b.x)"; 21].join(" * ");
    let source = format!("fn p(a: Point3, b: Point3) -> Sign {{ {product} }}");
    rejected(try_differences(&source, Leaves::One), "no input range");
    // With two components per leaf the expression is first too large.
    rejected(try_differences(&source, Leaves::Two), "too large");
    let raw_product = vec!["a.x"; 21].join(" * ");
    let source = format!("fn p(a: Point3, b: Point3) -> Sign {{ {raw_product} - b.x }}");
    rejected(try_raw(&source), "no input range");
    rejected(try_raw(&source), "degree 20");
}

#[test]
fn a_constant_too_large_is_an_error() {
    // 2^53 + 1 is not exact in `f64`; 2^60 is.
    let inexact = "fn p(a: Point3, b: Point3) -> Sign { 9007199254740993 * a.x - b.x }";
    rejected(try_raw(inexact), "the constant 9007199254740993");
    let exact = "fn p(a: Point3, b: Point3) -> Sign { 1152921504606846976 * a.x - b.x }";
    assert!(try_raw(exact).is_ok());
}

#[test]
fn a_temporary_too_large_is_an_error() {
    // Forty squarings would need 2^(2^40) components; the sizes are
    // checked node by node, so this returns at the sixth.
    let source = squarings(40, "t0");
    rejected(try_raw(&source), "too large");
}

#[test]
fn an_operation_count_too_large_is_an_error() {
    // `x` has 2^30 components and the last product 2^31, within the size
    // limit, but merging its scalings takes about 2^62 operations.
    let source = "fn p(a: Point3, b: Point3) -> Sign { let t0 = a.x - b.x; \
                  let t1 = t0 * t0; let t2 = t1 * t1; let t3 = t2 * t2; let t4 = t3 * t3; \
                  let w = t1 * t0; let v = t2 * w; let u = t3 * v; let x = t4 * u; x * t0 }";
    rejected(try_differences(source, Leaves::One), "operations");
}

#[test]
fn a_temporary_of_exactly_2_32_components_is_an_error() {
    // `t5` has 2^31 components and the last product exactly 2^32, which
    // does not fit in a `usize` on a 32-bit host.
    rejected(
        try_differences(&squarings(5, "t0"), Leaves::One),
        "2^32 components or more",
    );
}
