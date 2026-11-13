//! Canonical polynomials of expression graphs.
//!
//! The canonical polynomial is the expanded form of an expression over the
//! input coordinates, with integer coefficients and no zero terms. Two
//! expressions are the same polynomial exactly when their canonical forms are
//! equal.

use std::collections::BTreeMap;

use num_bigint::BigInt;

use crate::differences::{DiffGraph, DiffNode};
use crate::error::Error;
use crate::graph::{Graph, Node};
use crate::parse::{Position, Predicate};

/// A variable: coordinate `axis` of parameter `point`.
type Variable = (usize, u8);

/// A product of variables: each variable with its exponent, sorted by
/// variable, exponents at least 1. The constant monomial is empty.
type Monomial = Vec<(Variable, u32)>;

/// An expanded polynomial over the input coordinates, with integer
/// coefficients. Terms with a zero coefficient are never stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Polynomial {
    terms: BTreeMap<Monomial, BigInt>,
}

impl Polynomial {
    fn zero() -> Self {
        Polynomial {
            terms: BTreeMap::new(),
        }
    }

    fn constant(value: i64) -> Self {
        let mut p = Polynomial::zero();
        if value != 0 {
            p.terms.insert(Vec::new(), BigInt::from(value));
        }
        p
    }

    fn variable(point: usize, axis: u8) -> Self {
        let mut p = Polynomial::zero();
        p.terms.insert(vec![((point, axis), 1)], BigInt::from(1));
        p
    }

    /// Adds `other`, or subtracts it when `negate`, and returns the number of
    /// bits of the largest coefficient it changed.
    fn add_scaled(&mut self, other: &Polynomial, negate: bool) -> u64 {
        let mut bits = 0;
        for (monomial, coefficient) in &other.terms {
            let entry = self.terms.entry(monomial.clone()).or_default();
            if negate {
                *entry -= coefficient;
            } else {
                *entry += coefficient;
            }
            bits = bits.max(entry.bits());
            if *entry == BigInt::ZERO {
                self.terms.remove(monomial);
            }
        }
        bits
    }

    /// The product with `other`, or the message to report when an exponent
    /// would exceed `u32::MAX`.
    fn multiply(&self, other: &Polynomial) -> Result<Polynomial, String> {
        let mut result = Polynomial::zero();
        for (m1, c1) in &self.terms {
            for (m2, c2) in &other.terms {
                let entry = result.terms.entry(multiply_monomials(m1, m2)?).or_default();
                *entry += c1 * c2;
            }
        }
        result.terms.retain(|_, c| *c != BigInt::ZERO);
        Ok(result)
    }

    /// Total degree of the highest-degree term, or `None` for the zero
    /// polynomial.
    pub fn degree(&self) -> Option<u32> {
        self.terms.keys().map(monomial_degree).max()
    }

    /// Whether every term has the same total degree. The zero polynomial is
    /// homogeneous.
    pub fn is_homogeneous(&self) -> bool {
        let mut degrees = self.terms.keys().map(monomial_degree);
        match degrees.next() {
            None => true,
            Some(first) => degrees.all(|d| d == first),
        }
    }

    /// Number of terms, that is, of monomials with a nonzero coefficient.
    pub fn terms(&self) -> usize {
        self.terms.len()
    }

    /// Whether this is the zero polynomial.
    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }

    /// Whether the polynomial is affine in each input coordinate: no term
    /// has a coordinate with an exponent above 1. Such a polynomial takes,
    /// at every point of a box of coordinates, a value between the least and
    /// the greatest of its values at the corners of the box.
    pub fn is_affine_in_each_coordinate(&self) -> bool {
        self.terms
            .keys()
            .all(|monomial| monomial.iter().all(|&(_, exponent)| exponent <= 1))
    }
}

/// The total degree of a monomial. Every monomial [`canonical`] builds has
/// a total degree of at most `u32::MAX`, so this sum does not overflow.
fn monomial_degree(monomial: &Monomial) -> u32 {
    monomial.iter().map(|&(_, e)| e).sum()
}

/// Merges two sorted monomials, adding the exponents of shared variables, or
/// returns the message to report when the total degree would exceed
/// `u32::MAX`.
fn multiply_monomials(a: &Monomial, b: &Monomial) -> Result<Monomial, String> {
    let degree = u64::from(monomial_degree(a)) + u64::from(monomial_degree(b));
    if degree > u64::from(u32::MAX) {
        return Err(format!(
            "the expression is too large to expand: a term would have a degree above {}",
            u32::MAX
        ));
    }
    let mut out = Vec::with_capacity(a.len() + b.len());
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        match a[i].0.cmp(&b[j].0) {
            std::cmp::Ordering::Less => {
                out.push(a[i]);
                i += 1;
            }
            std::cmp::Ordering::Greater => {
                out.push(b[j]);
                j += 1;
            }
            std::cmp::Ordering::Equal => {
                // At most the total degree, checked above.
                out.push((a[i].0, a[i].1 + b[j].1));
                i += 1;
                j += 1;
            }
        }
    }
    out.extend_from_slice(&a[i..]);
    out.extend_from_slice(&b[j..]);
    Ok(out)
}

