//! Tests for the difference form.

use indirect_predicates_gen::{DiffNode, Error, difference_graph, parse};

const ORIENT3D: &str = include_str!("fixtures/orient3d.pred");
const INSPHERE: &str = include_str!("fixtures/insphere.pred");

fn diff(source: &str) -> Result<indirect_predicates_gen::DiffGraph, Error> {
    let p = parse(source, "t.pred").unwrap().remove(0);
    difference_graph(&p.main, "t.pred")
}

fn rejected(source: &str, fragment: &str) {
    let e = diff(source).expect_err("the form must be rejected");
    assert!(e.message.contains(fragment), "{e}");
    assert_eq!(e.file, "t.pred");
    assert_eq!((e.line, e.column), (1, 4), "{e}");
}

#[test]
fn orient3d_has_nine_leaves_and_degree_3() {
    let g = diff(ORIENT3D).unwrap();
    let leaves = g
        .nodes
        .iter()
        .filter(|n| matches!(n, DiffNode::Leaf { .. }))
        .count();
    assert_eq!(leaves, 9);
    assert_eq!(g.degrees[g.root], 3);
    assert_eq!(g.degrees.len(), g.nodes.len());
    for (id, node) in g.nodes.iter().enumerate() {
        match *node {
            DiffNode::Leaf {
                minuend,
                subtrahend,
                ..
            } => {
                assert_eq!(g.degrees[id], 1);
                assert_eq!(subtrahend, 3);
                assert!(minuend < 3);
            }
            DiffNode::Add(x, y) | DiffNode::Sub(x, y) | DiffNode::Mul(x, y) => {
                assert!(x < id && y < id);
            }
        }
    }
}

#[test]
fn insphere_has_degree_5() {
    let g = diff(INSPHERE).unwrap();
    assert_eq!(g.degrees[g.root], 5);
}

#[test]
fn negation_keeps_the_degree_and_the_leaves() {
    let g = diff("fn p(a: Point3, d: Point3) -> Sign { -(a.x - d.x) * (a.y - d.y) }").unwrap();
    assert_eq!(g.degrees[g.root], 2);
    assert_eq!(g.nodes.len(), 3);
}

#[test]
fn the_same_difference_is_one_leaf_and_opposite_ones_are_two() {
    let g = diff("fn p(a: Point3, d: Point3) -> Sign { (a.x - d.x) * (a.x - d.x) }").unwrap();
    assert_eq!(g.nodes.len(), 2);
    let g = diff("fn p(a: Point3, d: Point3) -> Sign { (a.x - d.x) * (d.x - a.x) }").unwrap();
    assert_eq!(g.nodes.len(), 3);
}

#[test]
fn a_raw_coordinate_is_rejected() {
    rejected(
        "fn p(a: Point3, d: Point3) -> Sign { a.x * (a.y - d.y) }",
        "outside a difference",
    );
    rejected("fn p(a: Point3) -> Sign { a.x }", "outside a difference");
    rejected(
        "fn p(a: Point3, d: Point3) -> Sign { -a.x * (a.y - d.y) }",
        "outside a difference",
    );
}

#[test]
fn mixing_degrees_in_a_sum_is_rejected() {
    rejected(
        "fn p(a: Point3, d: Point3) -> Sign { (a.x - d.x) * (a.y - d.y) + (a.z - d.z) }",
        "degree 2 and 1",
    );
}

#[test]
fn a_difference_across_axes_is_rejected() {
    rejected(
        "fn p(a: Point3, d: Point3) -> Sign { a.x - d.y }",
        "one axis",
    );
}

#[test]
fn a_point_minus_itself_is_rejected() {
    rejected(
        "fn p(a: Point3, d: Point3) -> Sign { (a.x - a.x) * (a.y - d.y) }",
        "with itself",
    );
}

#[test]
fn a_literal_is_rejected() {
    rejected(
        "fn p(a: Point3, d: Point3) -> Sign { 2 * (a.x - d.x) }",
        "integer literal 2",
    );
}

#[test]
fn a_long_sum_does_not_overflow_the_stack() {
    // A chain of 100 000 sums over two leaves: it must be analyzed without
    // recursion.
    let mut src = String::from("fn p(a: Point3, d: Point3) -> Sign { (a.x - d.x)");
    src.push_str(&" + (a.y - d.y)".repeat(100_000));
    src.push_str(" }");
    // Each sum is a distinct node, so the chain has 100 000 of them.
    let g = diff(&src).unwrap();
    assert_eq!(g.degrees[g.root], 1);
    assert_eq!(g.nodes.len(), 2 + 100_000);
}
