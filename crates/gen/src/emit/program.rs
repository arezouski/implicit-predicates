//! The operations of an exact stage, before they are written as Rust.
//!
//! An exact stage evaluates one form of a predicate with the expansion
//! functions of the arithmetic core. This module turns the form into a list
//! of [`Step`]s, each one call of the core, following the evaluation model
//! that the range and the buffer sizes of [`crate::ExactPlan`] are derived
//! for: a leaf is a coordinate, an exactly computed difference or the
//! `two_diff` of two coordinates; a product of two `f64` is
//! `from_two_product`, an expansion times an `f64` is `scale`, and an
//! expansion times an expansion is a sum of scalings, each component of the
//! shorter operand scaling the longer one, accumulated with `sum`; sums and
//! differences are `sum` and `diff`; a negation negates every component.
//! The writer in [`super::exact`] renders each step as one call, so the
//! value of the program, checked against the source polynomial by the tests,
//! is the value of the emitted code.

use crate::differences::{DiffGraph, DiffNode};
use crate::graph::{Graph, Node, NodeId};
use crate::range::Leaves;

/// An `f64` that a stage reads without an expansion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Scalar {
    /// Coordinate `axis` of point `point`: `p[point][axis]`.
    Coordinate { point: usize, axis: u8 },
    /// The local `v{node}` defined by a [`Step::Difference`] or a
    /// [`Step::NegateScalar`].
    Local(NodeId),
    /// An integer constant, exact in `f64`.
    Constant(f64),
}

/// One operation of an exact stage. Every node `n` of the form holds either
/// a [`Scalar`] or the expansion `e{n}` of capacity `capacities[n]`, whose
/// length is `l{n}`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Step {
    /// `v{node} = p[minuend][axis] - p[subtrahend][axis]` with its rounding
    /// error, which the stage requires to be zero.
    Difference {
        node: NodeId,
        axis: u8,
        minuend: usize,
        subtrahend: usize,
    },
    /// `v{node} = -x`, exactly.
    NegateScalar { node: NodeId, x: Scalar },
    /// `e{node}` is the empty expansion, zero.
    Zero { node: NodeId },
    /// `e{node} = x`.
    FromF64 { node: NodeId, x: Scalar },
    /// `e{node} = a - b`, exactly, with `from_two_diff`.
    FromTwoDiff { node: NodeId, a: Scalar, b: Scalar },
    /// `e{node} = a * b`, exactly, with `from_two_product`.
    FromTwoProduct { node: NodeId, a: Scalar, b: Scalar },
    /// `e{node} = e{e} * b`, with `scale`.
    Scale { node: NodeId, e: NodeId, b: Scalar },
    /// `e{node} = e{x} * e{y}`: the sum of the scalings of `e{y}` by each
    /// component of `e{x}`.
    Product { node: NodeId, x: NodeId, y: NodeId },
    /// `e{node} = e{x} + e{y}`, with `sum`.
    Sum { node: NodeId, x: NodeId, y: NodeId },
    /// `e{node} = e{x} - e{y}`, with `diff`.
    Diff { node: NodeId, x: NodeId, y: NodeId },
    /// `e{node} = -e{x}`: a copy with every component negated.
    Negate { node: NodeId, x: NodeId },
}

/// The steps of an exact stage and what they need.
#[derive(Clone, Debug)]
pub(crate) struct Program {
    /// The operations, in order.
    pub(crate) steps: Vec<Step>,
    /// The capacity of the expansion of every node: its worst-case length.
    pub(crate) capacities: Vec<usize>,
    /// The node whose expansion is the value of the form.
    pub(crate) root: NodeId,
    /// The coordinates whose range the stage checks, sorted.
    pub(crate) coordinates: Vec<(usize, u8)>,
}

/// What a node of the form holds.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Value {
    /// An `f64`.
    Scalar(Scalar),
    /// The expansion `e{n}` of the node `n`.
    Expansion(NodeId),
}

