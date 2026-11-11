//! Tests for the filter bound.

use indirect_predicates_gen::{Error, FilterBound, difference_graph, filter_bound, parse};

const ORIENT3D: &str = include_str!("fixtures/orient3d.pred");
const INSPHERE: &str = include_str!("fixtures/insphere.pred");

/// `u = 2^-53`, the unit roundoff of `f64`.
const U: f64 = 1.0 / 9_007_199_254_740_992.0;

fn try_bound(source: &str) -> Result<FilterBound, Error> {
    let p = parse(source, "t.pred").unwrap().remove(0);
    let g = difference_graph(&p.main, "t.pred", p.position).unwrap();
    filter_bound(&g, "t.pred", p.position)
}

fn bound(source: &str) -> FilterBound {
    try_bound(source).unwrap()
}

/// Asserts that a definition on one line is rejected at the name of the
/// predicate with a message containing every fragment.
fn rejected(source: &str, fragments: &[&str]) {
    let e = try_bound(source).expect_err("the definition must be rejected");
    for fragment in fragments {
        assert!(e.message.contains(fragment), "{e}");
    }
    assert_eq!(e.file, "t.pred");
    assert_eq!((e.line, e.column), (1, 4), "{e}");
}

/// A predicate on one line: the product of `count` equal differences.
fn power(count: usize) -> String {
    let product = vec!["(a.x - b.x)"; count].join(" * ");
    format!("fn p(a: Point3, b: Point3) -> Sign {{ {product} }}")
}

fn is_power_of_two(x: f64) -> bool {
    x > 0.0 && x.is_normal() && x.to_bits() & ((1 << 52) - 1) == 0
}

#[test]
fn orient3d_has_the_expected_constants() {
    let b = bound(ORIENT3D);
    assert_eq!(b.degree, 3);
    assert_eq!(b.products, 9);
    assert!(b.epsilon_is_u_times(46), "eps / u = {}", b.epsilon_in_u);
    assert!(b.kappa_is(18), "kappa = {}", b.kappa_exact);
    assert!(!b.epsilon_is_u_times(45) && !b.epsilon_is_u_times(47));
    assert!(!b.kappa_is(17) && !b.kappa_is(19));
}

#[test]
fn insphere_has_the_expected_constants() {
    let b = bound(INSPHERE);
    assert_eq!(b.degree, 5);
    assert!(b.epsilon_is_u_times(1122), "eps / u = {}", b.epsilon_in_u);
    assert!(b.kappa_is(360), "kappa = {}", b.kappa_exact);
    assert!(!b.epsilon_is_u_times(1121) && !b.epsilon_is_u_times(1123));
    assert!(!b.kappa_is(359) && !b.kappa_is(361));
}

#[test]
fn emitted_constants_round_up() {
    for (source, eps_in_u, kappa) in [(ORIENT3D, 46.0, 18.0), (INSPHERE, 1122.0, 360.0)] {
        let c = bound(source).constants();
        // Both lower limits are exact in f64: an 11-bit integer times
        // 1 + 2^-30.
        let inflate = 1.0 + 2f64.powi(-30);
        let eps_low = eps_in_u * U * inflate;
        let kappa_low = kappa * inflate;
        // The exact constants are below n * (1 + 2^-40) and are inflated by
        // 1 + 2^-30, so the emitted ones are below the next f64 above
        // n * (1 + 2^-30) * (1 + 2^-39).
        let slack = 1.0 + 2f64.powi(-39);
        assert!(c.epsilon >= eps_low, "{} < {eps_low}", c.epsilon);
        assert!(c.epsilon <= (eps_low * slack).next_up(), "{}", c.epsilon);
        assert!(c.kappa >= kappa_low, "{} < {kappa_low}", c.kappa);
        assert!(c.kappa <= (kappa_low * slack).next_up(), "{}", c.kappa);
    }
}

#[test]
fn a_single_difference_has_alpha_u_and_beta_1() {
    let b = bound("fn p(a: Point3, b: Point3) -> Sign { a.x - b.x }");
    assert_eq!(b.degree, 1);
    assert_eq!(b.products, 0);
    assert_eq!(b.epsilon_in_u, "1");
    assert_eq!(b.kappa_exact, "1");
    assert!(b.epsilon_is_u_times(1));
    assert!(b.kappa_is(1));
}

