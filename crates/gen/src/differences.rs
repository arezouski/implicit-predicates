//! The difference form of a predicate: a graph whose leaves are differences
//! of point coordinates.

use crate::error::Error;
use crate::graph::{Graph, Node, NodeId};
use crate::parse::Position;

/// One node of a [`DiffGraph`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffNode {
    /// The difference `minuend.axis - subtrahend.axis` of one coordinate of
    /// two different points, given as parameter indices.
    Leaf {
        /// Coordinate: 0, 1 and 2 are x, y and z.
        axis: u8,
        /// Index of the parameter whose coordinate is subtracted from.
        minuend: usize,
        /// Index of the parameter whose coordinate is subtracted.
        subtrahend: usize,
    },
    /// Negation of a node, with the degree of its operand.
    Neg(NodeId),
    /// Sum of two nodes of the same degree.
    Add(NodeId, NodeId),
    /// Difference of two nodes of the same degree.
    Sub(NodeId, NodeId),
    /// Product of two nodes.
    Mul(NodeId, NodeId),
}

/// An expression graph homogeneous in differences of point coordinates.
///
/// Children come before their parents. The graph is a faithful image of the
/// source: `0 - x` becomes `Neg(x)`, and nothing is folded or reordered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffGraph {
    /// The nodes, in dependency order.
    pub nodes: Vec<DiffNode>,
    /// The node holding the value of the whole expression.
    pub root: NodeId,
    /// Degree of each node: 1 for a leaf, the sum for a product, the common
    /// degree for a sum or difference.
    pub degrees: Vec<u32>,
}

/// What a node of the source graph became.
#[derive(Clone, Copy)]
enum Mapped {
    /// A coordinate, which is only allowed as an operand of a difference.
    Coord { point: usize, axis: u8 },
    /// An integer literal.
    Literal(i64),
    /// A node of the result.
    Node(NodeId),
}

/// Rewrites `graph` over differences of coordinates, checking that every sum
/// and difference combines nodes of the same degree.
///
/// A leaf is a difference of the same coordinate of two different points.
/// A leaf used twice is one node; `a.x - d.x` and `d.x - a.x` are different
/// leaves. A negation is `0 - x`, kept as a `Neg` node. The graph is walked iteratively in node order.
///
/// # Errors
///
/// Returns an error, at `position` (the name of the predicate), when a
/// coordinate is used outside a difference, a difference mixes axes or
/// subtracts a point from itself, a sum or difference combines nodes of
/// different degree, a degree exceeds `u32::MAX`, or an integer literal
/// appears other than as the zero of a negation.
pub fn difference_graph(graph: &Graph, file: &str, position: Position) -> Result<DiffGraph, Error> {
    let fail = |message: String| Error {
        file: file.to_string(),
        line: position.line,
        column: position.column,
        message,
    };
    let mut out = DiffGraph {
        nodes: Vec::new(),
        root: 0,
        degrees: Vec::new(),
    };
    let mut mapped: Vec<Mapped> = Vec::with_capacity(graph.nodes.len());
    for node in &graph.nodes {
        let value = match *node {
            Node::Coord { point, axis } => Mapped::Coord { point, axis },
            Node::Int(n) => Mapped::Literal(n),
            Node::Sub(x, y) => match (mapped[x], mapped[y]) {
                (Mapped::Coord { point: p, axis: i }, Mapped::Coord { point: q, axis: j }) => {
                    if i != j {
                        return Err(fail(format!(
                            "a difference of coordinates must use one axis, found {} and {}",
                            coordinate(p, i),
                            coordinate(q, j)
                        )));
                    }
                    if p == q {
                        return Err(fail(format!(
                            "a difference of {} with itself is always zero",
                            coordinate(p, i)
                        )));
                    }
                    Mapped::Node(push(
                        &mut out,
                        DiffNode::Leaf {
                            axis: i,
                            minuend: p,
                            subtrahend: q,
                        },
                        1,
                    ))
                }
                (Mapped::Literal(0), Mapped::Node(id)) => {
                    let degree = out.degrees[id];
                    Mapped::Node(push(&mut out, DiffNode::Neg(id), degree))
                }
                (Mapped::Literal(0), operand) => return Err(fail(operand_problem(operand))),
                (x, y) => combine(&mut out, DiffNode::Sub, x, y, &fail)?,
            },
            Node::Add(x, y) => combine(&mut out, DiffNode::Add, mapped[x], mapped[y], &fail)?,
            Node::Mul(x, y) => combine(&mut out, DiffNode::Mul, mapped[x], mapped[y], &fail)?,
        };
        mapped.push(value);
    }
    match mapped[graph.root] {
        Mapped::Node(root) => {
            out.root = root;
            Ok(out)
        }
        other => Err(fail(operand_problem(other))),
    }
}

/// Adds the sum, difference or product of two operands to `out`.
fn combine(
    out: &mut DiffGraph,
    make: fn(NodeId, NodeId) -> DiffNode,
    x: Mapped,
    y: Mapped,
    fail: &dyn Fn(String) -> Error,
) -> Result<Mapped, Error> {
    let (Mapped::Node(x), Mapped::Node(y)) = (x, y) else {
        let bad = if matches!(x, Mapped::Node(_)) { y } else { x };
        return Err(fail(operand_problem(bad)));
    };
    let (gx, gy) = (out.degrees[x], out.degrees[y]);
    let node = make(x, y);
    let degree = if matches!(node, DiffNode::Mul(..)) {
        gx.checked_add(gy).ok_or_else(|| {
            fail(format!(
                "a product has a degree in the differences above {}",
                u32::MAX
            ))
        })?
    } else if gx == gy {
        gx
    } else {
        return Err(fail(format!(
            "a sum or difference combines terms of degree {gx} and {gy} in the differences"
        )));
    };
    Ok(Mapped::Node(push(out, node, degree)))
}

fn push(out: &mut DiffGraph, node: DiffNode, degree: u32) -> NodeId {
    out.nodes.push(node);
    out.degrees.push(degree);
    out.nodes.len() - 1
}

fn operand_problem(bad: Mapped) -> String {
    match bad {
        Mapped::Coord { point, axis } => format!(
            "the coordinate {} is used outside a difference of coordinates",
            coordinate(point, axis)
        ),
        Mapped::Literal(n) => format!(
            "the integer literal {n} is not allowed in the main form, \
             except as the zero of a negation"
        ),
        Mapped::Node(_) => unreachable!("only coordinates and literals are problems"),
    }
}

fn coordinate(point: usize, axis: u8) -> String {
    format!("`point {point}`.{}", ["x", "y", "z"][usize::from(axis)])
}
