//! Tests for reading predicate definitions.

use indirect_predicates_gen::{Error, Graph, Node, parse};

const ORIENT3D: &str = "\
fn orient3d(a: Point3, b: Point3, c: Point3, d: Point3) -> Sign {
    let ax = a.x - d.x;  let ay = a.y - d.y;  let az = a.z - d.z;
    let bx = b.x - d.x;  let by = b.y - d.y;  let bz = b.z - d.z;
    let cx = c.x - d.x;  let cy = c.y - d.y;  let cz = c.z - d.z;
    ax*(by*cz - bz*cy) - ay*(bx*cz - bz*cx) + az*(bx*cy - by*cx)
}
";

fn count(graph: &Graph, pred: impl Fn(&Node) -> bool) -> usize {
    graph.nodes.iter().filter(|n| pred(n)).count()
}

fn rejected(source: &str) -> Error {
    parse(source, "t.pred").expect_err("the source must be rejected")
}

fn check(source: &str, line: usize, column: usize, fragment: &str) {
    let e = rejected(source);
    assert_eq!((e.line, e.column), (line, column), "{e}");
    assert!(e.message.contains(fragment), "{e}");
    assert_eq!(e.file, "t.pred");
    assert_eq!(
        e.to_string(),
        format!("t.pred:{line}:{column}: {}", e.message)
    );
}

#[test]
fn parses_the_orient3d_definition() {
    let preds = parse(ORIENT3D, "orient3d.pred").unwrap();
    assert_eq!(preds.len(), 1);
    let p = &preds[0];
    assert_eq!(p.name, "orient3d");
    assert_eq!(p.params, ["a", "b", "c", "d"]);
    assert!(p.exact.is_none());
    assert_eq!(count(&p.main, |n| matches!(n, Node::Coord { .. })), 12);
    // Children come before parents and the root is the last node.
    assert_eq!(p.main.root, p.main.nodes.len() - 1);
    for (id, node) in p.main.nodes.iter().enumerate() {
        if let Node::Add(x, y) | Node::Sub(x, y) | Node::Mul(x, y) = node {
            assert!(*x < id && *y < id);
        }
    }
}

#[test]
fn shared_subexpressions_share_a_node() {
    let src = "fn p(a: Point3, d: Point3) -> Sign { let m = a.x - d.x; m*m + m*m }";
    let p = &parse(src, "t.pred").unwrap()[0];
    assert_eq!(count(&p.main, |n| matches!(n, Node::Sub(..))), 1);
    assert_eq!(count(&p.main, |n| matches!(n, Node::Mul(..))), 1);
    assert_eq!(count(&p.main, |n| matches!(n, Node::Add(..))), 1);
}

#[test]
fn unary_minus_is_canonical_and_unused_lets_are_dropped() {
    let src = "fn p(a: Point3) -> Sign { let u = a.y * a.y; -a.x * -a.x + -a.x }";
    let p = &parse(src, "t.pred").unwrap()[0];
    // `-a.x` is `0 - a.x`, shared by all three occurrences.
    assert_eq!(count(&p.main, |n| matches!(n, Node::Sub(..))), 1);
    assert_eq!(count(&p.main, |n| matches!(n, Node::Int(0))), 1);
    // The unused `let` leaves no node behind.
    assert_eq!(
        count(&p.main, |n| matches!(n, Node::Coord { axis: 1, .. })),
        0
    );
}

#[test]
fn an_exact_form_is_attached_to_its_predicate() {
    let src = "\
fn p(a: Point3, b: Point3) -> Sign { a.x * b.x }
#[exact]
fn p(a: Point3, b: Point3) -> Sign { a.x * b.x + a.y * b.y }
fn q(a: Point3) -> Sign { a.z }
";
    let preds = parse(src, "t.pred").unwrap();
    assert_eq!(preds.len(), 2);
    assert_eq!(preds[0].name, "p");
    assert_eq!(count(&preds[0].main, |n| matches!(n, Node::Add(..))), 0);
    let exact = preds[0].exact.as_ref().expect("exact form");
    assert_eq!(count(exact, |n| matches!(n, Node::Add(..))), 1);
    assert_eq!(preds[1].name, "q");
    assert!(preds[1].exact.is_none());
}

#[test]
fn integer_literals_parse() {
    let p = &parse("fn p(a: Point3) -> Sign { 2 * a.x }", "t.pred").unwrap()[0];
    assert_eq!(count(&p.main, |n| matches!(n, Node::Int(2))), 1);
}

#[test]
fn empty_source_has_no_predicates_and_is_an_error() {
    check("", 1, 1, "no predicate");
    check("// just a comment\n", 1, 1, "no predicate");
}

#[test]
fn division_is_rejected() {
    check(
        "fn p(a: Point3, b: Point3) -> Sign {\n    a.x / b.x\n}\n",
        2,
        9,
        "division",
    );
}

