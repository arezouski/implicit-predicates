//! Tests for canonical polynomials.

use implicit_predicates_gen::{
    Graph, Polynomial, Position, Predicate, canonical, check_same_polynomial, parse,
};

const ORIENT3D: &str = include_str!("fixtures/orient3d.pred");
const INSPHERE: &str = include_str!("fixtures/insphere.pred");

/// The canonical polynomial of `graph`, which must be small enough to expand.
fn expand(graph: &Graph) -> Polynomial {
    canonical(graph, "t.pred", Position { line: 1, column: 1 }).unwrap()
}

fn first(source: &str) -> Predicate {
    parse(source, "t.pred").unwrap().remove(0)
}

#[test]
fn orient3d_has_24_terms_of_degree_3() {
    let p = expand(&first(ORIENT3D).main);
    assert_eq!(p.terms(), 24);
    assert_eq!(p.degree(), Some(3));
    assert!(p.is_homogeneous());
}

#[test]
fn the_two_forms_of_orient3d_are_the_same_polynomial() {
    let p = first(ORIENT3D);
    let exact = p.exact.as_ref().expect("the fixture has an exact form");
    assert_eq!(expand(&p.main), expand(exact));
    assert_eq!(check_same_polynomial(&p, "orient3d.pred"), Ok(()));
}

#[test]
fn a_sign_error_in_the_exact_form_is_rejected() {
    let broken = ORIENT3D.replace("-mbcd + macd", "mbcd + macd");
    assert_ne!(broken, ORIENT3D);
    let p = first(&broken);
    let e = check_same_polynomial(&p, "orient3d.pred").expect_err("the forms differ");
    assert!(e.message.contains("differs from its main form"), "{e}");
    assert_eq!(e.file, "orient3d.pred");
    // The position is that of the exact form's name.
    let line = ORIENT3D.lines().position(|l| l == "#[exact]").unwrap() + 2;
    assert_eq!((e.line, e.column), (line, 4), "{e}");
}

#[test]
fn x_times_y_equals_y_times_x() {
    let a = first("fn p(a: Point3) -> Sign { a.x * a.y }");
    let b = first("fn p(a: Point3) -> Sign { a.y * a.x }");
    assert_eq!(expand(&a.main), expand(&b.main));
    let c = first("fn p(a: Point3) -> Sign { a.x * a.x }");
    assert_ne!(expand(&a.main), expand(&c.main));
}

#[test]
fn constants_are_coefficients() {
    let a = first("fn p(a: Point3) -> Sign { 2 * a.x + a.x }");
    let b = first("fn p(a: Point3) -> Sign { 3 * a.x }");
    assert_eq!(expand(&a.main), expand(&b.main));
    assert_eq!(expand(&a.main).terms(), 1);
}

#[test]
fn a_difference_of_equal_terms_is_the_zero_polynomial() {
    let p = first("fn p(a: Point3, b: Point3) -> Sign { (a.x + b.x) - (b.x + a.x) }");
    let zero = expand(&p.main);
    assert_eq!(zero.terms(), 0);
    assert_eq!(zero.degree(), None);
    let e = check_same_polynomial(&p, "t.pred").expect_err("a zero predicate is rejected");
    assert!(e.message.contains("identically zero"), "{e}");
    assert_eq!((e.line, e.column), (1, 4), "{e}");
}

#[test]
fn insphere_has_degree_5() {
    let p = expand(&first(INSPHERE).main);
    assert_eq!(p.degree(), Some(5));
    assert!(p.is_homogeneous());
    assert!(p.terms() > 0);
}

#[test]
fn mixed_degrees_are_not_homogeneous() {
    let p = expand(&first("fn p(a: Point3) -> Sign { a.x * a.y + a.z }").main);
    assert!(!p.is_homogeneous());
    assert_eq!(p.degree(), Some(2));
}

#[test]
fn a_long_sum_does_not_overflow_the_stack() {
    // Distinct terms (`k * a.x` for each k), so the sum is a chain of 100 000
    // nodes; it must be expanded without recursion.
    let n = 100_000;
    let mut src = String::from("fn p(a: Point3) -> Sign { a.x");
    for k in 2..=n {
        src.push_str(&format!(" + {k} * a.x"));
    }
    src.push_str(" }");
    let p = first(&src);
    let poly = expand(&p.main);
    assert_eq!(poly.terms(), 1);
    assert_eq!(check_same_polynomial(&p, "t.pred"), Ok(()));
}
