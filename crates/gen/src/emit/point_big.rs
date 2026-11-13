//! The exact coordinates of a point type as big integers: `write_exact_big`,
//! which evaluates the declaration node by node with the big-integer
//! operations of the arena.
//!
//! A leaf is converted with `big_from_f64`, a sum, difference or product is
//! the operation of the same name, and a negation flips the sign of its
//! operand without touching the arena. Before the first operation the
//! function checks that every input is finite, the only way the evaluation
//! can fail other than by running out of room.

use std::fmt::Write;

use super::comment;
use super::points::TypeInfo;
use crate::point::PointNode;

/// Writes `write_exact_big` for the type `info`.
pub(crate) fn write_exact_big(out: &mut String, info: &TypeInfo<'_>) {
    let point = info.point;
    comment(
        out,
        "///",
        0,
        "Writes the exact coordinates as big integers in `scratch`, from the `F64x3` \
         parameters `c` and the `f64` parameters `s`, in the order of the declaration. Fails \
         with [`ExactError::OutOfRange`] if an input is not finite, before it writes anything; \
         on [`ExactError::Exhausted`] the arena may hold partial results.",
    );
    writeln!(
        out,
        "fn write_exact_big(scratch: &mut Scratch<'_>, {}) -> Result<BigCoords, ExactError> {{",
        info.array_params()
    )
    .unwrap();
    let mut finite = String::from("c.iter().flatten().all(|x| x.is_finite())");
    if info.scalars > 0 {
        finite.push_str(" && s.iter().all(|x| x.is_finite())");
    }
    writeln!(
        out,
        "if !({finite}) {{\nreturn Err(ExactError::OutOfRange);\n}}"
    )
    .unwrap();
    let nodes = &point.graph.nodes;
    let mut live = vec![false; nodes.len()];
    for &root in &point.graph.roots {
        live[root] = true;
    }
    for id in (0..nodes.len()).rev() {
        if !live[id] {
            continue;
        }
        match nodes[id] {
            PointNode::Neg(x) => live[x] = true,
            PointNode::Add(x, y) | PointNode::Sub(x, y) | PointNode::Mul(x, y) => {
                live[x] = true;
                live[y] = true;
            }
            _ => {}
        }
    }
    for (id, &node) in nodes.iter().enumerate().filter(|&(id, _)| live[id]) {
        let value = match node {
            PointNode::Neg(x) => format!("v{x}.neg()"),
            PointNode::Add(x, y) => format!("scratch.big_add(v{x}, v{y})?"),
            PointNode::Sub(x, y) => format!("scratch.big_sub(v{x}, v{y})?"),
            PointNode::Mul(x, y) => format!("scratch.big_mul(v{x}, v{y})?"),
            leaf => format!(
                "scratch.big_from_f64({})?",
                info.leaf(leaf).expect("a leaf")
            ),
        };
        writeln!(out, "let v{id} = {value};").unwrap();
    }
    let [x, y, z] = point.graph.roots;
    writeln!(out, "Ok(BigCoords::new(v{x}, v{y}, v{z}))\n}}").unwrap();
}
