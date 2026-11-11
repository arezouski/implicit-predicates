//! Tests for the per-axis form of the filter bound.

use implicit_predicates_gen::{
    AxisBound, DiffGraph, Error, FilterBound, FilterForm, difference_graph, filter_bound,
    filter_form, parse,
};

/// `u = 2^-53`, the unit roundoff of `f64`.
const U: f64 = 1.0 / 9_007_199_254_740_992.0;

const ORIENT3D: &str = include_str!("fixtures/orient3d.pred");
const INSPHERE: &str = include_str!("fixtures/insphere.pred");
const INSPHERE_RAW: &str = include_str!("fixtures/insphere_raw.pred");

fn graph(source: &str) -> DiffGraph {
    let p = parse(source, "t.pred").unwrap().remove(0);
    difference_graph(&p.main, "t.pred", p.position).unwrap()
}

#[test]
fn orient3d_has_degree_one_in_each_axis() {
    assert_eq!(graph(ORIENT3D).axis_degrees(), Some([1, 1, 1]));
}

#[test]
fn insphere_is_not_multihomogeneous() {
    assert_eq!(graph(INSPHERE).axis_degrees(), None);
    // The form on raw coordinates has no difference form at all, so it never
    // reaches the per-axis analysis.
    let p = parse(INSPHERE_RAW, "t.pred").unwrap().remove(0);
    assert!(difference_graph(&p.main, "t.pred", p.position).is_err());
}

#[test]
fn an_axis_may_have_degree_zero() {
    let g = graph(
        "fn p(a: Point3, b: Point3, c: Point3) -> Sign {
            (a.x - b.x) * (a.x - c.x) * (b.y - c.y) - (a.x - c.x) * (b.x - c.x) * (a.y - b.y)
        }",
    );
    assert_eq!(g.axis_degrees(), Some([2, 1, 0]));
}

#[test]
fn a_sum_of_equal_total_degree_but_different_axes_is_not_multihomogeneous() {
    let g = graph("fn p(a: Point3, b: Point3) -> Sign { (a.x - b.x) + (a.y - b.y) }");
    assert_eq!(g.axis_degrees(), None);
    // A negation keeps the degrees of its operand.
    let g = graph("fn p(a: Point3, b: Point3) -> Sign { -(a.z - b.z) * (a.z - b.z) }");
    assert_eq!(g.axis_degrees(), Some([0, 0, 2]));
}

fn form(source: &str) -> Result<FilterForm, Error> {
    let p = parse(source, "t.pred").unwrap().remove(0);
    let g = difference_graph(&p.main, "t.pred", p.position).unwrap();
    filter_form(&g, "t.pred", p.position)
}

fn per_axis(source: &str) -> AxisBound {
    match form(source).unwrap() {
        FilterForm::PerAxis(b) => b,
        FilterForm::Global(_) => panic!("expected the per-axis form"),
    }
}

/// The exact `epsilon / u` of orient3d, as the rules give it:
/// `75693209636775490727267909043039752761566733276873528450916286465 * 2^-210`.
const ORIENT3D_EPSILON_IN_U: &str = "75693209636775490727267909043039752761566733276873528450916286465/\
    1645504557321206042154969182557350504982735865633579863348609024";

/// The exact `kappa_k` of orient3d on each axis: `6 + 27021597764222977 * 2^-105`.
const ORIENT3D_KAPPA: &str = "243388915243820072108964779655169/40564819207303340847894502572032";

/// The exact global `kappa` of orient3d, three times [`ORIENT3D_KAPPA`].
const ORIENT3D_GLOBAL_KAPPA: &str =
    "730166745731460216326894338965507/40564819207303340847894502572032";

/// `numerator / denominator` of a fraction whose parts fit in `u128`.
fn fraction(text: &str) -> (u128, u128) {
    let (n, d) = text.split_once('/').unwrap_or((text, "1"));
    (n.parse().unwrap(), d.parse().unwrap())
}

