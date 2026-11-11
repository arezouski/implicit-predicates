//! The emitted stages have the value of the source polynomial, their buffers
//! are large enough for every call, and the filter bound stays within the
//! operation count its constants allow for; the checks have the value of the
//! source polynomial too, and their cases straddle the bound of the filter.

mod axis;
mod axis_edges;
mod readback;

use std::collections::BTreeMap;

use implicit_predicates_core::eft::{PRODUCT_OPERAND_MAX, PRODUCT_OPERAND_MIN};

use super::edges::bound_edge_cases;
use super::filter::{FilterKind, bound_operations, bound_rounding_factors, bound_statements};
use super::program::{Program, Scalar, Step};
use crate::bound::filter_bound;
use crate::differences::{DiffGraph, DiffNode, difference_graph};
use crate::dyadic::Dyadic;
use crate::graph::{Graph, Node, NodeId};
use crate::parse::{Position, Predicate, parse};
use crate::polynomial::{Polynomial, canonical};
use crate::range::{Leaves, exact_plan_differences, exact_plan_raw};
use readback::{Symbolic, interpret_statements};

const ORIENT3D: &str = include_str!("../../tests/fixtures/orient3d.pred");
const INSPHERE: &str = include_str!("../../tests/fixtures/insphere.pred");

/// Definitions with negations in every position, constants in the exact
/// form, a squared node and a node of degree one.
const NEGATIONS: &str = "
fn n(a: Point3, b: Point3, c: Point3) -> Sign {
    let x = a.x - c.x;
    let y = -(b.y - c.y);
    let s = -x * y - -(x * x);
    -s * -(a.z - b.z) + --y * x * (c.z - a.z)
}
#[exact]
fn n(a: Point3, b: Point3, c: Point3) -> Sign {
    let x = a.x - c.x;
    let y = c.y - b.y;
    let s = -x * y + x * x;
    s * (a.z - b.z) + 2 * y * x * (c.z - a.z) + -1 * y * x * (c.z - a.z) + 0 * a.x * b.x * c.z
}
fn linear(a: Point3, b: Point3) -> Sign {
    -(a.y - b.y)
}
";

/// The canonical polynomial of `graph`, which must be small enough to expand.
fn expand(graph: &Graph) -> Polynomial {
    canonical(graph, "t.pred", Position { line: 1, column: 1 }).unwrap()
}

fn predicates() -> Vec<Predicate> {
    [ORIENT3D, INSPHERE, NEGATIONS]
        .iter()
        .flat_map(|source| parse(source, "t.pred").unwrap())
        .collect()
}

/// The program of each exact stage of `p`, with its name.
fn programs(p: &Predicate) -> Vec<(&'static str, Program, DiffGraph)> {
    let (min, max) = (PRODUCT_OPERAND_MIN, PRODUCT_OPERAND_MAX);
    let diff = difference_graph(&p.main, "t.pred", p.position).unwrap();
    let mut out = Vec::new();
    for (name, leaves) in [("differences", Leaves::One), ("translated", Leaves::Two)] {
        let plan = exact_plan_differences(&diff, leaves, "t.pred", p.position, min, max).unwrap();
        out.push((
            name,
            Program::differences(&diff, leaves, &plan.lengths),
            diff.clone(),
        ));
    }
    if let Some(exact) = &p.exact {
        let plan = exact_plan_raw(exact, "t.pred", p.position, min, max).unwrap();
        out.push((
            "untranslated",
            Program::raw(exact, &plan.lengths),
            diff.clone(),
        ));
    }
    out
}

/// The value of the program as a graph over the coordinates: each step is
/// read as the operation it is written as.
fn interpret(program: &Program) -> Graph {
    let mut g = Symbolic::default();
    let mut scalars: BTreeMap<NodeId, NodeId> = BTreeMap::new();
    let mut expansions: BTreeMap<NodeId, NodeId> = BTreeMap::new();
    let scalar = |g: &mut Symbolic, scalars: &BTreeMap<NodeId, NodeId>, x: Scalar| match x {
        Scalar::Coordinate { point, axis } => g.coordinate(point, axis),
        Scalar::Local(n) => scalars[&n],
        Scalar::Constant(c) => {
            assert_eq!(c.fract(), 0.0);
            g.push(Node::Int(c as i64))
        }
    };
    for step in &program.steps {
        match *step {
            Step::Difference {
                node,
                axis,
                minuend,
                subtrahend,
            } => {
                let (a, b) = (g.coordinate(minuend, axis), g.coordinate(subtrahend, axis));
                let value = g.push(Node::Sub(a, b));
                scalars.insert(node, value);
            }
            Step::NegateScalar { node, x } => {
                let (zero, x) = (g.push(Node::Int(0)), scalar(&mut g, &scalars, x));
                let value = g.push(Node::Sub(zero, x));
                scalars.insert(node, value);
            }
            ref other => {
                let e = |n: NodeId| expansions[&n];
                let value = match *other {
                    Step::Zero { .. } => g.push(Node::Int(0)),
                    Step::FromF64 { x, .. } => scalar(&mut g, &scalars, x),
                    Step::FromTwoDiff { a, b, .. } => {
                        let (a, b) = (scalar(&mut g, &scalars, a), scalar(&mut g, &scalars, b));
                        g.push(Node::Sub(a, b))
                    }
                    Step::FromTwoProduct { a, b, .. } => {
                        let (a, b) = (scalar(&mut g, &scalars, a), scalar(&mut g, &scalars, b));
                        g.push(Node::Mul(a, b))
                    }
                    Step::Scale { e: x, b, .. } => {
                        let b = scalar(&mut g, &scalars, b);
                        g.push(Node::Mul(e(x), b))
                    }
                    Step::Product { x, y, .. } => g.push(Node::Mul(e(x), e(y))),
                    Step::Sum { x, y, .. } => g.push(Node::Add(e(x), e(y))),
                    Step::Diff { x, y, .. } => g.push(Node::Sub(e(x), e(y))),
                    Step::Negate { x, .. } => {
                        let zero = g.push(Node::Int(0));
                        g.push(Node::Sub(zero, e(x)))
                    }
                    Step::Difference { .. } | Step::NegateScalar { .. } => unreachable!(),
                };
                let node = other.expansion().unwrap();
                assert!(
                    expansions.insert(node, value).is_none(),
                    "e{node} written twice"
                );
            }
        }
    }
    let root = expansions[&program.root];
    g.finish(root)
}

