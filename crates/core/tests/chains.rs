//! Polynomial expressions evaluated exactly on the scratch arena: sums,
//! differences and products of expansions agree with the oracle in value and
//! sign, including results that are exactly zero or that survive only after
//! deep cancellation.
//!
//! # Why every product stays in the domain of `two_product`
//!
//! Leaf slots hold zero, an `f64` with magnitude in `2^-100 .. 2^100`, or
//! (for the near-equal pairs) the exact sum, built with `grow` and `sum`, of
//! up to 16 such values (`f` is `e` plus `small`, with up to 8 leaves each).
//! Generated expressions nest at most 4 levels, so they have at most 16 leaf
//! occurrences, and have total degree at most 4, so each operand of a
//! product has degree at most 3.
//!
//! - *Lower bound.* Every leaf is a multiple of `2^-152`. Error-free sums of
//!   multiples of `u` are multiples of `u`, and error-free products of
//!   multiples of `u` and `v` are multiples of `u * v`, so every component
//!   of an expansion of degree at most 3 is a nonzero multiple of `2^-456`,
//!   above the `2^-480` the domain requires.
//! - *Upper bound.* Track the mass `M(e)`, the sum of the magnitudes of the
//!   components of `e`. With round-to-nearest and no overflow, `two_sum` and
//!   `fast_two_sum` give `s + err = a + b` with `|err| <= 2^-53 |a + b|`, so
//!   `|s| + |err| <= |a + b| + 2 |err| <= (|a| + |b|) (1 + 2^-52)`; inside its
//!   domain `two_product` likewise gives `|p| + |err| <= |a b| (1 + 2^-52)`.
//!   Every expansion operation is a sequence of these steps, each replacing
//!   two values by two others, and zero elimination only drops zeros. Hence
//!   `M(sum or diff) <= (M(a) + M(b)) c`, `M(scale(e, b)) <= M(e) |b| c` and
//!   `M(product) <= M(a) M(b) c`, where `c = (1 + 2^-52)^K` for the number
//!   `K` of steps of the whole evaluation, counting for each use of a leaf
//!   slot the steps that built it. `K` is far below `2^51`, so `c < 2`. A
//!   leaf slot has mass at most `16 * 2^100 c = 2^104 c`, so every component
//!   of an expansion is at most its mass, which is at most `c B(x)`. Here
//!   `B(x)` is the expression `x` evaluated with every difference turned
//!   into a sum and every leaf replaced by `2^104`. By induction on `x`, with
//!   `n` leaf occurrences and degree `d`, `B(x) <= 2^(n - 1) 2^(104 d)`; for
//!   a product operand, `n <= 16` and `d <= 3`, so `B(x) <= 2^327` and its
//!   components are below `2^328`, under the `2^480` the domain allows.
//!
//! The rewritten expressions of the cancelling and near-equal properties
//! apply commutativity, associativity and distributivity, identities that
//! `B` satisfies too, so a rewrite has the same `B` as what it rewrites.
//! Every product operand in a rewrite is the rewrite of an original product
//! operand or of a summand of one, so its `B` and its degree are at most
//! those of that operand, and the same bounds hold for it, although
//! distributivity can raise its number of leaf occurrences. The arena
//! evaluator still asserts the domain before every product, so that a flaw
//! in this argument would fail loudly.

use indirect_predicates_core::{Scratch, Span, eft, expansion};
use indirect_predicates_oracle::Dyadic;
use indirect_predicates_test_support::{
    assert_valid_expansion, config, leaf, near_equal, sample, value,
};
use proptest::prelude::*;

/// The number of leaf slots an expression can refer to.
const LEAVES: usize = 6;

/// The largest total degree of a generated expression.
const MAX_DEGREE: u32 = 4;

/// The arena size the properties run on.
const ARENA: usize = 65_536;

/// Random expression over f64 leaves: Leaf(i) | Add | Sub | Mul, total degree <= 4.
#[derive(Clone, Debug, PartialEq)]
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

    /// An expression with the same value, computed differently. The
    /// operands are rewritten first; then, if `choices` yields `true`, the
    /// node itself is rewritten by distributivity (`a * (x ± y)` and
    /// `(x ± y) * b`), associativity (`(x + y) ± b` into `x + (y ± b)`) or,
    /// when neither applies, commutativity of a sum or product.
    fn rewritten(&self, choices: &mut impl Iterator<Item = bool>) -> Expr {
        let mut parts = |a: &Expr, b: &Expr| {
            let (a, b) = (a.rewritten(choices), b.rewritten(choices));
            (a, b, choices.next().unwrap_or(false))
        };
        match self {
            Expr::Leaf(i) => Expr::Leaf(*i),
            Expr::Add(a, b) => match parts(a, b) {
                (Expr::Add(x, y), b, true) => add(*x, add(*y, b)),
                (a, b, true) => add(b, a),
                (a, b, false) => add(a, b),
            },
            Expr::Sub(a, b) => match parts(a, b) {
                (Expr::Add(x, y), b, true) => add(*x, sub(*y, b)),
                (a, b, _) => sub(a, b),
            },
            Expr::Mul(a, b) => match parts(a, b) {
                (a, Expr::Add(x, y), true) => add(mul(a.clone(), *x), mul(a, *y)),
                (a, Expr::Sub(x, y), true) => sub(mul(a.clone(), *x), mul(a, *y)),
                (Expr::Add(x, y), b, true) => add(mul(*x, b.clone()), mul(*y, b)),
                (Expr::Sub(x, y), b, true) => sub(mul(*x, b.clone()), mul(*y, b)),
                (a, b, true) => mul(b, a),
                (a, b, false) => mul(a, b),
            },
        }
    }
}