#[test]
fn orient3d_takes_the_per_axis_form_with_the_expected_constants() {
    let b = per_axis(ORIENT3D);
    assert_eq!(b.degrees, [1, 1, 1]);
    assert!(b.epsilon_is_u_times(46), "eps / u = {}", b.epsilon_in_u);
    assert!(!b.epsilon_is_u_times(45) && !b.epsilon_is_u_times(47));
    for axis in 0..3 {
        assert!(b.kappa_is(axis, 6), "kappa = {}", b.kappa_exact[axis]);
        assert!(!b.kappa_is(axis, 5) && !b.kappa_is(axis, 7));
    }
    assert!(!b.kappa_is(3, 6), "there are three axes");
    assert!(b.certain_zero());
}

#[test]
fn orient3d_reproduces_the_exact_fractions() {
    let b = per_axis(ORIENT3D);
    assert_eq!(b.epsilon_in_u, ORIENT3D_EPSILON_IN_U);
    assert_eq!(b.kappa_exact, [ORIENT3D_KAPPA; 3].map(String::from));
}

#[test]
fn the_per_axis_kappas_add_up_to_the_global_kappa() {
    let b = per_axis(ORIENT3D);
    let global = bound(ORIENT3D);
    assert_eq!(global.kappa_exact, ORIENT3D_GLOBAL_KAPPA);
    // The epsilon of both forms is the same.
    assert_eq!(b.epsilon_in_u, global.epsilon_in_u);
    let (n, d) = fraction(&global.kappa_exact);
    let parts = b.kappa_exact.clone().map(|k| fraction(&k));
    assert!(parts.iter().all(|&(_, dk)| dk == d));
    assert_eq!(parts.iter().map(|&(nk, _)| nk).sum::<u128>(), n);
}

fn bound(source: &str) -> FilterBound {
    let p = parse(source, "t.pred").unwrap().remove(0);
    let g = difference_graph(&p.main, "t.pred", p.position).unwrap();
    filter_bound(&g, "t.pred", p.position).unwrap()
}

#[test]
fn insphere_keeps_the_global_form() {
    let FilterForm::Global(b) = form(INSPHERE).unwrap() else {
        panic!("insphere is not multihomogeneous");
    };
    assert_eq!(b.degree, 5);
    assert!(b.epsilon_is_u_times(1122), "eps / u = {}", b.epsilon_in_u);
    assert!(b.kappa_is(360), "kappa = {}", b.kappa_exact);
    assert_eq!(b.constants(), bound(INSPHERE).constants());
}

#[test]
fn emitted_constants_round_up() {
    let c = per_axis(ORIENT3D).constants();
    assert_eq!(c.degrees, [1, 1, 1]);
    // As for the global form: the lower limits are exact in f64, and the
    // exact constants are below n (1 + 2^-40).
    let inflate = 1.0 + 2f64.powi(-30);
    let slack = 1.0 + 2f64.powi(-39);
    let eps_low = 46.0 * U * inflate;
    assert!(c.epsilon >= eps_low && c.epsilon <= (eps_low * slack).next_up());
    assert_eq!(c.epsilon, bound(ORIENT3D).constants().epsilon);
    for kappa in c.kappa {
        let low = 6.0 * inflate;
        assert!(kappa >= low && kappa <= (low * slack).next_up(), "{kappa}");
    }
}

#[test]
fn an_axis_of_degree_zero_has_no_kappa_and_no_certain_zero() {
    let b = per_axis(
        "fn p(a: Point3, b: Point3, c: Point3) -> Sign {
            (a.x - b.x) * (a.x - c.x) * (b.y - c.y) - (a.x - c.x) * (b.x - c.x) * (a.y - b.y)
        }",
    );
    assert_eq!(b.degrees, [2, 1, 0]);
    assert_eq!(b.kappa_exact[2], "0");
    assert_eq!(b.constants().kappa[2], 0.0);
    assert!(!b.certain_zero());
    // A single difference: every coefficient is that of a leaf.
    let b = per_axis("fn p(a: Point3, b: Point3) -> Sign { a.y - b.y }");
    assert_eq!(b.degrees, [0, 1, 0]);
    assert_eq!(b.epsilon_in_u, "1");
    assert_eq!(b.kappa_exact, ["0", "1", "0"].map(String::from));
}