#[test]
fn unknown_name_is_rejected() {
    check(
        "fn p(a: Point3, b: Point3) -> Sign {\n    a.x * q\n}\n",
        2,
        11,
        "unknown name",
    );
}

#[test]
fn unknown_field_is_rejected() {
    check(
        "fn p(a: Point3, b: Point3) -> Sign {\n    a.w * b.x\n}\n",
        2,
        7,
        "unknown field",
    );
}

#[test]
fn a_let_that_redefines_a_name_is_rejected() {
    check(
        "fn p(a: Point3, b: Point3) -> Sign {\n    let a = b.x;\n    a * b.x\n}\n",
        2,
        9,
        "redefine",
    );
    check(
        "fn p(a: Point3) -> Sign {\n    let m = a.x;\n    let m = a.y;\n    m\n}\n",
        3,
        9,
        "redefine",
    );
}

#[test]
fn an_unused_parameter_is_rejected() {
    check(
        "fn p(a: Point3, b: Point3) -> Sign {\n    a.x * a.y\n}\n",
        1,
        17,
        "unused",
    );
}

#[test]
fn a_parameter_that_is_not_point3_is_rejected() {
    check(
        "fn p(a: f64, b: Point3) -> Sign {\n    a * b.x\n}\n",
        1,
        9,
        "Point3",
    );
}

#[test]
fn two_main_forms_with_one_name_are_rejected() {
    let src = "fn p(a: Point3) -> Sign {\n    a.x\n}\nfn p(a: Point3) -> Sign {\n    a.y\n}\n";
    check(src, 4, 4, "already defined");
}

#[test]
fn an_exact_form_without_a_main_form_is_rejected() {
    check(
        "#[exact]\nfn p(a: Point3) -> Sign {\n    a.x\n}\n",
        2,
        4,
        "main form",
    );
}

#[test]
fn an_exact_form_with_other_parameters_is_rejected() {
    let src = "\
fn p(a: Point3, b: Point3) -> Sign {
    a.x * b.x
}
#[exact]
fn p(a: Point3, c: Point3) -> Sign {
    a.x * c.x
}
";
    check(src, 5, 4, "parameters");
}

#[test]
fn a_syntax_error_reports_its_position() {
    check("fn p(a: Point3) -> Sign { a.x + }", 1, 33, "expected");
    check(
        "fn p(a: Point3) -> Sign {\n    a.x +\n}\n",
        3,
        1,
        "expected",
    );
}

#[test]
fn float_literals_are_rejected() {
    check(
        "fn p(a: Point3) -> Sign {\n    a.x * 1.5\n}\n",
        2,
        11,
        "float",
    );
}

#[test]
fn no_input_makes_the_parser_panic() {
    let deep = format!(
        "fn p(a: Point3) -> Sign {{ {}a.x{} }}",
        "(".repeat(10_000),
        ")".repeat(10_000)
    );
    let sources = [
        "".to_string(),
        "fn".to_string(),
        "fn p(".to_string(),
        "fn p(a: Point3) -> Sign {".to_string(),
        "}".to_string(),
        "\u{1F600}".to_string(),
        "fn p(a: Point3) -> Sign { a.x \u{2713} }".to_string(),
        "fn p(a: Point3) -> Sign { \"caf\u{e9}\" }".to_string(),
        "\u{0}\u{0}\u{0}".to_string(),
        "fn p(a: Point3) -> Sign { a.x + + }".to_string(),
        "fn p(a: Point3) -> Sign { let = 3; a.x }".to_string(),
        "fn p(a: Point3) -> Sign { a. }".to_string(),
        "fn p(a: Point3) -> Sign { a.0 }".to_string(),
        "fn p(a: Point3) -> Sign { 99999999999999999999 * a.x }".to_string(),
        "struct S;".to_string(),
        "#![allow(unused)]\nfn p(a: Point3) -> Sign { a.x }".to_string(),
        "fn p(self) -> Sign { 1 }".to_string(),
        "(".repeat(10_000),
        deep,
        "fn p(a: Point3) -> Sign { let m = a.x; }".to_string(),
    ];
    assert_eq!(sources.len(), 20);
    for source in &sources {
        assert!(parse(source, "t.pred").is_err(), "accepted: {:.60}", source);
    }
}

#[test]
fn long_chains_are_rejected_without_exhausting_the_stack() {
    let n = 100_000;
    let chains = [
        format!("a.x{}", " + a.x".repeat(n)),
        format!("a.x{}", ".x".repeat(n)),
        format!("{}a.x", "-".repeat(n)),
        format!("a.x{}", " as i32".repeat(n)),
        format!("{}a.x", "return ".repeat(n)),
    ];
    for chain in chains {
        let source = format!("fn p(a: Point3) -> Sign {{ {chain} }}");
        assert!(parse(&source, "t.pred").is_err());
    }
    // A moderately long sum is still accepted.
    let sum = vec!["a.x"; 150].join(" + ");
    let source = format!("fn p(a: Point3) -> Sign {{ {sum} }}");
    assert!(parse(&source, "t.pred").is_ok());
}