/// What a node is, as far as the evaluation is concerned.
#[derive(Clone, Copy)]
enum Op {
    Coordinate {
        point: usize,
        axis: u8,
    },
    Constant(i64),
    Leaf {
        axis: u8,
        minuend: usize,
        subtrahend: usize,
    },
    Neg(NodeId),
    Add(NodeId, NodeId),
    Sub(NodeId, NodeId),
    Mul(NodeId, NodeId),
}

impl Program {
    /// The program of the raw form `graph`, whose leaves are coordinates,
    /// with the worst-case lengths `lengths` of its exact plan. A difference
    /// `0 - x` is a negation.
    pub(crate) fn raw(graph: &Graph, lengths: &[usize]) -> Program {
        let ops = graph.nodes.iter().map(|node| match *node {
            Node::Coord { point, axis } => Op::Coordinate { point, axis },
            Node::Int(n) => Op::Constant(n),
            Node::Sub(x, y) if graph.nodes[x] == Node::Int(0) => Op::Neg(y),
            Node::Add(x, y) => Op::Add(x, y),
            Node::Sub(x, y) => Op::Sub(x, y),
            Node::Mul(x, y) => Op::Mul(x, y),
        });
        build(ops.collect(), graph.root, lengths, Leaves::One)
    }

    /// The program of the difference form `graph` with leaves of the given
    /// kind and the worst-case lengths `lengths` of its exact plan.
    pub(crate) fn differences(graph: &DiffGraph, leaves: Leaves, lengths: &[usize]) -> Program {
        let ops = graph.nodes.iter().map(|node| match *node {
            DiffNode::Leaf {
                axis,
                minuend,
                subtrahend,
            } => Op::Leaf {
                axis,
                minuend,
                subtrahend,
            },
            DiffNode::Neg(x) => Op::Neg(x),
            DiffNode::Add(x, y) => Op::Add(x, y),
            DiffNode::Sub(x, y) => Op::Sub(x, y),
            DiffNode::Mul(x, y) => Op::Mul(x, y),
        });
        build(ops.collect(), graph.root, lengths, leaves)
    }

    /// The largest scaling buffer and the largest accumulator that the
    /// [`Step::Product`]s need: `2 l_y` and `2 l_x l_y`, or zero without
    /// such products.
    pub(crate) fn product_scratch(&self) -> (usize, usize) {
        self.steps
            .iter()
            .filter_map(|step| match *step {
                Step::Product { node, y, .. } => {
                    Some((2 * self.capacities[y], self.capacities[node]))
                }
                _ => None,
            })
            .fold((0, 0), |(s, a), (ns, na)| (s.max(ns), a.max(na)))
    }

    /// The `f64`s of all the buffers of the stage: one per node that has an
    /// expansion, plus the scratch of the products.
    pub(crate) fn footprint(&self) -> usize {
        let buffers: usize = self
            .steps
            .iter()
            .filter_map(Step::expansion)
            .map(|node| self.capacities[node])
            .sum();
        let (scaled, spare) = self.product_scratch();
        buffers + scaled + spare
    }
}

impl Step {
    /// The node whose expansion the step writes, or `None` for a step that
    /// defines a scalar.
    pub(crate) fn expansion(&self) -> Option<NodeId> {
        match *self {
            Step::Difference { .. } | Step::NegateScalar { .. } => None,
            Step::Zero { node }
            | Step::FromF64 { node, .. }
            | Step::FromTwoDiff { node, .. }
            | Step::FromTwoProduct { node, .. }
            | Step::Scale { node, .. }
            | Step::Product { node, .. }
            | Step::Sum { node, .. }
            | Step::Diff { node, .. }
            | Step::Negate { node, .. } => Some(node),
        }
    }
}