fn is_power_of_two(x: f64) -> bool {
    x > 0.0 && x.is_normal() && x.to_bits() & ((1 << 52) - 1) == 0
}

#[test]
fn the_range_is_two_powers_of_two_pinned_for_orient3d() {
    let c = per_axis(ORIENT3D).constants();
    assert!(
        is_power_of_two(c.d_min) && is_power_of_two(c.d_max),
        "{c:?}"
    );
    assert_eq!(c.d_min, 2f64.powi(-324));
    assert_eq!(c.d_max, 2f64.powi(339));
    // The same range as the global form.
    let g = bound(ORIENT3D).constants();
    assert_eq!((c.d_min, c.d_max), (g.d_min, g.d_max));
}

#[test]
fn the_range_is_ordered_for_other_degrees() {
    for source in [
        "fn p(a: Point3, b: Point3) -> Sign { a.y - b.y }",
        "fn p(a: Point3, b: Point3, c: Point3) -> Sign {
            (a.x - b.x) * (a.x - c.x) * (b.y - c.y) - (a.x - c.x) * (b.x - c.x) * (a.y - b.y)
        }",
    ] {
        let c = per_axis(source).constants();
        assert!(0.0 < c.d_min && c.d_min < 1.0 && 1.0 < c.d_max, "{c:?}");
        assert!(
            is_power_of_two(c.d_min) && is_power_of_two(c.d_max),
            "{c:?}"
        );
    }
}

#[test]
fn a_condition_on_the_product_alone_is_not_the_range() {
    // `D_x = 2^100` and `D_y = D_z = 2^-520`: the product `2^-940` is far
    // from the limits of a condition on the product, but `D_y` and `D_z`
    // are below `d_min`.
    let c = per_axis(ORIENT3D).constants();
    let d = [2f64.powi(100), 2f64.powi(-520), 2f64.powi(-520)];
    assert!(d[0] <= c.d_max && d[0] >= c.d_min);
    assert!(d[1] < c.d_min && d[2] < c.d_min);
}

/// Asserts that a definition on one line is rejected at the name of the
/// predicate with a message containing every fragment.
fn rejected(source: &str, fragments: &[&str]) {
    let e = form(source).expect_err("the definition must be rejected");
    for fragment in fragments {
        assert!(e.message.contains(fragment), "{e}");
    }
    assert_eq!(e.file, "t.pred");
    assert_eq!((e.line, e.column), (1, 4), "{e}");
}

/// A predicate on one line: the product of `count` equal differences, of
/// degree `count` in x and 0 in y and z.
fn power(count: usize) -> String {
    let product = vec!["(a.x - b.x)"; count].join(" * ");
    format!("fn p(a: Point3, b: Point3) -> Sign {{ {product} }}")
}

#[test]
fn a_degree_without_a_range_is_an_error_not_a_panic() {
    assert_eq!(graph(&power(1000)).axis_degrees(), Some([1000, 0, 0]));
    rejected(&power(1022), &["degree 1022", "below 2^1022"]);
    rejected(&power(1000), &["degree 1000", "underflow"]);
    assert!(matches!(form(&power(900)), Ok(FilterForm::PerAxis(_))));
    // Eleven squarings and one more factor: degree 2049.
    let mut body = String::from("let t0 = a.x - b.x; ");
    for i in 1..=11 {
        body += &format!("let t{i} = t{} * t{}; ", i - 1, i - 1);
    }
    let source = format!("fn p(a: Point3, b: Point3) -> Sign {{ {body}t11 * t0 }}");
    rejected(&source, &["degree 2049", "below 2^1022"]);
}

#[test]
fn a_long_chain_is_rejected_by_the_budget() {
    // As for the global form: a sum of 4,000 terms written as one chain.
    let terms = vec!["(a.x - b.x) * (a.y - b.y)"; 4000].join(" + ");
    let source = format!("fn p(a: Point3, b: Point3) -> Sign {{ {terms} }}");
    assert_eq!(graph(&source).axis_degrees(), Some([1, 1, 0]));
    rejected(&source, &["too large", "balanced"]);
}
