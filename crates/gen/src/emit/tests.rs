//! The emitted stages have the value of the source polynomial, their buffers
//! are large enough for every call, and the filter bound stays within the
//! operation count its constants allow for.

use std::collections::BTreeMap;

use indirect_predicates_core::eft::{PRODUCT_OPERAND_MAX, PRODUCT_OPERAND_MIN};

use super::filter::{bound_operations, bound_statements, write_filter};
use super::program::{Program, Scalar, Step};
use crate::differences::{DiffGraph, difference_graph};
use crate::graph::{Graph, Node, NodeId};
use crate::parse::{Predicate, parse};
use crate::polynomial::canonical;
use crate::range::{Leaves, exact_plan_differences, exact_plan_raw};

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

/// A graph under construction, without sharing.
#[derive(Default)]
struct Symbolic {
    nodes: Vec<Node>,
}

impl Symbolic {
    fn push(&mut self, node: Node) -> NodeId {
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    fn coordinate(&mut self, point: usize, axis: u8) -> NodeId {
        self.push(Node::Coord { point, axis })
    }

    fn finish(self, root: NodeId) -> Graph {
        Graph {
            nodes: self.nodes,
            root,
        }
    }
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
        let expected = canonical(&p.main);
        for (stage, program, _) in programs(&p) {
            assert_eq!(
                canonical(&interpret(&program)),
                expected,
                "{} {stage}",
                p.name
            );
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

/// The value of the emitted filter, read back from its statements.
fn interpret_filter(code: &str) -> Graph {
    let mut g = Symbolic::default();
    let mut names: BTreeMap<String, NodeId> = BTreeMap::new();
    let operand = |g: &mut Symbolic, names: &BTreeMap<String, NodeId>, text: &str| {
        if let Some(rest) = text.strip_prefix("p[") {
            let (point, axis) = rest.trim_end_matches(']').split_once("][").unwrap();
            g.coordinate(point.parse().unwrap(), axis.parse().unwrap())
        } else {
            names[text]
        }
    };
    let mut last = None;
    for line in code.lines() {
        let Some(statement) = line.strip_prefix("let ") else {
            continue;
        };
        let (name, value) = statement.trim_end_matches(';').split_once(" = ").unwrap();
        if !(name == "det" || name.starts_with('t')) {
            continue;
        }
        let tokens: Vec<&str> = value.split(' ').collect();
        let id = match tokens.as_slice() {
            [x] => {
                let x = x.strip_prefix('-').expect("a negation");
                let (zero, x) = (g.push(Node::Int(0)), operand(&mut g, &names, x));
                g.push(Node::Sub(zero, x))
            }
            [x, op, y] => {
                let (x, y) = (operand(&mut g, &names, x), operand(&mut g, &names, y));
                g.push(match *op {
                    "+" => Node::Add(x, y),
                    "-" => Node::Sub(x, y),
                    "*" => Node::Mul(x, y),
                    _ => panic!("{line}"),
                })
            }
            _ => panic!("{line}"),
        };
        names.insert(name.to_string(), id);
        last = Some(name.to_string());
    }
    assert_eq!(last.as_deref(), Some("det"));
    let root = names["det"];
    g.finish(root)
}

#[test]
fn the_filter_has_the_value_of_the_source_polynomial() {
    for p in predicates() {
        let diff = difference_graph(&p.main, "t.pred", p.position).unwrap();
        let mut code = String::new();
        write_filter(&mut code, &diff, p.params.len(), diff.degrees[diff.root]);
        assert_eq!(
            canonical(&interpret_filter(&code)),
            canonical(&p.main),
            "{}",
            p.name
        );
    }
}

#[test]
fn the_filter_bound_takes_at_most_2k_plus_8_operations() {
    for degree in [1, 2, 3, 5, 9, 1022] {
        let statements = bound_statements(degree);
        let in_code = statements
            .chars()
            .filter(|c| matches!(c, '+' | '-' | '*' | '/'))
            .count();
        // The caller forms `e` with one more sum.
        let total = u32::try_from(in_code).unwrap() + 1;
        assert_eq!(total, bound_operations(degree), "{statements}");
        assert!(total <= 2 * degree + 8, "{statements}");
        assert_eq!(statements.matches("* d").count(), degree as usize - 1 + 1);
    }
    assert_eq!(
        bound_statements(3),
        "let d = delta + e;\nlet bound = (EPSILON * d + KAPPA * e) * d * d;\n"
    );
}
