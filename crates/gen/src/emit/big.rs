//! The final exact stage on big integers: the main form evaluated with the
//! big-integer operations of a `Scratch` arena on the coordinates of the
//! points as big integers.
//!
//! Every node of the form becomes one statement, its value named `e{n}` as
//! in the stages over spans: a leaf is `big_sub` of two coordinates, a sum,
//! difference or product is `big_add`, `big_sub` or `big_mul`, and a
//! negation is `neg`, which writes nothing. The operations are exact, so the
//! stage always decides. Whoever changes this scheme changes the model of
//! `range/big.rs`.

use std::fmt::Write;

use super::comment;
use crate::differences::{DiffGraph, DiffNode};

/// The name of the stage.
pub(crate) const BIG_STAGE: &str = "big_integers_in";

/// The name of the constant of the size of its arena for explicit points.
pub(crate) const BIG_ARENA: &str = "BIG_INTEGERS_EXPLICIT_ARENA";

/// Writes the constant of the size of the arena, `words`.
pub(crate) fn write_big_arena(out: &mut String, words: usize) {
    comment(
        out,
        "///",
        0,
        &format!(
            "The `f64` slots of the arena, one 64-bit word each, that suffice for \
             [`{BIG_STAGE}`] on explicit points, their coordinates included: writing the \
             coordinates of the points as big integers and running the stage take at most \
             this many slots above the top of the arena. Points with an implicit coordinate \
             can need more."
        ),
    );
    writeln!(out, "pub const {BIG_ARENA}: usize = {words};\n").unwrap();
}

/// Writes the stage on big integers of the main form `graph` with `points`
/// points.
pub(crate) fn write_big_stage(out: &mut String, graph: &DiffGraph, points: usize) {
    comment(
        out,
        "///",
        0,
        &format!(
            "The exact sign of the predicate for points given by their coordinates as big \
             integers: its main form evaluated with the big-integer operations of `scratch`, \
             which are exact whatever the size of the values, so this stage always \
             decides.\n\n\
             For explicit points, [`{BIG_ARENA}`] slots of `scratch`, the coordinates \
             included, suffice.\n\n\
             The arena is left as it was, on success and on error.\n\n\
             # Errors\n\n\
             [`ScratchExhausted`] if `scratch` runs out of room."
        ),
    );
    out.push_str("#[cold]\n#[inline(never)]\n");
    writeln!(
        out,
        "pub fn {BIG_STAGE}(scratch: &mut Scratch<'_>, p: [&BigCoords; {points}]) \
         -> Result<Sign, ScratchExhausted> {{"
    )
    .unwrap();
    writeln!(
        out,
        "fn evaluate(scratch: &mut Scratch<'_>, c: [[Big; 3]; {points}]) \
         -> Result<Sign, ScratchExhausted> {{"
    )
    .unwrap();
    for (n, node) in graph.nodes.iter().enumerate() {
        let value = match *node {
            DiffNode::Leaf {
                axis,
                minuend,
                subtrahend,
            } => format!("scratch.big_sub(c[{minuend}][{axis}], c[{subtrahend}][{axis}])?"),
            DiffNode::Neg(x) => format!("e{x}.neg()"),
            DiffNode::Add(x, y) => format!("scratch.big_add(e{x}, e{y})?"),
            DiffNode::Sub(x, y) => format!("scratch.big_sub(e{x}, e{y})?"),
            DiffNode::Mul(x, y) => format!("scratch.big_mul(e{x}, e{y})?"),
        };
        writeln!(out, "let e{n} = {value};").unwrap();
    }
    writeln!(out, "Ok(e{}.sign())\n}}", graph.root).unwrap();
    comment(
        out,
        "//",
        4,
        "The arena is reset to its mark whatever the evaluation returns.",
    );
    out.push_str(
        "let c = p.map(|q| [q.x, q.y, q.z]);\n\
         let mark = scratch.mark();\n\
         let sign = evaluate(scratch, c);\n\
         scratch.reset(mark);\n\
         sign\n\
         }\n\n",
    );
}
