//! The structure of an exact evaluation: per node, the worst-case length,
//! the number of error-free transformations, the grid and the magnitude
//! bounds.

use crate::differences::{DiffGraph, DiffNode};
use crate::dyadic::Dyadic;
use crate::graph::{Graph, Node, NodeId};

use super::Leaves;

/// One node of an exact evaluation, as far as sizes and bounds are
/// concerned: a difference is an exact sum of a negated operand.
#[derive(Clone, Copy, Debug)]
enum Op {
    /// An input: a coordinate, or a difference of two coordinates.
    Leaf,
    /// An integer constant of the raw form.
    Constant(i64),
    /// A negation.
    Neg(NodeId),
    /// A sum or a difference.
    Sum(NodeId, NodeId),
    /// A product.
    Product(NodeId, NodeId),
}

/// The upper limit on the number of error-free transformations behind one
/// node, `2^51`; it keeps `3 u T <= 3/4` in the component bound.
const MAX_OPERATIONS: u128 = 1 << 51;

/// An exact evaluation of a graph with leaves of one kind.
#[derive(Debug)]
pub(crate) struct Shape {
    ops: Vec<Op>,
    /// The exponent `s` such that every component of a leaf is at most
    /// `2^s M` times the component factor: 0 for a coordinate, 1 for a
    /// difference of two coordinates.
    leaf_scale: i64,
    /// The worst-case length of every node.
    pub(crate) lengths: Vec<usize>,
    /// `T`: the number of error-free transformations of every node and of
    /// everything it is computed from, with multiplicity, saturated at
    /// `u128::MAX`.
    operations: Vec<u128>,
}

impl Shape {
    /// The raw form: every coordinate is a leaf of one component.
    pub(crate) fn raw(graph: &Graph) -> Self {
        let ops = graph
            .nodes
            .iter()
            .map(|node| match *node {
                Node::Coord { .. } => Op::Leaf,
                Node::Int(n) => Op::Constant(n),
                Node::Add(x, y) | Node::Sub(x, y) => Op::Sum(x, y),
                Node::Mul(x, y) => Op::Product(x, y),
            })
            .collect();
        Shape::new(ops, Leaves::One)
    }

    /// A difference form, with leaves of one or two components.
    pub(crate) fn differences(graph: &DiffGraph, leaves: Leaves) -> Self {
        let ops = graph
            .nodes
            .iter()
            .map(|node| match *node {
                DiffNode::Leaf { .. } => Op::Leaf,
                DiffNode::Neg(x) => Op::Neg(x),
                DiffNode::Add(x, y) | DiffNode::Sub(x, y) => Op::Sum(x, y),
                DiffNode::Mul(x, y) => Op::Product(x, y),
            })
            .collect();
        Shape::new(ops, leaves)
    }

    /// Computes the lengths and operation counts, in the order of the graph.
    ///
    /// Operation counts, with `l_x` the worst-case length of operand `x`:
    /// a leaf of two components is one `two_diff`; a sum or difference is a
    /// LINEAR-EXPANSION-SUM, at most two transformations per input
    /// component, `2 (l_x + l_y)`; a product forms `l_x l_y` exact products,
    /// each with at most three transformations, and merges the resulting
    /// scalings, at most `l_x + l_y` merges of inputs of total length at most
    /// `2 l_x l_y`, whichever operand is scaled and in whatever order the
    /// scalings are merged: `3 l_x l_y + 4 l_x l_y (l_x + l_y)`.
    fn new(ops: Vec<Op>, leaves: Leaves) -> Self {
        let (leaf_length, leaf_scale, leaf_operations) = match leaves {
            Leaves::One => (1, 0, 0),
            Leaves::Two => (2, 1, 1),
        };
        let mut lengths: Vec<usize> = Vec::with_capacity(ops.len());
        let mut operations: Vec<u128> = Vec::with_capacity(ops.len());
        for op in &ops {
            let (length, own) = match *op {
                Op::Leaf => (leaf_length, leaf_operations),
                Op::Constant(n) => (usize::from(n != 0), 0),
                Op::Neg(x) => (lengths[x], 0),
                Op::Sum(x, y) => {
                    let length = lengths[x].checked_add(lengths[y]).expect(TOO_LONG);
                    (length, 2 * length as u128)
                }
                Op::Product(x, y) => {
                    let (lx, ly) = (lengths[x], lengths[y]);
                    let length = lx.checked_mul(ly).and_then(|n| n.checked_mul(2));
                    // Lengths fit in 64 bits, so these do not overflow.
                    let pairs = lx as u128 * ly as u128;
                    let merges = pairs.saturating_mul(4 * (lx as u128 + ly as u128));
                    (length.expect(TOO_LONG), merges.saturating_add(3 * pairs))
                }
            };
            let below = match *op {
                Op::Leaf | Op::Constant(_) => 0,
                Op::Neg(x) => operations[x],
                Op::Sum(x, y) | Op::Product(x, y) => operations[x].saturating_add(operations[y]),
            };
            lengths.push(length);
            operations.push(below.saturating_add(own));
        }
        Shape {
            ops,
            leaf_scale,
            lengths,
            operations,
        }
    }

