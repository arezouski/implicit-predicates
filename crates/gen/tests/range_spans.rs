//! Tests for the ranges of the exact stages over arena spans, whose leaves
//! are expansions of up to `MAX_COORD_COMPONENTS` components.

use implicit_predicates_gen::{
    Error, ExactPlan, Leaves, MAX_COORD_COMPONENTS, difference_graph, exact_plan_differences,
    exact_plan_spans_translated, parse,
};

const ORIENT3D: &str = include_str!("fixtures/orient3d.pred");
const INSPHERE: &str = include_str!("fixtures/insphere.pred");
const DIFFERENCE: &str = "fn p(a: Point3, b: Point3) -> Sign { a.x - b.x }";

/// The product domain of the arithmetic core, `[2^-480, 2^480]`; the
/// generator passes the core's own constants.
fn domain() -> (f64, f64) {
    (2f64.powi(-480), 2f64.powi(480))
}

/// The plan over spans of the translated form of the main form of `source`.
fn try_translated(source: &str, components: usize) -> Result<ExactPlan, Error> {
    let p = parse(source, "t.pred").unwrap().remove(0);
    let g = difference_graph(&p.main, "t.pred", p.position).unwrap();
    let (min, max) = domain();
    exact_plan_spans_translated(&g, components, "t.pred", p.position, min, max)
}

fn translated(source: &str) -> ExactPlan {
    try_translated(source, MAX_COORD_COMPONENTS).unwrap()
}

/// The range as the exponents of its two powers of two, which must be
/// normal powers of two.
fn exponents(plan: &ExactPlan) -> (i32, i32) {
    let exponent = |x: f64| {
        assert!(
            x.is_normal() && x > 0.0 && x.to_bits() & ((1 << 52) - 1) == 0,
            "{x} is not a power of two"
        );
        x.log2() as i32
    };
    (exponent(plan.range.min), exponent(plan.range.max))
}

#[test]
fn coordinates_have_at_most_16_components() {
    assert_eq!(MAX_COORD_COMPONENTS, 16);
}

#[test]
fn the_ends_of_every_range_are_powers_of_two() {
    for plan in [
        translated(ORIENT3D),
        translated(INSPHERE),
        translated(DIFFERENCE),
    ] {
        let (min, max) = exponents(&plan);
        assert!(min <= max, "{plan:?}");
    }
}

/// The independent check of the rule: these exponents were derived by hand
/// from the binding product operand of each form, not from the code under
/// test. With leaves of `L = 16` components each at most `M`, a coordinate
/// sums to at most `16 M` and a difference of two to at most `32 M`; the
/// smallest component is bounded below by the grid, as for one component.
///
/// - orient3d, translated: a 2x2 minor is an operand, at most
///   `2 (32 M)^2 = 2^11 M^2 <= 2^480`, so `M <= 2^234`; its grid
///   `2 (m - 52) >= -480` gives `m >= -188`.
/// - in-sphere, translated: a 3x3 minor, `3 * 32 M * 2^11 M^2 = 3 * 2^16
///   M^3 <= 2^480`, so `M <= 2^154`; its grid `3 (m - 52) >= -480` gives
///   `m >= -108`.
#[test]
fn every_form_has_the_derived_range() {
    assert_eq!(exponents(&translated(ORIENT3D)), (-188, 234));
    assert_eq!(exponents(&translated(INSPHERE)), (-108, 154));
}

#[test]
fn with_one_component_the_range_is_that_of_the_stack_form() {
    // One component per coordinate is the stack evaluation, up to the
    // operation counts, which only enter a factor close to one.
    let (min, max) = domain();
    for source in [ORIENT3D, INSPHERE] {
        let p = parse(source, "t.pred").unwrap().remove(0);
        let g = difference_graph(&p.main, "t.pred", p.position).unwrap();
        let two = exact_plan_differences(&g, Leaves::Two, "t.pred", p.position, min, max).unwrap();
        assert_eq!(try_translated(source, 1).unwrap().range, two.range);
    }
}

#[test]
fn the_range_over_spans_lies_within_the_range_over_one_component() {
    let (min, max) = domain();
    for source in [ORIENT3D, INSPHERE] {
        let p = parse(source, "t.pred").unwrap().remove(0);
        let g = difference_graph(&p.main, "t.pred", p.position).unwrap();
        let two = exact_plan_differences(&g, Leaves::Two, "t.pred", p.position, min, max).unwrap();
        let spans = translated(source);
        assert!(two.range.min <= spans.range.min && spans.range.max <= two.range.max);
    }
}

