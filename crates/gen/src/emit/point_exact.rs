//! The exact coordinates of a point type: `write_exact`, which writes each
//! coordinate as an expansion in the arena with `Scratch::write`.
//!
//! Each coordinate is computed in its own closure, node by node: a leaf, or
//! the negation of one, is an `f64`; every other node is an expansion in a
//! stack buffer of its worst-case length, except the coordinate itself,
//! which is written to the components the arena lends. Before each
//! operation the closure checks the requirements of the core on its
//! operands: every component of an operand of a sum below `2^1020`, and of
//! an operand of a product in the product domain of `eft`. If one fails,
//! the closure writes nothing and `write_exact` fails with
//! `ExactError::OutOfRange`.

use std::fmt::Write;

use super::comment;
use super::points::{Implicit, TypeInfo};
use crate::error::Error;
use crate::point::{PointNode, PointType};

/// The most `f64` that the stack buffers of one coordinate may take, as
/// for the stages of a predicate.
pub(crate) const MAX_STACK: usize = 4096;

/// How the exact value of a node is held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Repr {
    /// An `f64`, exactly.
    Scalar,
    /// An expansion of at most this many components.
    Expansion(usize),
}

/// The operation that computes an expansion node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Op {
    TwoSum(usize, usize),
    TwoDiff(usize, usize),
    TwoProduct(usize, usize),
    /// An expansion plus a scalar, or minus it when `negate`.
    Grow {
        e: usize,
        s: usize,
        negate: bool,
    },
    /// A scalar minus an expansion.
    LiftDiff {
        s: usize,
        e: usize,
    },
    Sum(usize, usize),
    Diff(usize, usize),
    Scale {
        e: usize,
        s: usize,
    },
    Product(usize, usize),
    Negate(usize),
}

/// How the exact coordinates of a point type are computed.
pub(crate) struct ExactPlan {
    repr: Vec<Repr>,
    ops: Vec<Option<Op>>,
}

impl ExactPlan {
    /// The plan of `point`.
    ///
    /// # Errors
    ///
    /// If the buffers of a coordinate take more than [`MAX_STACK`] `f64`.
    pub(crate) fn of(point: &PointType, file: &str) -> Result<ExactPlan, Error> {
        let mut repr: Vec<Repr> = Vec::with_capacity(point.graph.nodes.len());
        let mut ops = Vec::with_capacity(point.graph.nodes.len());
        for node in &point.graph.nodes {
            let length = |id: usize| match repr[id] {
                Repr::Scalar => None,
                Repr::Expansion(n) => Some(n),
            };
            let (op, len) = match *node {
                PointNode::Coord { .. } | PointNode::Scalar(_) | PointNode::Int(_) => (None, None),
                PointNode::Neg(x) => match length(x) {
                    None => (None, None),
                    Some(n) => (Some(Op::Negate(x)), Some(n)),
                },
                PointNode::Add(x, y) | PointNode::Sub(x, y) => {
                    let sub = matches!(node, PointNode::Sub(..));
                    match (length(x), length(y)) {
                        (None, None) if sub => (Some(Op::TwoDiff(x, y)), Some(2)),
                        (None, None) => (Some(Op::TwoSum(x, y)), Some(2)),
                        (Some(n), None) => (
                            Some(Op::Grow {
                                e: x,
                                s: y,
                                negate: sub,
                            }),
                            Some(n + 1),
                        ),
                        (None, Some(n)) if sub => (Some(Op::LiftDiff { s: x, e: y }), Some(n + 1)),
                        (None, Some(n)) => (
                            Some(Op::Grow {
                                e: y,
                                s: x,
                                negate: false,
                            }),
                            Some(n + 1),
                        ),
                        (Some(a), Some(b)) if sub => {
                            (Some(Op::Diff(x, y)), Some(a.saturating_add(b)))
                        }
                        (Some(a), Some(b)) => (Some(Op::Sum(x, y)), Some(a.saturating_add(b))),
                    }
                }
                PointNode::Mul(x, y) => match (length(x), length(y)) {
                    (None, None) => (Some(Op::TwoProduct(x, y)), Some(2)),
                    (Some(n), None) => (Some(Op::Scale { e: x, s: y }), Some(n.saturating_mul(2))),
                    (None, Some(n)) => (Some(Op::Scale { e: y, s: x }), Some(n.saturating_mul(2))),
                    (Some(a), Some(b)) => (
                        Some(Op::Product(x, y)),
                        Some(a.saturating_mul(b).saturating_mul(2)),
                    ),
                },
            };
            // Saturated lengths are rejected below, before anything uses them.
            repr.push(len.map_or(Repr::Scalar, |n| Repr::Expansion(n.min(usize::MAX / 4))));
            ops.push(op);
        }
        let plan = ExactPlan { repr, ops };
        for (axis, &root) in point.graph.roots.iter().enumerate() {
            let stack = plan.stack(&plan.reachable(point, root));
            if stack > MAX_STACK {
                return Err(Error::at(
                    file,
                    point.position,
                    format!(
                        "the exact {} coordinate of `{}` needs buffers of {stack} `f64` or \
                         more, above the {MAX_STACK} that its computation may take on the stack",
                        ["x", "y", "z"][axis],
                        point.name
                    ),
                ));
            }
        }
        Ok(plan)
    }

