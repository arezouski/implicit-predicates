//! Polynomial expressions evaluated exactly on the scratch arena: sums,
//! differences and products of expansions agree with the oracle in value and
//! sign, including results that are exactly zero or that survive only after
//! deep cancellation.
//!
//! # Why every product stays in the domain of `two_product`
//!
//! Leaf slots hold zero, an `f64` with magnitude in `2^-100 ..= 2^100`, or
//! (for the near-equal pairs) an exact sum of up to 8 such values, below
//! `2^103`. Expressions nest at most 4 levels, so they have at most 16 leaf
//! occurrences, and have total degree at most 4, so each operand of a
//! product has degree at most 3.
//!
//! - *Lower bound.* Every leaf is a multiple of `2^-152`. Error-free sums of
//!   multiples of `u` are multiples of `u`, and error-free products of
//!   multiples of `u` and `v` are multiples of `u * v`, so every component
//!   of an expansion of degree at most 3 is a nonzero multiple of `2^-456`,
//!   above the `2^-480` the domain requires.
//! - *Upper bound.* Replacing every difference by a sum and every leaf by
//!   `2^103` bounds the value of an operand of degree at most 3, with at
//!   most 16 leaf occurrences, by fewer than `2^10` monomials of at most
//!   `2^309`, that is, by `2^319`. Components of an expansion can exceed its
//!   value, but only by a few binades for each operation that built it, and
//!   the margin up to `2^480` is 160 binades for the few dozen operations of
//!   an expression.
//!
//! The upper bound is a margin argument rather than a proof, so the arena
//! evaluator asserts the domain before every product: a flaw in the argument
//! fails loudly instead of feeding the product an invalid input.

mod common;

use common::{assert_valid_expansion, near_equal, sample, value};
use indirect_predicates_core::{Scratch, Span, expansion};
use indirect_predicates_oracle::Dyadic;
use proptest::prelude::*;

/// The number of leaf slots an expression can refer to.
const LEAVES: usize = 6;

/// The largest total degree of a generated expression.
const MAX_DEGREE: u32 = 4;

/// The arena size the properties run on.
const ARENA: usize = 65_536;

/// Random expression over f64 leaves: Leaf(i) | Add | Sub | Mul, total degree <= 4.
#[derive(Clone, Debug)]
enum Expr {
    Leaf(usize),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
}

impl Expr {
    /// The total degree of the polynomial the expression computes, counting
    /// every leaf as degree 1.
    fn degree(&self) -> u32 {
        match self {
            Expr::Leaf(_) => 1,
            Expr::Add(a, b) | Expr::Sub(a, b) => a.degree().max(b.degree()),
            Expr::Mul(a, b) => a.degree() + b.degree(),
        }
    }

    /// The same expression with the operands of each sum and product
    /// swapped when `flips` yields `true`: the same value, computed in a
    /// different order.
    fn reordered(&self, flips: &mut impl Iterator<Item = bool>) -> Expr {
        match self {
            Expr::Leaf(i) => Expr::Leaf(*i),
            Expr::Add(a, b) => {
                let (a, b) = reordered_operands(a, b, flips);
                Expr::Add(a, b)
            }
            Expr::Sub(a, b) => {
                Expr::Sub(Box::new(a.reordered(flips)), Box::new(b.reordered(flips)))
            }
            Expr::Mul(a, b) => {
                let (a, b) = reordered_operands(a, b, flips);
                Expr::Mul(a, b)
            }
        }
    }
}

/// The reordered operands of a sum or product, swapped if `flips` says so.
fn reordered_operands(
    a: &Expr,
    b: &Expr,
    flips: &mut impl Iterator<Item = bool>,
) -> (Box<Expr>, Box<Expr>) {
    let (a, b) = (Box::new(a.reordered(flips)), Box::new(b.reordered(flips)));
    if flips.next().unwrap_or(false) {
        (b, a)
    } else {
        (a, b)
    }
}