/// Walks the nodes in order and emits the steps.
fn build(ops: Vec<Op>, root: NodeId, lengths: &[usize], leaves: Leaves) -> Program {
    let mut b = Builder {
        steps: Vec::new(),
        values: Vec::with_capacity(ops.len()),
        expansions: vec![false; ops.len()],
        capacities: lengths.to_vec(),
        coordinates: Vec::new(),
    };
    for (node, op) in ops.iter().enumerate() {
        let value = match *op {
            Op::Coordinate { point, axis } => {
                b.coordinate(point, axis);
                Value::Scalar(Scalar::Coordinate { point, axis })
            }
            Op::Constant(n) => Value::Scalar(Scalar::Constant(n as f64)),
            Op::Leaf {
                axis,
                minuend,
                subtrahend,
            } => match leaves {
                Leaves::One => {
                    b.steps.push(Step::Difference {
                        node,
                        axis,
                        minuend,
                        subtrahend,
                    });
                    Value::Scalar(Scalar::Local(node))
                }
                Leaves::Two => {
                    b.coordinate(minuend, axis);
                    b.coordinate(subtrahend, axis);
                    let coordinate = |point| Scalar::Coordinate { point, axis };
                    b.expansion(Step::FromTwoDiff {
                        node,
                        a: coordinate(minuend),
                        b: coordinate(subtrahend),
                    })
                }
            },
            Op::Neg(x) => match b.values[x] {
                Value::Scalar(Scalar::Constant(c)) => Value::Scalar(Scalar::Constant(-c)),
                Value::Scalar(s) => {
                    b.steps.push(Step::NegateScalar { node, x: s });
                    Value::Scalar(Scalar::Local(node))
                }
                Value::Expansion(e) => b.expansion(Step::Negate { node, x: e }),
            },
            Op::Add(x, y) => {
                let (x, y) = (b.materialize(x), b.materialize(y));
                b.expansion(Step::Sum { node, x, y })
            }
            Op::Sub(x, y) => {
                let (x, y) = (b.materialize(x), b.materialize(y));
                b.expansion(Step::Diff { node, x, y })
            }
            Op::Mul(x, y) => b.product(node, x, y),
        };
        b.values.push(value);
    }
    // The sign is read from an expansion.
    let root = b.materialize(root);
    b.coordinates.sort_unstable();
    Program {
        steps: b.steps,
        capacities: b.capacities,
        root,
        coordinates: b.coordinates,
    }
}

struct Builder {
    steps: Vec<Step>,
    values: Vec<Value>,
    /// Whether a scalar node already has its expansion.
    expansions: Vec<bool>,
    capacities: Vec<usize>,
    coordinates: Vec<(usize, u8)>,
}

impl Builder {
    fn coordinate(&mut self, point: usize, axis: u8) {
        if !self.coordinates.contains(&(point, axis)) {
            self.coordinates.push((point, axis));
        }
    }

    /// Adds a step that writes the expansion of its node.
    fn expansion(&mut self, step: Step) -> Value {
        let node = step.expansion().expect("a step that writes an expansion");
        // A node of capacity zero is identically zero.
        self.steps.push(if self.capacities[node] == 0 {
            Step::Zero { node }
        } else {
            step
        });
        Value::Expansion(node)
    }

    /// The expansion of `node`, written once if the node is a scalar.
    fn materialize(&mut self, node: NodeId) -> NodeId {
        match self.values[node] {
            Value::Expansion(e) => e,
            Value::Scalar(x) => {
                if !self.expansions[node] {
                    self.expansions[node] = true;
                    self.expansion(Step::FromF64 { node, x });
                }
                node
            }
        }
    }

    /// The product of `x` and `y`, by the kinds of its operands.
    fn product(&mut self, node: NodeId, x: NodeId, y: NodeId) -> Value {
        if self.capacities[node] == 0 {
            return self.expansion(Step::Zero { node });
        }
        match (self.values[x], self.values[y]) {
            (Value::Scalar(a), Value::Scalar(b)) => {
                self.expansion(Step::FromTwoProduct { node, a, b })
            }
            (Value::Expansion(e), Value::Scalar(b)) | (Value::Scalar(b), Value::Expansion(e)) => {
                self.expansion(Step::Scale { node, e, b })
            }
            (Value::Expansion(x), Value::Expansion(y)) => {
                let (x, y) = if self.capacities[x] <= self.capacities[y] {
                    (x, y)
                } else {
                    (y, x)
                };
                self.expansion(Step::Product { node, x, y })
            }
        }
    }
}