    /// Whether some coordinate takes an exact product.
    pub(crate) fn uses_products(&self) -> bool {
        self.ops
            .iter()
            .flatten()
            .any(|op| matches!(op, Op::TwoProduct(..) | Op::Scale { .. } | Op::Product(..)))
    }

    pub(crate) fn uses(&self, f: fn(&Op) -> bool) -> bool {
        self.ops.iter().flatten().any(f)
    }

    /// The nodes that coordinate `root` reads, in order.
    fn reachable(&self, point: &PointType, root: usize) -> Vec<usize> {
        let mut live = vec![false; point.graph.nodes.len()];
        live[root] = true;
        for id in (0..=root).rev() {
            if !live[id] {
                continue;
            }
            match point.graph.nodes[id] {
                PointNode::Neg(x) => live[x] = true,
                PointNode::Add(x, y) | PointNode::Sub(x, y) | PointNode::Mul(x, y) => {
                    live[x] = true;
                    live[y] = true;
                }
                _ => {}
            }
        }
        (0..=root).filter(|&id| live[id]).collect()
    }

    /// The length of the expansion of `id`, one for a scalar.
    fn length(&self, id: usize) -> usize {
        match self.repr[id] {
            Repr::Scalar => 1,
            Repr::Expansion(n) => n,
        }
    }

    /// The `f64` that the buffers of the nodes `nodes` take, the last one
    /// being written to the arena instead.
    fn stack(&self, nodes: &[usize]) -> usize {
        let Some((&root, rest)) = nodes.split_last() else {
            return 0;
        };
        let mut total = self.length(root);
        for &id in rest {
            if self.repr[id] != Repr::Scalar {
                total = total.saturating_add(self.length(id));
            }
        }
        for &id in nodes {
            total = total.saturating_add(match self.ops[id] {
                Some(Op::LiftDiff { .. }) => 1,
                Some(Op::Product(x, y)) => work_length(self.length(x), self.length(y)),
                _ => 0,
            });
        }
        total
    }
}

/// The working space of the product of expansions of `a` and `b` components.
fn work_length(a: usize, b: usize) -> usize {
    b.saturating_mul(2)
        .saturating_add(a.saturating_mul(b).saturating_mul(2))
}

/// `&x{id}[..n{id}]`, the expansion of node `id`.
fn expansion(id: usize) -> String {
    format!("&x{id}[..n{id}]")
}

/// Writes `write_exact` for the type `info`.
pub(crate) fn write_exact(out: &mut String, info: &TypeInfo<'_>, implicit: &Implicit) {
    let plan = &implicit.exact;
    let point = info.point;
    comment(
        out,
        "///",
        0,
        "Writes the exact coordinates in `scratch`, from the `F64x3` parameters `c` and the \
         `f64` parameters `s`, in the order of the declaration. Fails with \
         [`ExactError::OutOfRange`] if an input is not finite, or an operand of a sum of \
         expansions reaches `2^1020`, or an operand of an exact product leaves the product \
         domain; on an error the arena is as it was.",
    );
    writeln!(
        out,
        "fn write_exact(scratch: &mut Scratch<'_>, {}) -> Result<ExactCoords, ExactError> {{",
        info.array_params()
    )
    .unwrap();
    let mut finite = String::from("c.iter().flatten().all(|x| x.is_finite())");
    if info.scalars > 0 {
        finite.push_str(" && s.iter().all(|x| x.is_finite())");
    }
    writeln!(
        out,
        "if !({finite}) {{\nreturn Err(ExactError::OutOfRange);\n}}\nlet mark = scratch.mark();"
    )
    .unwrap();
    let coordinates: Vec<Vec<usize>> = (point.graph.roots.iter())
        .map(|&root| plan.reachable(point, root))
        .collect();
    let checked = coordinates
        .iter()
        .flatten()
        .any(|&id| plan.ops[id].is_some());
    if checked {
        out.push_str("let mut in_range = true;\n");
    }
    let flag = if checked { "in_range" } else { "true" };
    for (name, nodes) in ["x", "y", "z"].iter().zip(&coordinates) {
        let root = *nodes.last().expect("a coordinate reads its root");
        writeln!(
            out,
            "let {name} = scratch.write({}, |out| {{",
            plan.length(root)
        )
        .unwrap();
        for &id in nodes {
            write_node(out, info, plan, id, id == root);
        }
        writeln!(
            out,
            "}});\nlet {name} = written(scratch, mark, {name}, {flag})?;"
        )
        .unwrap();
    }
    out.push_str("Ok(ExactCoords {\nx,\ny,\nz,\nw: Weight::One,\n})\n}\n");
}