/// Expressions of total degree at most `max_degree`, nested at most `depth`
/// levels deep. A product splits the degree budget between its operands.
fn expr(max_degree: u32, depth: u32) -> BoxedStrategy<Expr> {
    let leaf = (0..LEAVES).prop_map(Expr::Leaf);
    if depth == 0 {
        return leaf.boxed();
    }
    let operand = move || expr(max_degree, depth - 1);
    let add = (operand(), operand()).prop_map(|(a, b)| Expr::Add(Box::new(a), Box::new(b)));
    let sub = (operand(), operand()).prop_map(|(a, b)| Expr::Sub(Box::new(a), Box::new(b)));
    if max_degree < 2 {
        return prop_oneof![leaf, add, sub].boxed();
    }
    let mul = (1..max_degree)
        .prop_flat_map(move |left| (expr(left, depth - 1), expr(max_degree - left, depth - 1)))
        .prop_map(|(a, b)| Expr::Mul(Box::new(a), Box::new(b)));
    prop_oneof![1 => leaf, 1 => add, 1 => sub, 3 => mul].boxed()
}

/// The values of the leaf slots. Each is zero now and then, and otherwise
/// has magnitude in `2^-100 ..= 2^100` and a significand of random length:
/// in half of the cases spread over the whole range, in the other half with
/// exponents within 30 of a common centre, so that sums carry and cancel,
/// often exactly when the significands are short.
fn leaves() -> impl Strategy<Value = Vec<f64>> {
    let slots = |leaf: BoxedStrategy<f64>| {
        prop::collection::vec(prop_oneof![1 => Just(0.0), 8 => leaf], LEAVES)
    };
    prop_oneof![
        slots(common::leaf(-100, 99, 53).boxed()),
        (-70i32..=70)
            .prop_flat_map(move |centre| slots(common::leaf(centre - 30, centre + 30, 53).boxed())),
    ]
}

/// Evaluates `expr` exactly on the arena, with the leaf slots at `leaves`.
fn eval_scratch(expr: &Expr, leaves: &[Span], s: &mut Scratch<'_>) -> Span {
    match expr {
        Expr::Leaf(i) => leaves[*i],
        Expr::Add(a, b) => {
            let (a, b) = (eval_scratch(a, leaves, s), eval_scratch(b, leaves, s));
            s.sum(a, b).unwrap()
        }
        Expr::Sub(a, b) => {
            let (a, b) = (eval_scratch(a, leaves, s), eval_scratch(b, leaves, s));
            s.diff(a, b).unwrap()
        }
        Expr::Mul(a, b) => {
            let (a, b) = (eval_scratch(a, leaves, s), eval_scratch(b, leaves, s));
            assert_in_product_domain(s.get(a));
            assert_in_product_domain(s.get(b));
            s.product(a, b).unwrap()
        }
    }
}

/// Evaluates `expr` with the oracle, with the leaf slots at `leaves`.
fn eval_oracle(expr: &Expr, leaves: &[Dyadic]) -> Dyadic {
    match expr {
        Expr::Leaf(i) => leaves[*i].clone(),
        Expr::Add(a, b) => &eval_oracle(a, leaves) + &eval_oracle(b, leaves),
        Expr::Sub(a, b) => &eval_oracle(a, leaves) - &eval_oracle(b, leaves),
        Expr::Mul(a, b) => &eval_oracle(a, leaves) * &eval_oracle(b, leaves),
    }
}

/// Panics unless every component of `e` lies in the domain of `two_product`.
fn assert_in_product_domain(e: &[f64]) {
    let domain = 2f64.powi(-480)..=2f64.powi(480);
    assert!(
        e.iter().all(|x| domain.contains(&x.abs())),
        "{e:?} left the domain of two_product"
    );
}

/// Evaluates `expr` on the arena over leaf slots holding the expansions
/// `leaves`, checks the result against the oracle in value and sign, and
/// returns the exact value.
fn check(expr: &Expr, leaves: &[Vec<f64>]) -> Dyadic {
    let mut buf = vec![0.0; ARENA];
    let mut s = Scratch::borrowed(&mut buf);
    let spans: Vec<Span> = leaves.iter().map(|e| s.push(e).unwrap()).collect();
    let result = eval_scratch(expr, &spans, &mut s);
    let exact = eval_oracle(expr, &leaves.iter().map(|e| value(e)).collect::<Vec<_>>());
    assert_valid_expansion(s.get(result));
    assert_eq!(value(s.get(result)), exact, "value of {expr:?}");
    assert_eq!(s.sign(result) as i8, exact.sign(), "sign of {expr:?}");
    exact
}

/// The expansions of the f64 leaf values, as `push` of `from_f64` gives them.
fn from_f64s(xs: &[f64]) -> Vec<Vec<f64>> {
    xs.iter()
        .map(|&x| {
            let mut e = vec![0.0; 1];
            let len = expansion::from_f64(x, &mut e);
            e.truncate(len);
            e
        })
        .collect()
}

