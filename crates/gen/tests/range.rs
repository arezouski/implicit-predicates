//! Tests for the ranges and buffer sizes of the exact stages.

use indirect_predicates_gen::{
    ExactPlan, Leaves, canonical, difference_graph, exact_plan_differences, exact_plan_raw, parse,
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

/// The plan of the raw form: the `#[exact]` form if the file has one, the
/// main form otherwise.
fn raw(source: &str) -> (ExactPlan, usize) {
    let p = parse(source, "t.pred").unwrap().remove(0);
    let g = p.exact.unwrap_or(p.main);
    let (min, max) = domain();
    (exact_plan_raw(&g, min, max), g.root)
}

/// The plan of the difference form of the main form.
fn differences(source: &str, leaves: Leaves) -> (ExactPlan, usize) {
    let p = parse(source, "t.pred").unwrap().remove(0);
    let g = difference_graph(&p.main, "t.pred", p.position).unwrap();
    let (min, max) = domain();
    (exact_plan_differences(&g, leaves, min, max), g.root)
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
    let polynomial = canonical(&raw.main);
    assert!(!polynomial.is_zero());
    assert_eq!(polynomial, canonical(&differences.main));
}

#[test]
fn orient3d_range_follows_from_the_operand_domain() {
    let (plan, _) = raw(ORIENT3D);
    assert_eq!(plan.range.min, 2f64.powi(-188));
    assert!(2f64.powi(238) <= plan.range.max && plan.range.max <= 2f64.powi(240));
}

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
#[should_panic(expected = "no range")]
fn a_predicate_of_too_high_degree_has_no_range() {
    // Twenty-one equal factors: an operand of degree 20 needs
    // `(m 2^-52)^20 >= 2^-480` and `M^20 <= 2^480`, so `m >= 2^28` and
    // `M < 2^24`.
    let product = vec!["(a.x - b.x)"; 21].join(" * ");
    let source = format!("fn p(a: Point3, b: Point3) -> Sign {{ {product} }}");
    differences(&source, Leaves::One);
}
