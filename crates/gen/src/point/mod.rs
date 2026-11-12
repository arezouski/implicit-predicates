//! Point types: how a point is computed from its parameters, and which
//! parameters are allowed.
//!
//! A point type is read from a declaration in a `.pred` file by
//! [`parse_points`]:
//!
//! ```text
//! point Explicit(p: F64x3) = p;
//!
//! point Lnc(p: F64x3, q: F64x3, t: f64) = q + t*(p - q)
//!     where 0 <= t, t <= 1;
//! ```
//!
//! # Representation
//!
//! The three coordinates of the point are held in one [`PointGraph`], in which
//! equal subexpressions share a node, also across coordinates: the scalar `t`
//! of the example is one node used by the three of them. Every operation on an
//! `F64x3` was split per coordinate when the declaration was read, so each node
//! is an operation on `f64` values, which is what the later steps evaluate, one
//! operation at a time, with an error bound for each. Negation is the explicit
//! node [`PointNode::Neg`] rather than `0 - x`, because it is exact.
//!
//! The `where` clause is a list of [`Constraint`]s, each one `lhs <= rhs`
//! between two [`LinearSum`]s of `f64` parameters, named by index, and an
//! integer constant: the form `a + b + 3`, ready to be emitted as code.

mod error_bound;
mod expr;
mod parse;

use crate::parse::Position;

pub use error_bound::{ErrorBound, MAX_ROUNDS, error_bound};
pub(crate) use error_bound::{ErrorProgram, Term};
pub use parse::parse_points;
pub(crate) use parse::read_declaration;

/// The most nodes that the [`PointGraph`] of a point type may have for
/// [`generate_points`](crate::generate_points) to emit it: distinct
/// operations and inputs, counted once each over the three coordinates.
pub const MAX_POINT_NODES: usize = 1024;

/// The type of a parameter of a point type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamType {
    /// Three `f64` coordinates.
    F64x3,
    /// One `f64`.
    F64,
}

/// A parameter of a point type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PointParam {
    /// Name of the parameter.
    pub name: String,
    /// Its type.
    pub ty: ParamType,
    /// Position of its name.
    pub position: Position,
}

/// A node of a [`PointGraph`]: an operation on `f64` values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PointNode {
    /// A coordinate of an `F64x3` parameter.
    Coord {
        /// Index of the parameter in [`PointType::params`].
        param: usize,
        /// Coordinate: 0, 1 and 2 are x, y and z.
        axis: u8,
    },
    /// An `f64` parameter, by index.
    Scalar(usize),
    /// An integer constant.
    Int(i64),
    /// The negation of a node, which is exact.
    Neg(usize),
    /// Sum of two nodes.
    Add(usize, usize),
    /// Difference of two nodes.
    Sub(usize, usize),
    /// Product of two nodes.
    Mul(usize, usize),
}

/// The three coordinates of a point as one graph of `f64` operations in which
/// equal subexpressions share a node.
///
/// Children come before their parents and every node is reachable from one of
/// the roots.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PointGraph {
    /// The nodes, in dependency order. Their operands are indices into this
    /// vector.
    pub nodes: Vec<PointNode>,
    /// The nodes holding the x, y and z coordinates of the point.
    pub roots: [usize; 3],
}

/// A sum of `f64` parameters and an integer constant: the side of a
/// [`Constraint`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinearSum {
    /// Indices of the `f64` parameters added, in the order written; one may
    /// appear more than once.
    pub params: Vec<usize>,
    /// The sum of the integer literals.
    pub constant: i64,
}

/// A comparison `lhs <= rhs` of the `where` clause of a point type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Constraint {
    /// The left side.
    pub lhs: LinearSum,
    /// The right side.
    pub rhs: LinearSum,
    /// Position of the first token of the comparison.
    pub position: Position,
}

/// A point type read from a definition file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PointType {
    /// Name of the type.
    pub name: String,
    /// Position of its name.
    pub position: Position,
    /// The parameters, in order.
    pub params: Vec<PointParam>,
    /// The coordinates of the point, computed from the parameters.
    pub graph: PointGraph,
    /// The `where` clause: what the parameters must satisfy, all at once.
    pub constraints: Vec<Constraint>,
}

impl PointType {
    /// Tells whether this is an explicit point: one `F64x3` parameter,
    /// returned as is, without constraints.
    #[must_use]
    pub fn is_explicit(&self) -> bool {
        let [param] = self.params.as_slice() else {
            return false;
        };
        param.ty == ParamType::F64x3
            && self.constraints.is_empty()
            && self.graph.roots.iter().enumerate().all(|(axis, &root)| {
                self.graph.nodes[root]
                    == PointNode::Coord {
                        param: 0,
                        axis: axis as u8,
                    }
            })
    }
}

/// Builds a [`PointGraph`], giving equal nodes the same index.
#[derive(Default)]
pub(crate) struct PointBuilder {
    nodes: Vec<PointNode>,
    index: std::collections::BTreeMap<PointNode, usize>,
}

impl PointBuilder {
    /// Returns the index of `node`, adding it if it is new.
    pub(crate) fn intern(&mut self, node: PointNode) -> usize {
        *self.index.entry(node).or_insert_with(|| {
            self.nodes.push(node);
            self.nodes.len() - 1
        })
    }

    /// Keeps only the nodes reachable from `roots`, in their original order.
    pub(crate) fn finish(self, roots: [usize; 3]) -> PointGraph {
        let mut live = vec![false; self.nodes.len()];
        for root in roots {
            live[root] = true;
        }
        for id in (0..self.nodes.len()).rev() {
            if !live[id] {
                continue;
            }
            match self.nodes[id] {
                PointNode::Neg(x) => live[x] = true,
                PointNode::Add(x, y) | PointNode::Sub(x, y) | PointNode::Mul(x, y) => {
                    live[x] = true;
                    live[y] = true;
                }
                _ => {}
            }
        }
        let mut new_id = vec![0; self.nodes.len()];
        let mut nodes = Vec::new();
        for (id, node) in self.nodes.iter().enumerate().filter(|&(id, _)| live[id]) {
            new_id[id] = nodes.len();
            nodes.push(match *node {
                PointNode::Neg(x) => PointNode::Neg(new_id[x]),
                PointNode::Add(x, y) => PointNode::Add(new_id[x], new_id[y]),
                PointNode::Sub(x, y) => PointNode::Sub(new_id[x], new_id[y]),
                PointNode::Mul(x, y) => PointNode::Mul(new_id[x], new_id[y]),
                leaf => leaf,
            });
        }
        PointGraph {
            nodes,
            roots: roots.map(|root| new_id[root]),
        }
    }
}
