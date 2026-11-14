//! The exact stages of a predicate, written from their [`Program`]s.
//!
//! Every step of a program becomes one call of the expansion functions of
//! the arithmetic core, on stack buffers whose sizes are the worst-case
//! lengths of the exact plan. Operands are sliced to the length each call
//! returns. The products of two expansions share one scaling buffer and one
//! spare accumulator per stage.

use std::fmt::Write;

use super::comment;
use super::program::{Program, Scalar, Step};

/// What the range check of a stage reads.
pub(crate) enum Check {
    /// The coordinates of the program, against the named range.
    Coordinates(&'static str),
    /// The differences of the program, which must be exact, against the
    /// named range.
    Differences(&'static str),
}

/// One exact stage to write.
pub(crate) struct Stage<'a> {
    /// The name of the function.
    pub(crate) name: &'static str,
    /// Its documentation; a blank line separates paragraphs.
    pub(crate) doc: &'a str,
    /// The program it runs.
    pub(crate) program: &'a Program,
    /// What its range check reads.
    pub(crate) check: Check,
}

/// Writes the function of an exact stage with `points` points.
pub(crate) fn write_stage(out: &mut String, stage: &Stage, points: usize) {
    let program = stage.program;
    comment(out, "///", 0, stage.doc);
    out.push_str("#[cold]\n#[inline(never)]\n");
    writeln!(
        out,
        "pub fn {}(p: [&[f64; 3]; {points}]) -> Option<Sign> {{",
        stage.name
    )
    .unwrap();
    let (scaled, spare) = program.product_scratch();
    let footprint = program.footprint();
    let scratch = if scaled > 0 {
        format!(
            ", {} of them the scratch of the products of two expansions",
            scaled + spare
        )
    } else {
        String::new()
    };
    comment(
        out,
        "//",
        4,
        &format!(
            "Stack: {footprint} f64 ({} bytes) of expansions{scratch}.",
            8 * footprint
        ),
    );
    let is_difference = |step: &&Step| matches!(step, Step::Difference { .. });
    match stage.check {
        Check::Coordinates(range) => {
            let coordinates: Vec<String> = program
                .coordinates
                .iter()
                .map(|&(point, axis)| format!("p[{point}][{axis}]"))
                .collect();
            writeln!(
                out,
                "let coordinates = [{}];\n\
                 if !coordinates.into_iter().all(|x| in_range(x, {range})) {{\n\
                 return None;\n}}",
                coordinates.join(", ")
            )
            .unwrap();
        }
        Check::Differences(range) => {
            let mut differences = Vec::new();
            for step in program.steps.iter().filter(is_difference) {
                write_step(out, step, program);
                if let Step::Difference { node, .. } = step {
                    differences.push(*node);
                }
            }
            let list = |prefix: &str| {
                let names: Vec<String> =
                    differences.iter().map(|n| format!("{prefix}{n}")).collect();
                names.join(", ")
            };
            comment(
                out,
                "//",
                4,
                "Each difference must be exact, and within the range.",
            );
            writeln!(
                out,
                "let errors = [{}];\n\
                 let differences = [{}];\n\
                 if !(errors.into_iter().all(|r| r == 0.0)\n\
                 && differences.into_iter().all(|x| in_range(x, {range})))\n\
                 {{\nreturn None;\n}}",
                list("r"),
                list("v")
            )
            .unwrap();
        }
    }
    if scaled > 0 {
        writeln!(
            out,
            "let mut scaled = [0.0; {scaled}];\nlet mut spare = [0.0; {spare}];"
        )
        .unwrap();
    }
    for step in program.steps.iter().filter(|s| !is_difference(s)) {
        write_step(out, step, program);
    }
    writeln!(
        out,
        "Some(expansion::sign(&e{0}[..l{0}]))\n}}\n",
        program.root
    )
    .unwrap();
}

/// Writes one step as one call.
fn write_step(out: &mut String, step: &Step, program: &Program) {
    let slice = |n: usize| format!("&e{n}[..l{n}]");
    if let Some(node) = step.expansion() {
        if !matches!(step, Step::Zero { .. }) {
            writeln!(
                out,
                "let mut e{node} = [0.0; {}];",
                program.capacities[node]
            )
            .unwrap();
        }
    }
    let line = match *step {
        Step::Difference {
            node,
            axis,
            minuend,
            subtrahend,
        } => format!(
            "let (v{node}, r{node}) = eft::two_diff(p[{minuend}][{axis}], p[{subtrahend}][{axis}]);"
        ),
        Step::NegateScalar { node, x } => format!("let v{node} = -{};", scalar(x)),
        Step::Zero { node } => format!("let e{node}: [f64; 0] = [];\nlet l{node} = 0;"),
        Step::FromF64 { node, x } => {
            format!(
                "let l{node} = expansion::from_f64({}, &mut e{node});",
                scalar(x)
            )
        }
        Step::FromTwoDiff { node, a, b } => format!(
            "let l{node} = expansion::from_two_diff({}, {}, &mut e{node});",
            scalar(a),
            scalar(b)
        ),
        Step::FromTwoProduct { node, a, b } => format!(
            "let l{node} = expansion::from_two_product({}, {}, &mut e{node});",
            scalar(a),
            scalar(b)
        ),
        Step::Scale { node, e, b } => format!(
            "let l{node} = expansion::scale({}, {}, &mut e{node});",
            slice(e),
            scalar(b)
        ),
        Step::Product { node, x, y } => format!(
            "let l{node} = product({}, {}, &mut scaled, &mut spare, &mut e{node});",
            slice(x),
            slice(y)
        ),
        Step::Sum { node, x, y } => format!(
            "let l{node} = expansion::sum({}, {}, &mut e{node});",
            slice(x),
            slice(y)
        ),
        Step::Diff { node, x, y } => format!(
            "let l{node} = expansion::diff({}, {}, &mut e{node});",
            slice(x),
            slice(y)
        ),
        Step::Negate { node, x } => format!(
            "e{node}[..l{x}].copy_from_slice({});\n\
             expansion::negate(&mut e{node}[..l{x}]);\n\
             let l{node} = l{x};",
            slice(x)
        ),
    };
    out.push_str(&line);
    out.push('\n');
}

fn scalar(x: Scalar) -> String {
    match x {
        Scalar::Coordinate { point, axis } => format!("p[{point}][{axis}]"),
        Scalar::Local(node) => format!("v{node}"),
    }
}

/// Writes the helper `in_range` that every exact stage uses.
pub(crate) fn write_in_range(out: &mut String) {
    comment(
        out,
        "///",
        0,
        "Whether `x` is zero or has a magnitude in `min..=max`; false for NaN and \
         infinities.",
    );
    out.push_str(
        "fn in_range(x: f64, (min, max): (f64, f64)) -> bool {\n\
         x == 0.0 || (min..=max).contains(&x.abs())\n\
         }\n\n",
    );
}

/// Writes the helper `product` that a [`Step::Product`] calls.
pub(crate) fn write_product(out: &mut String) {
    comment(
        out,
        "///",
        0,
        "Writes the expansion of `x * y` to `out` and returns its length: the sum of \
         the scalings of `y` by each component of `x`, each scaling written to \
         `scaled`, the partial sums alternating between `spare` and `out`.",
    );
    out.push_str(
        "fn product(x: &[f64], y: &[f64], scaled: &mut [f64], spare: &mut [f64], \
         out: &mut [f64]) -> usize {\n\
         let Some((&first, rest)) = x.split_first() else {\n\
         return 0;\n\
         };\n\
         // Start so that the last partial sum is written to `out`.\n\
         let (mut partial, mut next) = if rest.len() % 2 == 0 { (out, spare) } else { (spare, out) };\n\
         let mut length = expansion::scale(y, first, partial);\n\
         for &component in rest {\n\
         let scaled_length = expansion::scale(y, component, scaled);\n\
         length = expansion::sum(&partial[..length], &scaled[..scaled_length], next);\n\
         core::mem::swap(&mut partial, &mut next);\n\
         }\n\
         length\n\
         }\n\n",
    );
}
