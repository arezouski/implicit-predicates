//! Canonical polynomials of expression graphs.
//!
//! The canonical polynomial is the expanded form of an expression over the
//! input coordinates, with integer coefficients and no zero terms. Two
//! expressions are the same polynomial exactly when their canonical forms are
//! equal.

use std::collections::BTreeMap;

use num_bigint::BigInt;

use crate::error::Error;
use crate::graph::{Graph, Node};
use crate::parse::Predicate;

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

    /// Adds `sign * other` to `self`.
    fn add_scaled(&mut self, other: &Polynomial, negate: bool) {
        for (monomial, coefficient) in &other.terms {
            let entry = self.terms.entry(monomial.clone()).or_default();
            if negate {
                *entry -= coefficient;
            } else {
                *entry += coefficient;
            }
            if *entry == BigInt::ZERO {
                self.terms.remove(monomial);
            }
        }
    }

    fn multiply(&self, other: &Polynomial) -> Polynomial {
        let mut result = Polynomial::zero();
        for (m1, c1) in &self.terms {
            for (m2, c2) in &other.terms {
                let entry = result.terms.entry(multiply_monomials(m1, m2)).or_default();
                *entry += c1 * c2;
            }
        }
        result.terms.retain(|_, c| *c != BigInt::ZERO);
        result
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
}

fn monomial_degree(monomial: &Monomial) -> u32 {
    monomial.iter().map(|&(_, e)| e).sum()
}

/// Merges two sorted monomials, adding the exponents of shared variables.
fn multiply_monomials(a: &Monomial, b: &Monomial) -> Monomial {
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
                out.push((a[i].0, a[i].1 + b[j].1));
                i += 1;
                j += 1;
            }
        }
    }
    out.extend_from_slice(&a[i..]);
    out.extend_from_slice(&b[j..]);
    out
}

/// Expands `graph` into its canonical polynomial.
///
/// The graph is walked iteratively in node order, so the depth of the
/// expression does not matter. A node's polynomial is dropped after its last
/// use, which keeps long sums linear in memory.
pub fn canonical(graph: &Graph) -> Polynomial {
    let mut uses = vec![0usize; graph.nodes.len()];
    uses[graph.root] += 1;
    for node in &graph.nodes {
        if let Node::Add(x, y) | Node::Sub(x, y) | Node::Mul(x, y) = *node {
            uses[x] += 1;
            uses[y] += 1;
        }
    }
    let mut values: Vec<Option<Polynomial>> = vec![None; graph.nodes.len()];
    let mut take = |values: &mut Vec<Option<Polynomial>>, id: usize| {
        uses[id] -= 1;
        if uses[id] == 0 {
            values[id].take().expect("children come before parents")
        } else {
            values[id].clone().expect("children come before parents")
        }
    };
    for (id, node) in graph.nodes.iter().enumerate() {
        let value = match *node {
            Node::Coord { point, axis } => Polynomial::variable(point, axis),
            Node::Int(n) => Polynomial::constant(n),
            Node::Add(x, y) | Node::Sub(x, y) => {
                let mut left = take(&mut values, x);
                let right = take(&mut values, y);
                left.add_scaled(&right, matches!(node, Node::Sub(..)));
                left
            }
            Node::Mul(x, y) => {
                let left = take(&mut values, x);
                let right = take(&mut values, y);
                left.multiply(&right)
            }
        };
        values[id] = Some(value);
    }
    take(&mut values, graph.root)
}

/// Checks that the main form of `predicate` is a nonzero polynomial and, if
/// the predicate has an exact form, that both forms are the same polynomial.
///
/// # Errors
///
/// Returns an error at the position of the predicate's name when the main
/// form expands to zero, or when the exact form expands to a different
/// polynomial.
pub fn check_same_polynomial(predicate: &Predicate, file: &str) -> Result<(), Error> {
    let main = canonical(&predicate.main);
    let error = |graph: &Graph, message: String| Error {
        file: file.to_string(),
        line: graph.line,
        column: graph.column,
        message,
    };
    if main.is_zero() {
        return Err(error(
            &predicate.main,
            format!(
                "the polynomial of `{}` is identically zero, so its sign is always zero",
                predicate.name
            ),
        ));
    }
    if let Some(exact_graph) = &predicate.exact {
        let exact = canonical(exact_graph);
        if exact != main {
            return Err(error(
                exact_graph,
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
    Ok(())
}