#[test]
fn negation_does_not_change_the_bound() {
    let plain =
        bound("fn p(a: Point3, b: Point3, d: Point3) -> Sign { (a.x - d.x) * (b.y - d.y) }");
    let negated =
        bound("fn p(a: Point3, b: Point3, d: Point3) -> Sign { -(-(a.x - d.x) * (b.y - d.y)) }");
    assert_eq!(plain.epsilon_in_u, negated.epsilon_in_u);
    assert_eq!(plain.kappa_exact, negated.kappa_exact);
    assert_eq!(plain.products, negated.products);
    assert_eq!(plain.constants(), negated.constants());
}

#[test]
fn the_bound_does_not_depend_on_shared_subexpressions() {
    let inline = "fn orient3d(a: Point3, b: Point3, c: Point3, d: Point3) -> Sign {
        (a.x - d.x)*((b.y - d.y)*(c.z - d.z) - (b.z - d.z)*(c.y - d.y))
      - (a.y - d.y)*((b.x - d.x)*(c.z - d.z) - (b.z - d.z)*(c.x - d.x))
      + (a.z - d.z)*((b.x - d.x)*(c.y - d.y) - (b.y - d.y)*(c.x - d.x))
    }";
    let named = bound(ORIENT3D);
    let inline = bound(inline);
    assert_eq!(named.epsilon_in_u, inline.epsilon_in_u);
    assert_eq!(named.kappa_exact, inline.kappa_exact);
    assert_eq!(named.products, inline.products);
    assert_eq!(named.constants(), inline.constants());

    // A subexpression written twice is still one computation.
    let twice = bound(
        "fn p(a: Point3, b: Point3, d: Point3) -> Sign {
            (a.x - d.x)*(b.y - d.y) + (a.x - d.x)*(b.y - d.y)
        }",
    );
    let shared = bound(
        "fn p(a: Point3, b: Point3, d: Point3) -> Sign {
            let s = (a.x - d.x)*(b.y - d.y);
            s + s
        }",
    );
    assert_eq!(twice.products, 1);
    assert_eq!(twice.epsilon_in_u, shared.epsilon_in_u);
    assert_eq!(twice.constants(), shared.constants());
}

#[test]
fn d_min_and_d_max_are_powers_of_two_and_ordered() {
    for source in [
        ORIENT3D,
        INSPHERE,
        "fn p(a: Point3, b: Point3) -> Sign { a.x - b.x }",
    ] {
        let c = bound(source).constants();
        assert!(0.0 < c.d_min && c.d_min < 1.0 && 1.0 < c.d_max, "{c:?}");
        assert!(is_power_of_two(c.d_min), "{}", c.d_min);
        assert!(is_power_of_two(c.d_max), "{}", c.d_max);
    }
    let c = bound(ORIENT3D).constants();
    assert_eq!(c.degree, 3);
}

#[test]
fn a_degree_beyond_any_filter_scale_is_an_error_not_a_panic() {
    // Eleven squarings and one more factor: degree 2049.
    let mut body = String::from("let t0 = a.x - b.x; ");
    for i in 1..=11 {
        body += &format!("let t{i} = t{} * t{}; ", i - 1, i - 1);
    }
    let source = format!("fn p(a: Point3, b: Point3) -> Sign {{ {body}t11 * t0 }}");
    rejected(&source, &["degree 2049", "below 2^1022"]);
}

#[test]
fn a_degree_that_overflows_above_one_is_an_error() {
    // `c > 1` at degree 1022: `c D^1022 <= 2^1022` fails for `D = 2`.
    rejected(&power(1022), &["degree 1022", "below 2^1022"]);
}

#[test]
fn a_degree_whose_bound_underflows_below_one_is_an_error() {
    // `epsilon` is about `2^-42` at degree 1000: `epsilon 2^-1000` is below
    // `2^-1021`.
    rejected(&power(1000), &["degree 1000", "underflow"]);
    assert!(try_bound(&power(900)).is_ok());
}
