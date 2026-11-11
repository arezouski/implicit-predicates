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

/// The limit on the worst-case length of a temporary, and on the total
/// length of all of them: fewer than `2^32` components, 32 GiB, so that
/// every length fits in a `usize` even on a 32-bit host.
pub(crate) const MAX_LENGTH: u64 = 1 << 32;

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
    /// The highest degree in the leaves of every node, saturated at
    /// `u64::MAX`.
    pub(crate) degrees: Vec<u64>,
    /// `T`: the number of error-free transformations of every node and of
    /// everything it is computed from, with multiplicity, saturated at
    /// `u128::MAX`.
    operations: Vec<u128>,
}

impl Shape {
    /// The raw form: every coordinate is a leaf of one component.
    ///
    /// Fails, with the message to report, when a constant is not exact in
    /// `f64` or the evaluation is too large (see [`Shape::new`]).
    pub(crate) fn raw(graph: &Graph) -> Result<Self, String> {
        let ops = graph
            .nodes
            .iter()
            .map(|node| match *node {
                Node::Coord { .. } => Ok(Op::Leaf),
                Node::Int(n) => {
                    let magnitude = n.unsigned_abs();
                    if magnitude >> magnitude.trailing_zeros().min(63) < 1 << 53 {
                        Ok(Op::Constant(n))
                    } else {
                        Err(format!(
                            "the constant {n} is not exactly representable in f64, \
                             so it cannot be used in exact evaluation"
                        ))
                    }
                }
                Node::Add(x, y) | Node::Sub(x, y) => Ok(Op::Sum(x, y)),
                Node::Mul(x, y) => Ok(Op::Product(x, y)),
            })
            .collect::<Result<_, _>>()?;
        Shape::new(ops, Leaves::One)
    }

    /// A difference form, with leaves of one or two components.
    ///
    /// Fails, with the message to report, when the evaluation is too large
    /// (see [`Shape::new`]).
    pub(crate) fn differences(graph: &DiffGraph, leaves: Leaves) -> Result<Self, String> {
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
    ///
    /// Fails, with the message to report, at the first node whose length
    /// reaches [`MAX_LENGTH`] or whose operation count exceeds `2^51`, before
    /// anything is computed from the oversized values.
    fn new(ops: Vec<Op>, leaves: Leaves) -> Result<Self, String> {
        let (leaf_length, leaf_scale, leaf_operations): (u64, i64, u128) = match leaves {
            Leaves::One => (1, 0, 0),
            Leaves::Two => (2, 1, 1),
        };
        let mut lengths: Vec<u64> = Vec::with_capacity(ops.len());
        let mut degrees: Vec<u64> = Vec::with_capacity(ops.len());
        let mut operations: Vec<u128> = Vec::with_capacity(ops.len());
        for op in &ops {
            // Lengths are below 2^32, so none of this overflows.
            let (length, degree, own) = match *op {
                Op::Leaf => (leaf_length, 1, leaf_operations),
                Op::Constant(n) => (u64::from(n != 0), 0, 0),
                Op::Neg(x) => (lengths[x], degrees[x], 0),
                Op::Sum(x, y) => {
                    let length = lengths[x] + lengths[y];
                    (length, degrees[x].max(degrees[y]), 2 * u128::from(length))
                }
                Op::Product(x, y) => {
                    let (lx, ly) = (u128::from(lengths[x]), u128::from(lengths[y]));
                    let length = u64::try_from(2 * lx * ly).unwrap_or(u64::MAX);
                    let degree = degrees[x].saturating_add(degrees[y]);
                    (length, degree, 3 * lx * ly + 4 * lx * ly * (lx + ly))
                }
            };
            if length >= MAX_LENGTH {
                return Err(
                    "the expression is too large for exact evaluation: a temporary \
                            would need 2^32 components or more"
                        .to_string(),
                );
            }
            let below = match *op {
                Op::Leaf | Op::Constant(_) => 0,
                Op::Neg(x) => operations[x],
                Op::Sum(x, y) | Op::Product(x, y) => operations[x].saturating_add(operations[y]),
            };
            let count = below.saturating_add(own);
            if count > MAX_OPERATIONS {
                return Err(
                    "the expression is too large for exact evaluation: a temporary \
                            would take more than 2^51 floating-point operations"
                        .to_string(),
                );
            }
            lengths.push(length);
            degrees.push(degree);
            operations.push(count);
        }
        let lengths = lengths
            .into_iter()
            .map(|n| usize::try_from(n).expect("a length below 2^32 fits in usize"))
            .collect();
        Ok(Shape {
            ops,
            leaf_scale,
            lengths,
            degrees,
            operations,
        })
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
    pub(crate) fn magnitudes(&self, big_m: i64) -> Vec<Dyadic> {
        let leaf = Dyadic::pow2(big_m + self.leaf_scale);
        let mut magnitudes: Vec<Dyadic> = Vec::with_capacity(self.ops.len());
        for op in &self.ops {
            let magnitude = match *op {
                Op::Leaf => leaf.clone(),
                Op::Constant(n) => Dyadic::integer(n.unsigned_abs()),
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
    /// [`Shape::magnitudes`], `T <= 2^51` the operation count and
    /// `u = 2^-53`.
    pub(crate) fn component_bounds(&self, big_m: i64) -> Vec<Dyadic> {
        let six_u = Dyadic::new(6, -53);
        let one = Dyadic::integer(1);
        self.magnitudes(big_m)
            .iter()
            .zip(&self.operations)
            .map(|(magnitude, &operations)| {
                let factor = &one + &(&six_u * &Dyadic::integer(operations));
                magnitude * &factor
            })
            .collect()
    }
}