/// The most terms a polynomial may have while a graph is expanded.
pub(crate) const MAX_TERMS: usize = 1 << 16;

/// The most work an expansion may take, in units of one product or sum of
/// two coefficients of at most 64 bits, or one term copied.
pub(crate) const MAX_WORK: u64 = 1 << 20;

/// The most bits a coefficient may have while a graph is expanded.
pub(crate) const MAX_COEFFICIENT_BITS: u64 = 1 << 16;

/// A polynomial being expanded, with an upper bound on the number of bits of
/// its coefficients.
#[derive(Clone)]
struct Value {
    polynomial: Polynomial,
    bits: u64,
}

/// The work done so far, against [`MAX_WORK`].
struct Budget {
    work: u64,
}

impl Budget {
    /// Spends `pairs` operations on coefficients of at most `bits` bits, or
    /// returns the message to report, before the work is done.
    fn spend(&mut self, pairs: u64, bits: u64) -> Result<(), String> {
        if bits > MAX_COEFFICIENT_BITS {
            return Err(format!(
                "the expression is too large to expand: a coefficient would have \
                 more than {MAX_COEFFICIENT_BITS} bits"
            ));
        }
        let cost = pairs.saturating_mul(1 + bits / 64);
        self.work = self.work.saturating_add(cost);
        if self.work > MAX_WORK {
            return Err(format!(
                "the expression is too large to expand: expanding it would take \
                 more than {MAX_WORK} operations on its terms"
            ));
        }
        Ok(())
    }
}

/// The number of bits of the largest coefficient of `p`.
fn coefficient_bits(p: &Polynomial) -> u64 {
    p.terms.values().map(BigInt::bits).max().unwrap_or(0)
}

/// Expands `graph` into its canonical polynomial.
///
/// The graph is walked iteratively in node order, so the depth of the
/// expression does not matter. A node's polynomial is dropped after its last
/// use, which keeps long sums linear in memory.
///
/// # Errors
///
/// Returns an error, at `position` in `file`, when the expansion is too
/// large: a polynomial with more than 2^16 terms, a coefficient of more than
/// 2^16 bits, a term of degree above `u32::MAX`, or more than 2^20 units of
/// work, a unit being one product or sum of two coefficients of up to 64
/// bits or one term copied. Each limit is checked before the work it
/// guards, so an expansion that is too large fails quickly.
pub fn canonical(graph: &Graph, file: &str, position: Position) -> Result<Polynomial, Error> {
    expand(graph).map_err(|message| Error::at(file, position, message))
}

fn expand(graph: &Graph) -> Result<Polynomial, String> {
    let mut uses = vec![0usize; graph.nodes.len()];
    uses[graph.root] += 1;
    for node in &graph.nodes {
        if let Node::Add(x, y) | Node::Sub(x, y) | Node::Mul(x, y) = *node {
            uses[x] += 1;
            uses[y] += 1;
        }
    }
    let mut budget = Budget { work: 0 };
    let mut values: Vec<Option<Value>> = vec![None; graph.nodes.len()];
    let mut take = |values: &mut Vec<Option<Value>>, budget: &mut Budget, id: usize| {
        uses[id] -= 1;
        if uses[id] == 0 {
            Ok(values[id].take().expect("children come before parents"))
        } else {
            let value = values[id].as_ref().expect("children come before parents");
            budget.spend(value.polynomial.terms() as u64, value.bits)?;
            Ok::<Value, String>(value.clone())
        }
    };
    for (id, node) in graph.nodes.iter().enumerate() {
        let value = match *node {
            Node::Coord { point, axis } => Value {
                polynomial: Polynomial::variable(point, axis),
                bits: 1,
            },
            Node::Int(n) => {
                let polynomial = Polynomial::constant(n);
                let bits = coefficient_bits(&polynomial);
                Value { polynomial, bits }
            }
            Node::Add(x, y) | Node::Sub(x, y) => {
                let mut left = take(&mut values, &mut budget, x)?;
                let right = take(&mut values, &mut budget, y)?;
                budget.spend(
                    right.polynomial.terms() as u64,
                    left.bits.max(right.bits) + 1,
                )?;
                let changed = left
                    .polynomial
                    .add_scaled(&right.polynomial, matches!(node, Node::Sub(..)));
                // Coefficients that did not change keep their bound.
                left.bits = left.bits.max(changed);
                left
            }
            Node::Mul(x, y) => {
                let left = take(&mut values, &mut budget, x)?;
                let right = take(&mut values, &mut budget, y)?;
                let (l, r) = (
                    left.polynomial.terms() as u64,
                    right.polynomial.terms() as u64,
                );
                // A coefficient of the product is a sum of at most
                // `min(l, r)` products of coefficients.
                let bits = left.bits + right.bits + u64::from(64 - l.min(r).leading_zeros());
                budget.spend(l.saturating_mul(r), bits)?;
                let polynomial = left.polynomial.multiply(&right.polynomial)?;
                let bits = coefficient_bits(&polynomial);
                Value { polynomial, bits }
            }
        };
        if value.polynomial.terms() > MAX_TERMS {
            return Err(format!(
                "the expression is too large to expand: a subexpression has more \
                 than {MAX_TERMS} terms"
            ));
        }
        values[id] = Some(value);
    }
    Ok(take(&mut values, &mut budget, graph.root)?.polynomial)
}

