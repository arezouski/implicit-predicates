//! The statements that evaluate a difference form node by node, shared by
//! the filter, in `f64`, and by the exact evaluator of the checks.

use crate::differences::{DiffGraph, DiffNode};
use crate::graph::NodeId;

/// The name of the local that holds node `id`: `det` for the root, `t{id}`
/// for the others.
pub(crate) fn node_name(graph: &DiffGraph, id: NodeId) -> String {
    if id == graph.root {
        "det".to_string()
    } else {
        format!("t{id}")
    }
}

/// One statement per node of `graph`, in the order of the graph, each with
/// the operation its node names: no reassociation, and a negation as a
/// unary minus. A leaf subtracts coordinates read from `array`, as
/// `array[point][axis]`. With `by_reference`, every operand is borrowed, as
/// the operators of a number type that is not `Copy` need.
pub(crate) fn node_statements(graph: &DiffGraph, array: &str, by_reference: bool) -> Vec<String> {
    let borrow = if by_reference { "&" } else { "" };
    let name = |id: NodeId| format!("{borrow}{}", node_name(graph, id));
    graph
        .nodes
        .iter()
        .enumerate()
        .map(|(id, node)| {
            let value = match *node {
                DiffNode::Leaf {
                    axis,
                    minuend,
                    subtrahend,
                } => format!(
                    "{borrow}{array}[{minuend}][{axis}] - {borrow}{array}[{subtrahend}][{axis}]"
                ),
                DiffNode::Neg(x) => format!("-{}", name(x)),
                DiffNode::Add(x, y) => format!("{} + {}", name(x), name(y)),
                DiffNode::Sub(x, y) => format!("{} - {}", name(x), name(y)),
                DiffNode::Mul(x, y) => format!("{} * {}", name(x), name(y)),
            };
            format!("let {} = {value};", node_name(graph, id))
        })
        .collect()
}