#[test]
fn more_components_never_widen_the_range() {
    for source in [ORIENT3D, INSPHERE] {
        let mut last = try_translated(source, 1).unwrap().range;
        for components in [2, 4, 5, 9, 16, 17, 64] {
            let range = try_translated(source, components).unwrap().range;
            assert!(
                last.min <= range.min && range.max <= last.max,
                "{components}"
            );
            last = range;
        }
    }
}

#[test]
fn a_degree_one_predicate_has_the_full_domain_as_range() {
    let (min, max) = domain();
    let plan = translated(DIFFERENCE);
    assert_eq!((plan.range.min, plan.range.max), (min, max));
}

/// Asserts that a definition on one line is rejected at the name of the
/// predicate with a message containing `fragment`.
fn rejected(result: Result<ExactPlan, Error>, fragment: &str) {
    let e = result.expect_err("the definition must be rejected");
    assert!(e.message.contains(fragment), "{e}");
    assert_eq!(e.file, "t.pred");
    assert_eq!((e.line, e.column), (1, 4), "{e}");
}

#[test]
fn a_degree_without_a_range_over_spans_is_an_error() {
    // An operand of degree 17, `t4 * x` with `t4 = x^16`: on the stack, with
    // exact differences, its grid needs `17 (m - 52) >= -480`, `m >= 2^24`,
    // and its magnitude `M^17 <= 2^480`, `M <= 2^28`. Over spans the leaf is
    // `32 M`, so `M <= 2^23 < 2^24`: no range.
    let source = "fn p(a: Point3, b: Point3) -> Sign { let x = a.x - b.x; \
                  let t1 = x * x; let t2 = t1 * t1; let t3 = t2 * t2; let t4 = t3 * t3; \
                  let t5 = t4 * x; t5 * x }";
    let p = parse(source, "t.pred").unwrap().remove(0);
    let g = difference_graph(&p.main, "t.pred", p.position).unwrap();
    let (min, max) = domain();
    let stack = exact_plan_differences(&g, Leaves::One, "t.pred", p.position, min, max).unwrap();
    assert_eq!(exponents(&stack), (24, 28));
    rejected(
        try_translated(source, MAX_COORD_COMPONENTS),
        "no input range",
    );
    rejected(try_translated(source, MAX_COORD_COMPONENTS), "degree 17");
    // Twenty-one factors have no range for any number of components.
    let product = vec!["(a.x - b.x)"; 21].join(" * ");
    let source = format!("fn p(a: Point3, b: Point3) -> Sign {{ {product} }}");
    rejected(try_translated(&source, 1), "no input range");
    rejected(
        try_translated(&source, MAX_COORD_COMPONENTS),
        "no input range",
    );
}

#[test]
fn a_definition_too_large_is_an_error_not_a_hang() {
    // Thirty-one squarings, a degree of `2^31 + 1`: the structural lengths
    // would be beyond any limit, but the range search fails, quickly.
    let mut body = String::from("let t0 = a.x - b.x; ");
    for i in 1..=31 {
        body += &format!("let t{i} = t{} * t{}; ", i - 1, i - 1);
    }
    let source = format!("fn p(a: Point3, b: Point3) -> Sign {{ {body}t31 * t0 }}");
    rejected(
        try_translated(&source, MAX_COORD_COMPONENTS),
        "no input range",
    );
}

#[test]
fn the_lengths_are_those_of_the_derived_range() {
    // A leaf of the translated form is the difference of two coordinates of
    // up to 16 components: 32. The range is `[2^-428, 2^474]`: the leaf,
    // `32 M` times the rounding factor of the merge that computes it, must
    // stay within the domain, and `32 * 2^475` is its top. The square would
    // have `2 * 32 * 32 = 2048` components, but a valid expansion of values
    // below `2 * 2^958` on the grid `2^-960` has at most `958 + 960 + 1`.
    let plan = translated("fn p(a: Point3, b: Point3) -> Sign { (a.x - b.x) * (a.x - b.x) }");
    assert_eq!(exponents(&plan), (-428, 474));
    assert_eq!(plan.lengths, [32, 1919]);
    assert_eq!(plan.total, 1951);
}