#[test]
fn every_exact_stage_has_the_value_of_the_source_polynomial() {
    let mut checked = 0;
    for p in predicates() {
        let expected = expand(&p.main);
        for (stage, program, _) in programs(&p) {
            assert_eq!(expand(&interpret(&program)), expected, "{} {stage}", p.name);
            checked += 1;
        }
    }
    assert_eq!(checked, 3 + 2 + 3 + 2);
}

/// The capacity each step needs from its output buffer, given the
/// capacities of its inputs, as the core documents it.
fn needed(step: &Step, capacities: &[usize]) -> usize {
    match *step {
        Step::Difference { .. } | Step::NegateScalar { .. } | Step::Zero { .. } => 0,
        Step::FromF64 { .. } => 1,
        Step::FromTwoDiff { .. } | Step::FromTwoProduct { .. } => 2,
        Step::Scale { e, .. } => 2 * capacities[e],
        Step::Product { x, y, .. } => 2 * capacities[x] * capacities[y],
        Step::Sum { x, y, .. } | Step::Diff { x, y, .. } => capacities[x] + capacities[y],
        Step::Negate { x, .. } => capacities[x],
    }
}

#[test]
fn every_buffer_holds_the_longest_result_of_its_call() {
    for p in predicates() {
        for (stage, program, _) in programs(&p) {
            let (scaled, spare) = program.product_scratch();
            for step in &program.steps {
                if let Some(node) = step.expansion() {
                    assert!(
                        program.capacities[node] >= needed(step, &program.capacities),
                        "{} {stage}: {step:?}",
                        p.name
                    );
                }
                if let Step::Product { node, x, y } = *step {
                    // Each scaling of `e{y}`, and each partial sum, which
                    // grows to the length of the product.
                    assert!(scaled >= 2 * program.capacities[y]);
                    assert!(spare >= program.capacities[node]);
                    assert!(program.capacities[x] <= program.capacities[y]);
                }
            }
        }
    }
}

#[test]
fn a_product_of_two_short_expansions_is_a_sum_of_scalings() {
    // In the translated stage of orient3d every leaf has two components,
    // so the 2 x 2 minors are products of two expansions.
    let p = parse(ORIENT3D, "t.pred").unwrap().remove(0);
    let (_, translated, _) = programs(&p).remove(1);
    let products = translated
        .steps
        .iter()
        .filter(|s| matches!(s, Step::Product { .. }))
        .count();
    assert_eq!(products, 9);
    assert_eq!(translated.product_scratch(), (32, 64));
}

#[test]
fn the_filter_bound_has_at_most_2k_plus_8_rounding_factors() {
    for degree in [1, 2, 3, 5, 9, 1022] {
        let statements = bound_statements(degree);
        let operations = statements
            .chars()
            .filter(|c| matches!(c, '+' | '-' | '*' | '/'))
            .count();
        let operations = u32::try_from(operations).unwrap();
        assert_eq!(operations, bound_operations(degree), "{statements}");
        assert_eq!(operations, degree + 3, "{statements}");
        // `d` is computed once and used `degree` times in the bound.
        let bound = statements.split("let bound").nth(1).unwrap();
        let uses_of_d = bound.matches(" d").count();
        assert_eq!(uses_of_d, degree as usize, "{statements}");
        let factors = operations + (u32::try_from(uses_of_d).unwrap() - 1);
        assert_eq!(factors, bound_rounding_factors(degree));
        assert_eq!(factors, 2 * degree + 2);
        assert!(factors <= 2 * degree + 8, "{statements}");
    }
    assert_eq!(
        bound_statements(3),
        "let d = delta + e;\nlet bound = (EPSILON * d + KAPPA * e) * d * d;\n"
    );
}

