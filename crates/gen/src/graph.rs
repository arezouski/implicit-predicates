//! Expression graphs with shared subexpressions.

use std::collections::BTreeMap;

/// Index of a node in [`Graph::nodes`].
pub type NodeId = usize;

/// One node of an expression graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Node {
    /// A coordinate of a point parameter.
    Coord {
        /// Index of the parameter.
        point: usize,
        /// Coordinate: 0, 1 and 2 are x, y and z.
        axis: u8,
    },
    /// An integer constant.
    Int(i64),
    /// Sum of two nodes.
    Add(NodeId, NodeId),
    /// Difference of two nodes.
    Sub(NodeId, NodeId),
    /// Product of two nodes.
    Mul(NodeId, NodeId),
}

/// A polynomial expression as a graph in which equal subexpressions share a
/// node.
///
/// Children come before their parents, every node is reachable from the root,
/// and the root is the last node. Unary minus is represented as `0 - x`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Graph {
    /// The nodes, in dependency order.
    pub nodes: Vec<Node>,
    /// The node holding the value of the whole expression.
    pub root: NodeId,
}

impl Graph {
    /// An upper bound on the total degree of the polynomial, found in one
    /// pass without expanding it: a coordinate has degree 1, a constant 0, a
    /// sum the larger degree of its operands and a product their sum,
    /// saturated at `u64::MAX`. Terms that cancel can make the true degree
    /// lower.
    pub(crate) fn degree_bound(&self) -> u64 {
        let mut degrees: Vec<u64> = Vec::with_capacity(self.nodes.len());
        for node in &self.nodes {
            degrees.push(match *node {
                Node::Coord { .. } => 1,
                Node::Int(_) => 0,
                Node::Add(x, y) | Node::Sub(x, y) => degrees[x].max(degrees[y]),
                Node::Mul(x, y) => degrees[x].saturating_add(degrees[y]),
            });
        }
        degrees[self.root]
    }
}

/// Builds a graph, giving equal nodes the same id.
#[derive(Default)]
pub(crate) struct Builder {
    nodes: Vec<Node>,
    index: BTreeMap<Node, NodeId>,
}

impl Builder {
    /// Returns the id of `node`, adding it if it is new.
    pub(crate) fn intern(&mut self, node: Node) -> NodeId {
        *self.index.entry(node).or_insert_with(|| {
            self.nodes.push(node);
            self.nodes.len() - 1
        })
    }

    /// Returns the id of `0 - x`.
    pub(crate) fn negate(&mut self, x: NodeId) -> NodeId {
        let zero = self.intern(Node::Int(0));
        self.intern(Node::Sub(zero, x))
    }

    /// Keeps only the nodes reachable from `root`, in their original order.
    pub(crate) fn finish(self, root: NodeId) -> Graph {
        let mut live = vec![false; self.nodes.len()];
        live[root] = true;
        for id in (0..self.nodes.len()).rev() {
            if !live[id] {
                continue;
            }
            if let Node::Add(x, y) | Node::Sub(x, y) | Node::Mul(x, y) = self.nodes[id] {
                live[x] = true;
                live[y] = true;
            }
        }
        let mut new_id = vec![0; self.nodes.len()];
        let mut nodes = Vec::new();
        for (id, node) in self.nodes.iter().enumerate().filter(|&(id, _)| live[id]) {
            new_id[id] = nodes.len();
            nodes.push(match *node {
                Node::Add(x, y) => Node::Add(new_id[x], new_id[y]),
                Node::Sub(x, y) => Node::Sub(new_id[x], new_id[y]),
                Node::Mul(x, y) => Node::Mul(new_id[x], new_id[y]),
                leaf => leaf,
            });
        }
        Graph {
            root: nodes.len() - 1,
            nodes,
        }
    }
}