/// Writes the statements of node `id`; the root is the value of the
/// closure.
fn write_node(out: &mut String, info: &TypeInfo<'_>, plan: &ExactPlan, id: usize, root: bool) {
    let node = info.point.graph.nodes[id];
    let Some(op) = plan.ops[id] else {
        let value = match node {
            PointNode::Neg(x) => format!("-v{x}"),
            leaf => info.leaf(leaf).expect("a leaf"),
        };
        writeln!(out, "let v{id} = {value};").unwrap();
        if root {
            writeln!(out, "expansion::from_f64(v{id}, out)").unwrap();
        }
        return;
    };
    let scalar = |s: usize| format!("&[v{s}]");
    let (check, operands, call) = match op {
        Op::TwoSum(x, y) => (
            "summable",
            [scalar(x), scalar(y)],
            format!("from_two_sum(v{x}, v{y}"),
        ),
        Op::TwoDiff(x, y) => (
            "summable",
            [scalar(x), scalar(y)],
            format!("from_two_diff(v{x}, v{y}"),
        ),
        Op::TwoProduct(x, y) => (
            "multipliable",
            [scalar(x), scalar(y)],
            format!("from_two_product(v{x}, v{y}"),
        ),
        Op::Grow { e, s, negate } => (
            "summable",
            [expansion(e), scalar(s)],
            format!(
                "grow({}, {}v{s}",
                expansion(e),
                if negate { "-" } else { "" }
            ),
        ),
        Op::LiftDiff { s, e } => {
            writeln!(
                out,
                "let mut l{id} = [0.0; 1];\nlet m{id} = expansion::from_f64(v{s}, &mut l{id});"
            )
            .unwrap();
            let lifted = format!("&l{id}[..m{id}]");
            (
                "summable",
                [lifted.clone(), expansion(e)],
                format!("diff({lifted}, {}", expansion(e)),
            )
        }
        Op::Sum(x, y) => (
            "summable",
            [expansion(x), expansion(y)],
            format!("sum({}, {}", expansion(x), expansion(y)),
        ),
        Op::Diff(x, y) => (
            "summable",
            [expansion(x), expansion(y)],
            format!("diff({}, {}", expansion(x), expansion(y)),
        ),
        Op::Scale { e, s } => (
            "multipliable",
            [expansion(e), scalar(s)],
            format!("scale({}, v{s}", expansion(e)),
        ),
        Op::Product(x, y) => (
            "multipliable",
            [expansion(x), expansion(y)],
            format!("product({}, {}", expansion(x), expansion(y)),
        ),
        Op::Negate(x) => (
            "summable",
            [String::from("&[]"), expansion(x)],
            format!("diff(&[], {}", expansion(x)),
        ),
    };
    let condition = match op {
        // An empty operand meets every requirement.
        Op::Negate(_) => format!("{check}({})", operands[1]),
        _ => format!("{check}({}) && {check}({})", operands[0], operands[1]),
    };
    writeln!(
        out,
        "if !({condition}) {{\nin_range = false;\nreturn 0;\n}}"
    )
    .unwrap();
    let module = if matches!(op, Op::Product(..)) {
        ""
    } else {
        "expansion::"
    };
    let work = if let Op::Product(x, y) = op {
        let length = work_length(plan.length(x), plan.length(y));
        writeln!(out, "let mut w{id} = [0.0; {length}];").unwrap();
        format!(", &mut w{id}")
    } else {
        String::new()
    };
    if root {
        writeln!(out, "{module}{call}, out{work})").unwrap();
    } else {
        writeln!(
            out,
            "let mut x{id} = [0.0; {}];\nlet n{id} = {module}{call}, &mut x{id}{work});",
            plan.length(id)
        )
        .unwrap();
    }
}