/// The emitted exact stages of every test predicate, with their names, and
/// the expected polynomial.
fn emitted_stages() -> Vec<(String, String, Polynomial)> {
    let mut out = Vec::new();
    for source in [ORIENT3D, INSPHERE, NEGATIONS] {
        for (p, g) in parse(source, "t.pred")
            .unwrap()
            .iter()
            .zip(super::generate(source, "t.pred", "run the tests.").unwrap())
        {
            for stage in [
                "exact_differences",
                "expansions_translated",
                "expansions_untranslated",
            ] {
                if stage == "expansions_untranslated" && p.exact.is_none() {
                    assert!(!g.stages.contains(stage));
                    continue;
                }
                let code = readback::function(&g.stages, stage).to_string();
                out.push((format!("{} {stage}", p.name), code, expand(&p.main)));
            }
        }
    }
    out
}

#[test]
fn the_text_of_every_exact_stage_has_the_value_of_the_source_polynomial() {
    let stages = emitted_stages();
    for (name, code, expected) in &stages {
        assert_eq!(
            &expand(&readback::interpret_stage(code)),
            expected,
            "{name}"
        );
    }
    // orient3d and n have three stages, in-sphere and linear two.
    assert_eq!(stages.len(), 3 + 2 + 3 + 2);
}

#[test]
fn the_exact_evaluator_of_the_checks_has_the_value_of_the_source_polynomial() {
    for source in [ORIENT3D, INSPHERE, NEGATIONS] {
        for (p, g) in parse(source, "t.pred")
            .unwrap()
            .iter()
            .zip(super::generate(source, "t.pred", "run the tests.").unwrap())
        {
            let code = readback::function(&g.checks, "exact");
            assert_eq!(
                expand(&interpret_statements(code, "c")),
                expand(&p.main),
                "{}",
                p.name
            );
        }
    }
}

/// The exact value of the source form `graph` at the points `p`.
fn value_at(graph: &Graph, p: &[[f64; 3]]) -> Dyadic {
    let mut values: Vec<Dyadic> = Vec::new();
    for node in &graph.nodes {
        let value = match *node {
            Node::Coord { point, axis } => Dyadic::from_f64(p[point][usize::from(axis)]),
            Node::Int(n) => Dyadic::integer(n),
            Node::Add(x, y) => &values[x] + &values[y],
            Node::Sub(x, y) => &values[x] - &values[y],
            Node::Mul(x, y) => &values[x] * &values[y],
        };
        values.push(value);
    }
    values.swap_remove(graph.root)
}

#[test]
fn the_bound_edge_cases_straddle_the_bound() {
    // The global form; the per-axis one is checked in `axis`.
    for source in [INSPHERE] {
        let p = parse(source, "t.pred").unwrap().remove(0);
        let diff = difference_graph(&p.main, "t.pred", p.position).unwrap();
        let constants = filter_bound(&diff, "t.pred", p.position)
            .unwrap()
            .constants();
        let cases = bound_edge_cases(&diff, p.params.len(), &FilterKind::Global(constants));
        assert!(cases.len() >= 32, "{}: {} pairs", p.name, cases.len());
        // `|value| > EPSILON * delta^k`, with `delta` the largest difference
        // of coordinates of the form, computed in `f64` as the filter does.
        let above = |q: &[[f64; 3]]| {
            let mut delta = 0.0f64;
            for node in &diff.nodes {
                if let DiffNode::Leaf {
                    axis,
                    minuend,
                    subtrahend,
                } = *node
                {
                    let axis = usize::from(axis);
                    delta = delta.max((q[minuend][axis] - q[subtrahend][axis]).abs());
                }
            }
            assert!((constants.d_min..=constants.d_max).contains(&delta));
            let mut bound = Dyadic::from_f64(constants.epsilon);
            for _ in 0..constants.degree {
                bound = &bound * &Dyadic::from_f64(delta);
            }
            let value = value_at(&p.main, q);
            value > bound || value < &Dyadic::integer(0) - &bound
        };
        let mut scales = Vec::new();
        for (below, over) in &cases {
            assert!(!above(below), "{}: {below:?}", p.name);
            assert!(above(over), "{}: {over:?}", p.name);
            // One coordinate apart, by one representable value.
            let moved: Vec<(f64, f64)> = below
                .iter()
                .flatten()
                .zip(over.iter().flatten())
                .filter(|(a, b)| a.to_bits() != b.to_bits())
                .map(|(&a, &b)| (a, b))
                .collect();
            assert_eq!(moved.len(), 1, "{}: {below:?} {over:?}", p.name);
            let (a, b) = moved[0];
            assert!(a.next_up() == b || a.next_down() == b, "{a:e} {b:e}");
            let top = below.iter().flatten().fold(0.0f64, |m, x| m.max(x.abs()));
            scales.push(top.log2().floor() as i64);
        }
        // From near D_MIN to near D_MAX.
        let (low, high) = (constants.d_min.log2() as i64, constants.d_max.log2() as i64);
        assert!(scales.iter().any(|&s| s < low + 10), "{scales:?}");
        assert!(scales.iter().any(|&s| s > high - 13), "{scales:?}");
    }
}