/// `e - e'`, where `e'` is `expr` with some operands swapped: exactly zero.
fn cancelling_expressions() -> impl Strategy<Value = Expr> {
    (
        expr(MAX_DEGREE, 4),
        prop::collection::vec(any::<bool>(), 64),
    )
        .prop_map(|(e, flips)| {
            let reordered = e.reordered(&mut flips.into_iter());
            Expr::Sub(Box::new(e), Box::new(reordered))
        })
}

/// `e * x - f * x'` over slots 0 and 1 holding `e` and `f`, where `x'` is
/// `x` reordered: two nearly equal products whose difference `(e - f) * x`
/// survives only after deep cancellation.
fn difference_of_near_equal_products() -> impl Strategy<Value = Expr> {
    (
        expr(MAX_DEGREE - 1, 3),
        prop::collection::vec(any::<bool>(), 64),
    )
        .prop_map(|(x, flips)| {
            let reordered = x.reordered(&mut flips.into_iter());
            Expr::Sub(
                Box::new(Expr::Mul(Box::new(Expr::Leaf(0)), Box::new(x))),
                Box::new(Expr::Mul(Box::new(Expr::Leaf(1)), Box::new(reordered))),
            )
        })
}

proptest! {
    #![proptest_config(common::config())]

    #[test]
    fn random_expressions_match_oracle(e in expr(MAX_DEGREE, 4), xs in leaves()) {
        prop_assert!(e.degree() <= MAX_DEGREE);
        check(&e, &from_f64s(&xs));
    }

    #[test]
    fn cancelling_expressions_are_exactly_zero(e in cancelling_expressions(), xs in leaves()) {
        let leaves = from_f64s(&xs);
        prop_assert!(check(&e, &leaves).is_zero());
    }

    #[test]
    fn near_equal_products_match_oracle(
        e in difference_of_near_equal_products(),
        (near, nearly) in near_equal(8, 100),
        xs in leaves(),
    ) {
        let mut leaves = from_f64s(&xs);
        leaves[0] = near;
        leaves[1] = nearly;
        check(&e, &leaves);
    }
}

/// What a sample of the random expressions exercises.
#[derive(Debug, Default)]
struct Coverage {
    /// The number of expressions of each total degree, from 1 to 4.
    by_degree: [usize; MAX_DEGREE as usize],
    /// The number of expressions whose value is exactly zero.
    zero: usize,
    /// The number of expressions with a product of two expansions of at
    /// least two components each.
    multi_component_products: usize,
}

/// The number of products in `expr` whose operands both evaluate to at least
/// two components.
fn multi_component_products(expr: &Expr, leaves: &[Span], s: &mut Scratch<'_>) -> usize {
    match expr {
        Expr::Leaf(_) => 0,
        Expr::Add(a, b) | Expr::Sub(a, b) => {
            multi_component_products(a, leaves, s) + multi_component_products(b, leaves, s)
        }
        Expr::Mul(a, b) => {
            let (ea, eb) = (eval_scratch(a, leaves, s), eval_scratch(b, leaves, s));
            usize::from(ea.len() >= 2 && eb.len() >= 2)
                + multi_component_products(a, leaves, s)
                + multi_component_products(b, leaves, s)
        }
    }
}

#[test]
fn random_expressions_cover_every_degree_and_multi_component_products() {
    let mut coverage = Coverage::default();
    let mut buf = vec![0.0; ARENA];
    for (e, xs) in sample((expr(MAX_DEGREE, 4), leaves()), 1000) {
        let mut s = Scratch::borrowed(&mut buf);
        let spans: Vec<Span> = from_f64s(&xs).iter().map(|l| s.push(l).unwrap()).collect();
        let degree = usize::try_from(e.degree()).unwrap();
        coverage.by_degree[degree - 1] += 1;
        coverage.zero += usize::from(eval_scratch(&e, &spans, &mut s).is_empty());
        coverage.multi_component_products +=
            usize::from(multi_component_products(&e, &spans, &mut s) > 0);
    }
    println!("{coverage:?}");
    assert!(coverage.by_degree.iter().all(|&n| n >= 100), "{coverage:?}");
    assert!(coverage.zero >= 20, "{coverage:?}");
    assert!(coverage.multi_component_products >= 200, "{coverage:?}");
}