/// The sum `a + b`.
fn add(a: Expr, b: Expr) -> Expr {
    Expr::Add(Box::new(a), Box::new(b))
}

/// The difference `a - b`.
fn sub(a: Expr, b: Expr) -> Expr {
    Expr::Sub(Box::new(a), Box::new(b))
}

/// The product `a * b`.
fn mul(a: Expr, b: Expr) -> Expr {
    Expr::Mul(Box::new(a), Box::new(b))
}

/// Expressions of total degree at most `max_degree`, nested at most `depth`
/// levels deep. A product splits the degree budget between its operands.
fn expr(max_degree: u32, depth: u32) -> BoxedStrategy<Expr> {
    let leaf = (0..LEAVES).prop_map(Expr::Leaf);
    if depth == 0 {
        return leaf.boxed();
    }
    let operand = move || expr(max_degree, depth - 1);
    let add = (operand(), operand()).prop_map(|(a, b)| add(a, b));
    let sub = (operand(), operand()).prop_map(|(a, b)| sub(a, b));
    if max_degree < 2 {
        return prop_oneof![leaf, add, sub].boxed();
    }
    let mul = (1..max_degree)
        .prop_flat_map(move |left| (expr(left, depth - 1), expr(max_degree - left, depth - 1)))
        .prop_map(|(a, b)| mul(a, b));
    prop_oneof![1 => leaf, 1 => add, 1 => sub, 3 => mul].boxed()
}

/// The values of the leaf slots. Each is zero now and then, and otherwise
/// has magnitude in `2^-100 ..= 2^100` and a significand of random length:
/// in half of the cases spread over the whole range, in the other half with
/// exponents within 30 of a common centre, so that sums carry and cancel,
/// often exactly when the significands are short. Half of the cases repeat
/// values across slots, so that expressions such as `(a + b) - (b + c)` with
/// `a == c` cancel exactly.
fn leaves() -> impl Strategy<Value = Vec<f64>> {
    let slots = |leaf: BoxedStrategy<f64>| {
        let values = prop::collection::vec(prop_oneof![1 => Just(0.0), 8 => leaf], LEAVES);
        let picks = prop::collection::vec(0..LEAVES, LEAVES);
        // The first `distinct` slots take distinct values and the others
        // repeat one of those.
        (values, prop_oneof![Just(LEAVES), 1usize..=3], picks).prop_map(
            |(values, distinct, picks)| {
                (0..LEAVES)
                    .map(|j| values[if j < distinct { j } else { picks[j] % distinct }])
                    .collect()
            },
        )
    };
    prop_oneof![
        slots(leaf(-100, 99, 53).boxed()),
        (-69i32..=69)
            .prop_flat_map(move |centre| slots(leaf(centre - 30, centre + 30, 53).boxed())),
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
    assert!(
        e.iter().all(|&x| eft::in_product_domain(x)),
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

/// Choices for [`Expr::rewritten`], enough for every node of a generated
/// expression.
fn choices() -> impl Strategy<Value = Vec<bool>> {
    prop::collection::vec(any::<bool>(), 64)
}

/// `e - e'`, where `e'` is `e` rewritten: exactly zero, reached through two
/// different computations.
fn cancelling_expressions() -> impl Strategy<Value = Expr> {
    (expr(MAX_DEGREE, 4), choices())
        .prop_map(|(e, choices)| sub(e.clone(), e.rewritten(&mut choices.into_iter())))
}

/// `e * x - f * x'` over slots 0 and 1 holding `e` and `f`, where `x'` is
/// `x` rewritten: two nearly equal products whose difference `(e - f) * x`
/// survives only after deep cancellation.
fn difference_of_near_equal_products() -> impl Strategy<Value = Expr> {
    (expr(MAX_DEGREE - 1, 3), choices()).prop_map(|(x, choices)| {
        let rewritten = x.rewritten(&mut choices.into_iter());
        sub(mul(Expr::Leaf(0), x), mul(Expr::Leaf(1), rewritten))
    })
}

proptest! {
    #![proptest_config(config())]

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
    /// The number of those whose leaf slots are all nonzero, so that the
    /// zero comes from cancellation.
    zero_without_zero_leaf: usize,
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
        let zero = eval_scratch(&e, &spans, &mut s).is_empty();
        coverage.zero += usize::from(zero);
        coverage.zero_without_zero_leaf += usize::from(zero && xs.iter().all(|&x| x != 0.0));
        coverage.multi_component_products +=
            usize::from(multi_component_products(&e, &spans, &mut s) > 0);
    }
    println!("{coverage:?}");
    assert!(coverage.by_degree.iter().all(|&n| n >= 100), "{coverage:?}");
    assert!(coverage.zero >= 20, "{coverage:?}");
    assert!(coverage.zero_without_zero_leaf >= 40, "{coverage:?}");
    assert!(coverage.multi_component_products >= 175, "{coverage:?}");
}

/// The number of leaf occurrences in `expr`.
fn leaf_count(expr: &Expr) -> usize {
    match expr {
        Expr::Leaf(_) => 1,
        Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b) => leaf_count(a) + leaf_count(b),
    }
}

#[test]
fn rewrites_usually_change_the_computation() {
    let samples = sample((expr(MAX_DEGREE, 4), choices()), 1000);
    let (mut changed, mut distributed) = (0, 0);
    for (e, choices) in samples {
        let rewritten = e.rewritten(&mut choices.into_iter());
        changed += usize::from(rewritten != e);
        distributed += usize::from(leaf_count(&rewritten) != leaf_count(&e));
    }
    println!("changed: {changed}, distributed: {distributed}");
    assert!(
        changed >= 600,
        "only {changed} of 1000 rewrites change the expression"
    );
    assert!(
        distributed >= 300,
        "only {distributed} of 1000 rewrites distribute a product"
    );
}
