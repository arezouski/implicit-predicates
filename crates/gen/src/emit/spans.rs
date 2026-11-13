//! The exact stages over arena spans: a form evaluated with the operations
//! of a `Scratch` arena on the exact coordinates of the points.
//!
//! Every node of the form becomes one call of the arena, following the
//! evaluation that [`crate::exact_plan_spans_translated`] and
//! [`crate::exact_plan_spans_untranslated`] derive the range for: a leaf of
//! the translated form is `diff` of two coordinates, a leaf of the
//! untranslated form is the coordinate itself, a constant is `push` of one
//! component (of none for zero), a sum or difference is `sum` or `diff`, a
//! negation is `diff` from the empty expansion and a product is `product`.
//! Whoever changes this scheme changes the model of `range/spans.rs`.

use std::fmt::Write;

use super::{comment, literal};
use crate::differences::{DiffGraph, DiffNode};
use crate::graph::{Graph, Node};

/// The form a stage over spans evaluates.
pub(crate) enum SpanForm<'a> {
    /// The main form, on differences of coordinates.
    Translated(&'a DiffGraph),
    /// The `#[exact]` form, on the coordinates.
    Untranslated(&'a Graph),
}

/// One exact stage over spans to write.
pub(crate) struct SpanStage<'a> {
    /// The name of the function.
    pub(crate) name: &'static str,
    /// Its documentation; a blank line separates paragraphs.
    pub(crate) doc: &'a str,
    /// The form it evaluates.
    pub(crate) form: SpanForm<'a>,
    /// The name of the constant of its range.
    pub(crate) range: &'static str,
}

/// The statements of the evaluation of a form over spans, its root and the
/// coordinates it reads.
struct Evaluation {
    statements: Vec<String>,
    root: String,
    coordinates: Vec<(usize, u8)>,
}

/// The evaluation of `form`: one statement per node that is not a
/// coordinate, the value of node `n` named `e{n}`.
fn evaluation(form: &SpanForm) -> Evaluation {
    let mut statements = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let mut coordinates = Vec::new();
    let call =
        |n: usize, op: &str, x: &str, y: &str| format!("let e{n} = scratch.{op}({x}, {y})?;");
    let root = match form {
        SpanForm::Translated(graph) => {
            if graph.nodes.iter().any(|n| matches!(n, DiffNode::Neg(_))) {
                statements.push("let zero = scratch.push(&[])?;".to_string());
            }
            for (n, node) in graph.nodes.iter().enumerate() {
                let statement = match *node {
                    DiffNode::Leaf {
                        axis,
                        minuend,
                        subtrahend,
                    } => {
                        coordinates.push((minuend, axis));
                        coordinates.push((subtrahend, axis));
                        let (a, b) = (
                            format!("c[{minuend}][{axis}]"),
                            format!("c[{subtrahend}][{axis}]"),
                        );
                        call(n, "diff", &a, &b)
                    }
                    DiffNode::Neg(x) => call(n, "diff", "zero", &names[x]),
                    DiffNode::Add(x, y) => call(n, "sum", &names[x], &names[y]),
                    DiffNode::Sub(x, y) => call(n, "diff", &names[x], &names[y]),
                    DiffNode::Mul(x, y) => call(n, "product", &names[x], &names[y]),
                };
                statements.push(statement);
                names.push(format!("e{n}"));
            }
            graph.root
        }
        SpanForm::Untranslated(graph) => {
            for (n, node) in graph.nodes.iter().enumerate() {
                let name = match *node {
                    Node::Coord { point, axis } => {
                        coordinates.push((point, axis));
                        format!("c[{point}][{axis}]")
                    }
                    Node::Int(0) => {
                        statements.push(format!("let e{n} = scratch.push(&[])?;"));
                        format!("e{n}")
                    }
                    Node::Int(c) => {
                        // An exact constant, as the plan requires.
                        statements.push(format!(
                            "let e{n} = scratch.push(&[{}])?;",
                            literal(c as f64)
                        ));
                        format!("e{n}")
                    }
                    Node::Add(x, y) | Node::Sub(x, y) | Node::Mul(x, y) => {
                        let op = match *node {
                            Node::Add(..) => "sum",
                            Node::Sub(..) => "diff",
                            _ => "product",
                        };
                        statements.push(call(n, op, &names[x], &names[y]));
                        format!("e{n}")
                    }
                };
                names.push(name);
            }
            graph.root
        }
    };
    coordinates.sort_unstable();
    coordinates.dedup();
    Evaluation {
        statements,
        root: names[root].clone(),
        coordinates,
    }
}

/// Writes the function of an exact stage over spans with `points` points.
pub(crate) fn write_span_stage(out: &mut String, stage: &SpanStage, points: usize) {
    let evaluation = evaluation(&stage.form);
    comment(out, "///", 0, stage.doc);
    out.push_str("#[cold]\n#[inline(never)]\n");
    writeln!(
        out,
        "pub fn {}(scratch: &mut Scratch<'_>, p: [&ExactCoords; {points}]) \
         -> Result<Option<Sign>, ScratchExhausted> {{",
        stage.name
    )
    .unwrap();
    writeln!(
        out,
        "fn evaluate(scratch: &mut Scratch<'_>, c: [[Span; 3]; {points}]) \
         -> Result<Sign, ScratchExhausted> {{"
    )
    .unwrap();
    for statement in &evaluation.statements {
        writeln!(out, "{statement}").unwrap();
    }
    writeln!(out, "Ok(scratch.sign({}))\n}}", evaluation.root).unwrap();
    let coordinates: Vec<String> = evaluation
        .coordinates
        .iter()
        .map(|&(point, axis)| format!("c[{point}][{axis}]"))
        .collect();
    comment(
        out,
        "//",
        4,
        "Every coordinate read must be short enough and within the range; the \
         arena is reset to its mark whatever the evaluation returns.",
    );
    writeln!(
        out,
        "if !p.iter().all(|q| q.w == Weight::One) {{\n\
         return Ok(None);\n}}\n\
         let c = p.map(|q| [q.x, q.y, q.z]);\n\
         let coordinates = [{}];\n\
         if !coordinates\n\
         .into_iter()\n\
         .all(|s| span_in_range(scratch.get(s), {}))\n\
         {{\nreturn Ok(None);\n}}\n\
         let mark = scratch.mark();\n\
         let sign = evaluate(scratch, c);\n\
         scratch.reset(mark);\n\
         sign.map(Some)\n\
         }}\n",
        coordinates.join(", "),
        stage.range
    )
    .unwrap();
}

/// Writes the helper `span_in_range` that every stage over spans uses.
pub(crate) fn write_span_in_range(out: &mut String) {
    comment(
        out,
        "///",
        0,
        "Whether the expansion `e` has at most [`MAX_COORD_COMPONENTS`] components, \
         each of a magnitude in `min..=max`. A zero component is not: zero is the empty \
         expansion, and an expansion with a zero component is not valid.",
    );
    out.push_str(
        "fn span_in_range(e: &[f64], (min, max): (f64, f64)) -> bool {\n\
         e.len() <= MAX_COORD_COMPONENTS && e.iter().all(|&x| (min..=max).contains(&x.abs()))\n\
         }\n\n",
    );
}