    /// Each operand of each product, once per product it is an operand of.
    pub(crate) fn operands(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.ops
            .iter()
            .filter_map(|op| match *op {
                Op::Product(x, y) => Some([x, y]),
                _ => None,
            })
            .flatten()
    }

    /// For leaves whose nonzero magnitudes are at least `2^m`, the exponent
    /// of a power of two that divides the value and every component of each
    /// node, or `None` for a node that is identically zero.
    ///
    /// A leaf gets `m - 52`, a nonzero constant the power of two that divides
    /// it, a negation that of its operand, a sum or difference the smaller of
    /// those of its operands, and a product their sum.
    pub(crate) fn grids(&self, m: i64) -> Vec<Option<i64>> {
        let mut grids: Vec<Option<i64>> = Vec::with_capacity(self.ops.len());
        for op in &self.ops {
            let grid = match *op {
                Op::Leaf => Some(m - 52),
                Op::Constant(n) => (n != 0).then(|| i64::from(n.trailing_zeros())),
                Op::Neg(x) => grids[x],
                Op::Sum(x, y) => match (grids[x], grids[y]) {
                    (Some(gx), Some(gy)) => Some(gx.min(gy)),
                    (grid, None) | (None, grid) => grid,
                },
                Op::Product(x, y) => grids[x].zip(grids[y]).map(|(gx, gy)| gx + gy),
            };
            grids.push(grid);
        }
        grids
    }

    /// For leaves of magnitude at most `M = 2^big_m`, the bound `S` on the
    /// sum of the magnitudes of the components of each node, before the
    /// rounding factor: `2^s M` for a leaf (see `leaf_scale`), `|n|` for a
    /// constant, the sum of the operands' bounds for a sum or difference,
    /// their product for a product, and that of the operand for a negation.
    ///
    /// `S` also bounds the magnitude of the exact value of the node.
    ///
    /// # Panics
    ///
    /// Panics if a constant is not exactly representable in `f64`.
    pub(crate) fn magnitudes(&self, big_m: i64) -> Vec<Dyadic> {
        let leaf = Dyadic::pow2(big_m + self.leaf_scale);
        let mut magnitudes: Vec<Dyadic> = Vec::with_capacity(self.ops.len());
        for op in &self.ops {
            let magnitude = match *op {
                Op::Leaf => leaf.clone(),
                Op::Constant(n) => {
                    assert!(
                        n.unsigned_abs() <= 1 << 53,
                        "the constant {n} is not exact in f64"
                    );
                    Dyadic::integer(n.unsigned_abs())
                }
                Op::Neg(x) => magnitudes[x].clone(),
                Op::Sum(x, y) => &magnitudes[x] + &magnitudes[y],
                Op::Product(x, y) => &magnitudes[x] * &magnitudes[y],
            };
            magnitudes.push(magnitude);
        }
        magnitudes
    }

    /// For leaves of magnitude at most `2^big_m`, a bound on the magnitude
    /// of every floating-point value the evaluation of each node handles, in
    /// particular of each of its components: `S (1 + 6 u T)`, with `S` from
    /// [`Shape::magnitudes`], `T` the operation count and `u = 2^-53`.
    ///
    /// # Panics
    ///
    /// Panics if a node depends on more than `2^51` error-free
    /// transformations.
    pub(crate) fn component_bounds(&self, big_m: i64) -> Vec<Dyadic> {
        let six_u = Dyadic::new(6, -53);
        let one = Dyadic::integer(1);
        self.magnitudes(big_m)
            .iter()
            .zip(&self.operations)
            .map(|(magnitude, &operations)| {
                assert!(
                    operations <= MAX_OPERATIONS,
                    "the exact evaluation needs too many operations to bound"
                );
                let factor = &one + &(&six_u * &Dyadic::integer(operations));
                magnitude * &factor
            })
            .collect()
    }
}

const TOO_LONG: &str = "a worst-case length overflows usize";