/// Checks that the main form of `predicate` is a nonzero polynomial and, if
/// the predicate has an exact form, that both forms are the same polynomial.
///
/// # Errors
///
/// Returns an error at the position of the predicate's name when the main
/// form expands to zero, at the position of the exact form's name when it
/// expands to a different polynomial, and at the position of the form when
/// [`canonical`] cannot expand it.
pub fn check_same_polynomial(predicate: &Predicate, file: &str) -> Result<(), Error> {
    checked_polynomial(predicate, file).map(|_| ())
}

/// The canonical polynomial of the main form of `predicate`, after the
/// checks of [`check_same_polynomial`], with its errors.
pub(crate) fn checked_polynomial(predicate: &Predicate, file: &str) -> Result<Polynomial, Error> {
    let main = canonical(&predicate.main, file, predicate.position)?;
    if main.is_zero() {
        return Err(Error::at(
            file,
            predicate.position,
            format!(
                "the polynomial of `{}` is identically zero, so its sign is always zero",
                predicate.name
            ),
        ));
    }
    if let Some(exact_graph) = &predicate.exact {
        let position = predicate.exact_position.unwrap_or(predicate.position);
        let exact = canonical(exact_graph, file, position)?;
        if exact != main {
            return Err(Error::at(
                file,
                position,
                format!(
                    "the exact form of `{}` differs from its main form: \
                     they are not the same polynomial (main form has {} terms, exact form has {})",
                    predicate.name,
                    main.terms(),
                    exact.terms()
                ),
            ));
        }
    }
    Ok(main)
}

/// Expands a [`DiffGraph`] into its canonical polynomial over the input
/// coordinates: a leaf is the minuend coordinate minus the subtrahend
/// coordinate and a negation is `0 - x`.
///
/// For the graph a difference form was made from, the result equals
/// [`canonical`] of that graph.
///
/// # Errors
///
/// Returns the error of [`canonical`], at `position` in `file`, when the
/// expansion is too large.
pub fn canonical_of_differences(
    graph: &DiffGraph,
    file: &str,
    position: Position,
) -> Result<Polynomial, Error> {
    let mut nodes = vec![Node::Int(0)];
    let mut ids = Vec::with_capacity(graph.nodes.len());
    for node in &graph.nodes {
        let id = match *node {
            DiffNode::Leaf {
                axis,
                minuend,
                subtrahend,
            } => {
                nodes.push(Node::Coord {
                    point: minuend,
                    axis,
                });
                nodes.push(Node::Coord {
                    point: subtrahend,
                    axis,
                });
                nodes.push(Node::Sub(nodes.len() - 2, nodes.len() - 1));
                nodes.len() - 1
            }
            DiffNode::Neg(x) => {
                nodes.push(Node::Sub(0, ids[x]));
                nodes.len() - 1
            }
            DiffNode::Add(x, y) => {
                nodes.push(Node::Add(ids[x], ids[y]));
                nodes.len() - 1
            }
            DiffNode::Sub(x, y) => {
                nodes.push(Node::Sub(ids[x], ids[y]));
                nodes.len() - 1
            }
            DiffNode::Mul(x, y) => {
                nodes.push(Node::Mul(ids[x], ids[y]));
                nodes.len() - 1
            }
        };
        ids.push(id);
    }
    let graph = Graph {
        root: ids[graph.root],
        nodes,
    };
    canonical(&graph, file, position)
}
