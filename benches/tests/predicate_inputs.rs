//! Checks that the predicate benchmark inputs are what the benchmarks claim,
//! and that the baseline libraries agree with the exact signs on them.

use indirect_predicates_benches::predicates::{
    InSphereInput, InputSet, Orient3dInput, SET_SIZE, geometry_insphere_sign,
    geometry_orient3d_sign, insphere_sets, naive_insphere, naive_orient3d, orient3d_sets,
    robust_insphere_sign, robust_orient3d_sign,
};
use indirect_predicates_corpus::exact;

fn exact_orient3d(p: &Orient3dInput) -> i8 {
    let [a, b, c, d] = p.map(|q| q.map(indirect_predicates_oracle::Dyadic::from_f64));
    exact::orient3d(&a, &b, &c, &d)
}

fn exact_insphere(p: &InSphereInput) -> i8 {
    let [a, b, c, d, e] = p.map(|q| q.map(indirect_predicates_oracle::Dyadic::from_f64));
    exact::insphere(&a, &b, &c, &d, &e)
}

fn set<'a, T>(sets: &'a [InputSet<T>], name: &str) -> &'a InputSet<T> {
    sets.iter()
        .find(|s| s.name.starts_with(name))
        .unwrap_or_else(|| panic!("no set named {name}"))
}

fn signum(x: f64) -> i8 {
    if x > 0.0 {
        1
    } else if x < 0.0 {
        -1
    } else {
        0
    }
}

#[test]
fn every_set_has_its_declared_size_and_is_deterministic() {
    let (first, second) = (orient3d_sets(), orient3d_sets());
    assert_eq!(first.len(), 4);
    for (x, y) in first.iter().zip(&second) {
        assert_eq!(x.name, y.name);
        assert_eq!(x.cases, y.cases, "{}", x.name);
        if !x.name.starts_with("corpus") {
            assert_eq!(x.cases.len(), SET_SIZE, "{}", x.name);
        }
        assert!(!x.cases.is_empty(), "{}", x.name);
    }
    let (first, second) = (insphere_sets(), insphere_sets());
    assert_eq!(first.len(), 4);
    for (x, y) in first.iter().zip(&second) {
        assert_eq!(x.name, y.name);
        assert_eq!(x.cases, y.cases, "{}", x.name);
        if !x.name.starts_with("corpus") {
            assert_eq!(x.cases.len(), SET_SIZE, "{}", x.name);
        }
        assert!(!x.cases.is_empty(), "{}", x.name);
    }
}

#[test]
fn degenerate_sets_are_exactly_zero() {
    for case in &set(&orient3d_sets(), "degenerate").cases {
        assert_eq!(exact_orient3d(case), 0, "{case:?}");
    }
    for case in &set(&insphere_sets(), "degenerate").cases {
        assert_eq!(exact_insphere(case), 0, "{case:?}");
    }
}

#[test]
fn random_sets_are_never_zero() {
    for case in &set(&orient3d_sets(), "random").cases {
        assert_ne!(exact_orient3d(case), 0, "{case:?}");
    }
    for case in &set(&insphere_sets(), "random").cases {
        assert_ne!(exact_insphere(case), 0, "{case:?}");
    }
}

#[test]
fn near_degenerate_sets_are_not_decided_by_the_naive_determinant_alone() {
    let sets = orient3d_sets();
    let cases = &set(&sets, "near_degenerate").cases;
    let wrong = cases
        .iter()
        .filter(|c| signum(naive_orient3d(c)) != exact_orient3d(c) || naive_orient3d(c) == 0.0)
        .count();
    assert!(
        2 * wrong >= cases.len(),
        "orient3d: {wrong} of {}",
        cases.len()
    );

    let sets = insphere_sets();
    let cases = &set(&sets, "near_degenerate").cases;
    let wrong = cases
        .iter()
        .filter(|c| signum(naive_insphere(c)) != exact_insphere(c) || naive_insphere(c) == 0.0)
        .count();
    assert!(
        2 * wrong >= cases.len(),
        "insphere: {wrong} of {}",
        cases.len()
    );
}

#[test]
fn robust_and_geometry_predicates_match_the_exact_sign_on_every_set() {
    for s in orient3d_sets() {
        for case in &s.cases {
            let exact = exact_orient3d(case);
            assert_eq!(
                robust_orient3d_sign(case),
                exact,
                "robust {}: {case:?}",
                s.name
            );
            assert_eq!(
                geometry_orient3d_sign(case),
                exact,
                "geometry-predicates {}: {case:?}",
                s.name
            );
        }
    }
    for s in insphere_sets() {
        for case in &s.cases {
            let exact = exact_insphere(case);
            assert_eq!(
                robust_insphere_sign(case),
                exact,
                "robust {}: {case:?}",
                s.name
            );
            assert_eq!(
                geometry_insphere_sign(case),
                exact,
                "geometry-predicates {}: {case:?}",
                s.name
            );
        }
    }
}

#[test]
fn naive_determinants_have_the_exact_sign_on_random_inputs() {
    // At magnitude below 2^10 with generic coordinates the rounding error of
    // the plain f64 determinant is far below the value, so the signs agree on
    // every case; a mismatch means the naive formula has the wrong convention.
    for case in &set(&orient3d_sets(), "random").cases {
        assert_eq!(
            signum(naive_orient3d(case)),
            exact_orient3d(case),
            "{case:?}"
        );
    }
    for case in &set(&insphere_sets(), "random").cases {
        assert_eq!(
            signum(naive_insphere(case)),
            exact_insphere(case),
            "{case:?}"
        );
    }
}
