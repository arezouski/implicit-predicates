//! Declarations of point types: what is read, and the errors for what is not.

use implicit_predicates_gen::{
    LinearSum, ParamType, PointGraph, PointNode, PointType, parse, parse_points,
};

const POINTS: &str = include_str!("fixtures/points.pred");
const ORIENT3D: &str = include_str!("fixtures/orient3d.pred");

/// Asserts that `source` is rejected at `(line, column)` of `t.pred` with a
/// message containing `fragment`.
fn check(source: &str, (line, column): (usize, usize), fragment: &str) {
    let e = parse_points(source, "t.pred").expect_err("the source must be rejected");
    assert_eq!(e.file, "t.pred");
    assert_eq!((e.line, e.column), (line, column), "{source}: {e}");
    assert!(e.message.contains(fragment), "{source}: {e}");
}

/// Evaluates coordinate `axis` of `point` for the given values: the value of
/// each parameter is a `[f64; 3]`, of which a scalar uses the first entry.
fn evaluate(graph: &PointGraph, axis: usize, values: &[[f64; 3]]) -> f64 {
    let mut v: Vec<f64> = Vec::new();
    for node in &graph.nodes {
        v.push(match *node {
            PointNode::Coord { param, axis } => values[param][usize::from(axis)],
            PointNode::Scalar(param) => values[param][0],
            PointNode::Int(i) => i as f64,
            PointNode::Neg(x) => -v[x],
            PointNode::Add(x, y) => v[x] + v[y],
            PointNode::Sub(x, y) => v[x] - v[y],
            PointNode::Mul(x, y) => v[x] * v[y],
        });
    }
    v[graph.roots[axis]]
}

#[test]
fn the_fixture_has_an_explicit_and_a_linear_point() {
    let points = parse_points(POINTS, "points.pred").unwrap();
    let names: Vec<&str> = points.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["Explicit", "Lnc"]);
    assert!(points[0].is_explicit());
    assert!(!points[1].is_explicit());
    assert_eq!((points[0].position.line, points[0].position.column), (3, 7));
    assert_eq!((points[1].position.line, points[1].position.column), (5, 7));
}

#[test]
fn the_parameters_are_read_with_their_types() {
    let points = parse_points(POINTS, "points.pred").unwrap();
    let lnc = &points[1];
    let params: Vec<(&str, ParamType)> =
        lnc.params.iter().map(|p| (p.name.as_str(), p.ty)).collect();
    assert_eq!(
        params,
        [
            ("p", ParamType::F64x3),
            ("q", ParamType::F64x3),
            ("t", ParamType::F64)
        ]
    );
    assert_eq!(
        (lnc.params[2].position.line, lnc.params[2].position.column),
        (5, 31)
    );
}

#[test]
fn the_coordinates_of_lnc_are_q_plus_t_times_p_minus_q() {
    let lnc = &parse_points(POINTS, "points.pred").unwrap()[1];
    let values = [[1.0, 2.0, 3.0], [5.0, -7.0, 11.0], [0.25, 0.0, 0.0]];
    for axis in 0..3 {
        let (p, q, t) = (values[0][axis], values[1][axis], values[2][0]);
        assert_eq!(evaluate(&lnc.graph, axis, &values), q + t * (p - q));
    }
    // `p - q` is computed once per coordinate and shared by nothing else, but
    // the three coordinates share the scalar `t`.
    let scalars = lnc
        .graph
        .nodes
        .iter()
        .filter(|n| matches!(n, PointNode::Scalar(2)))
        .count();
    assert_eq!(scalars, 1);
}

#[test]
fn the_explicit_point_returns_its_parameter() {
    let explicit = &parse_points(POINTS, "points.pred").unwrap()[0];
    for axis in 0..3 {
        let root = explicit.graph.roots[axis];
        assert_eq!(
            explicit.graph.nodes[root],
            PointNode::Coord {
                param: 0,
                axis: axis as u8
            }
        );
    }
    assert!(explicit.constraints.is_empty());
}

#[test]
fn the_where_clause_is_kept_as_sums() {
    let lnc = &parse_points(POINTS, "points.pred").unwrap()[1];
    let sum = |params: &[usize], constant| LinearSum {
        params: params.to_vec(),
        constant,
    };
    assert_eq!(lnc.constraints.len(), 2);
    assert_eq!(lnc.constraints[0].lhs, sum(&[], 0));
    assert_eq!(lnc.constraints[0].rhs, sum(&[2], 0));
    assert_eq!(lnc.constraints[1].lhs, sum(&[2], 0));
    assert_eq!(lnc.constraints[1].rhs, sum(&[], 1));
    assert_eq!(
        (
            lnc.constraints[0].position.line,
            lnc.constraints[0].position.column
        ),
        (6, 11)
    );

    let source = "point Tpi(p: F64x3, q: F64x3, r: F64x3, u: f64, v: f64) = \
                  p + u*(q - p) + v*(r - p) where 0 <= u, 0 <= v, u + v <= 1;";
    let tpi = &parse_points(source, "t.pred").unwrap()[0];
    assert_eq!(tpi.constraints.len(), 3);
    assert_eq!(tpi.constraints[2].lhs, sum(&[3, 4], 0));
    assert_eq!(tpi.constraints[2].rhs, sum(&[], 1));
}

#[test]
fn points_and_predicates_can_share_a_file() {
    let source = format!("{POINTS}\n{ORIENT3D}\npoint Again(p: F64x3) = p + p;\n");
    let points = parse_points(&source, "t.pred").unwrap();
    let names: Vec<&str> = points.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["Explicit", "Lnc", "Again"]);
    let predicates = parse(&source, "t.pred").unwrap();
    assert_eq!(predicates.len(), 1);
    assert_eq!(predicates[0].name, "orient3d");
    let alone = parse(ORIENT3D, "t.pred").unwrap();
    assert_eq!(predicates[0].main, alone[0].main);
    assert_eq!(predicates[0].exact, alone[0].exact);
}

#[test]
fn a_file_without_points_has_none() {
    assert_eq!(parse_points(ORIENT3D, "t.pred").unwrap(), []);
    assert_eq!(parse_points("", "t.pred").unwrap(), []);
    assert_eq!(parse_points("// nothing\n", "t.pred").unwrap(), []);
}

#[test]
fn a_function_named_point_is_not_a_declaration() {
    let source = "fn point(a: Point3) -> Sign { a.x }\npoint E(p: F64x3) = p;";
    assert_eq!(parse_points(source, "t.pred").unwrap().len(), 1);
    assert_eq!(parse(source, "t.pred").unwrap()[0].name, "point");
}

#[test]
fn a_file_of_points_has_no_predicates_but_is_not_an_error() {
    assert_eq!(parse(POINTS, "t.pred").unwrap(), []);
    let e = parse("", "t.pred").unwrap_err();
    assert!(e.message.contains("no predicate"), "{e}");
}

#[test]
fn explicit_means_one_vector_returned_as_is() {
    let is_explicit = |s: &str| -> bool { parse_points(s, "t.pred").unwrap()[0].is_explicit() };
    assert!(is_explicit("point A(a: F64x3) = a;"));
    assert!(!is_explicit("point A(a: F64x3) = a + a;"));
    assert!(!is_explicit("point A(a: F64x3) = -a;"));
    assert!(!is_explicit("point A(a: F64x3, t: f64) = a * t;"));
    assert!(!is_explicit("point A(a: F64x3) = a where 0 <= 1;"));
}

#[test]
fn arithmetic_is_per_coordinate_with_the_usual_precedence() {
    let source = "point A(a: F64x3, b: F64x3, s: f64, t: f64) = \
                  -a - b * s * 2 + (t + 3) * a - -b;";
    let point: PointType = parse_points(source, "t.pred").unwrap().remove(0);
    let values = [
        [1.5, -2.0, 4.0],
        [0.5, 8.0, -3.0],
        [3.0, 0.0, 0.0],
        [7.0; 3],
    ];
    for axis in 0..3 {
        let (a, b, s, t) = (values[0][axis], values[1][axis], values[2][0], values[3][0]);
        let expected = -a - b * s * 2.0 + (t + 3.0) * a - -b;
        assert_eq!(evaluate(&point.graph, axis, &values), expected);
    }
}

#[test]
fn equal_subexpressions_share_a_node() {
    let source = "point A(a: F64x3, t: f64) = (a * t) + (a * t);";
    let point = parse_points(source, "t.pred").unwrap().remove(0);
    let products = point
        .graph
        .nodes
        .iter()
        .filter(|n| matches!(n, PointNode::Mul(..)))
        .count();
    assert_eq!(products, 3);
    for (axis, &root) in point.graph.roots.iter().enumerate() {
        let PointNode::Add(x, y) = point.graph.nodes[root] else {
            panic!("the root must be a sum");
        };
        assert_eq!(x, y, "axis {axis}");
    }
}

#[test]
fn an_unused_parameter_is_an_error_at_its_name() {
    check(
        "point A(p: F64x3, q: F64x3) = p;",
        (1, 19),
        "unused parameter `q`",
    );
    check(
        "point A(p: F64x3, t: f64) = p\n    where 0 <= t;",
        (1, 19),
        "unused parameter `t`",
    );
}

#[test]
fn an_unknown_type_is_an_error_at_the_type() {
    check("point A(p: Point3) = p;", (1, 12), "`F64x3` or `f64`");
    check("point A(p: f32) = p;", (1, 12), "`F64x3` or `f64`");
    check("point A(p) = p;", (1, 10), "`:`");
    check("point A(p: ) = p;", (1, 12), "type");
}

#[test]
fn a_product_of_two_vectors_is_an_error_at_the_operator() {
    check(
        "point A(p: F64x3, q: F64x3) = p * q;",
        (1, 33),
        "product of two `F64x3`",
    );
    check(
        "point A(p: F64x3, q: F64x3, t: f64) = t * p\n  + p * (q - p);",
        (2, 7),
        "product of two `F64x3`",
    );
}

#[test]
fn a_sum_of_a_vector_and_a_scalar_is_an_error_at_the_operator() {
    check(
        "point A(p: F64x3, t: f64) = p + t;",
        (1, 31),
        "an `F64x3` and an `f64`",
    );
    check(
        "point A(p: F64x3, t: f64) = t - p;",
        (1, 31),
        "an `f64` and an `F64x3`",
    );
    check(
        "point A(p: F64x3, t: f64) = p + 1;",
        (1, 31),
        "an `F64x3` and an `f64`",
    );
}

#[test]
fn the_value_must_be_a_vector() {
    check("point A(t: f64) = t;", (1, 19), "must be an `F64x3`");
    check(
        "point A(p: F64x3, t: f64) =\n    t * 2 + 1;",
        (2, 5),
        "must be an `F64x3`",
    );
}

#[test]
fn a_where_clause_over_a_vector_is_an_error_at_the_name() {
    check(
        "point A(p: F64x3, t: f64) = p * t where p <= 1;",
        (1, 41),
        "`p` is an `F64x3`",
    );
    check(
        "point A(p: F64x3, t: f64) = p * t where 0 <= t, 2 + p <= 3;",
        (1, 53),
        "`p` is an `F64x3`",
    );
}

#[test]
fn a_where_clause_must_be_well_formed() {
    check(
        "point A(p: F64x3, t: f64) = p * t where 0 < t;",
        (1, 43),
        "`<=`",
    );
    check(
        "point A(p: F64x3, t: f64) = p * t where 0 >= t;",
        (1, 43),
        "`<=`",
    );
    check(
        "point A(p: F64x3, t: f64) = p * t where 0 <= u;",
        (1, 46),
        "unknown name `u`",
    );
    check(
        "point A(p: F64x3, t: f64) = p * t where 0 <= t,;",
        (1, 48),
        "expected",
    );
    check(
        "point A(p: F64x3, t: f64) = p * t where ;",
        (1, 41),
        "expected",
    );
    check(
        "point A(p: F64x3, t: f64) = p * t where t;",
        (1, 42),
        "`<=`",
    );
    check(
        "point A(p: F64x3, t: f64) = p * t where 0 <= t - 1;",
        (1, 48),
        "only sums",
    );
    check(
        "point A(p: F64x3, t: f64) = p * t where 0 <= (t);",
        (1, 46),
        "expected",
    );
    check(
        "point A(p: F64x3, t: f64) = p * t where 0 <= 9223372036854775807 + 1;",
        (1, 68),
        "does not fit",
    );
    check(
        "point A(p: F64x3, t: f64) = p * t where 1.5 <= t;",
        (1, 41),
        "float",
    );
}

#[test]
fn two_points_cannot_share_a_name() {
    check(
        "point A(p: F64x3) = p;\n\npoint A(q: F64x3) = q;",
        (3, 7),
        "`A` is already defined",
    );
}

#[test]
fn reserved_words_cannot_name_a_point_or_a_parameter() {
    for word in ["fn", "match", "Self", "r#type", "_", "where", "self"] {
        check(
            &format!("point {word}(p: F64x3) = p;"),
            (1, 7),
            "cannot be used as the name",
        );
    }
    check(
        "point A(match: F64x3) = match;",
        (1, 9),
        "cannot be used as the name",
    );
}

#[test]
fn a_declaration_must_follow_the_grammar() {
    check("point", (1, 1), "a point name");
    check("point A", (1, 7), "`(`");
    check("point A(p: F64x3)", (1, 8), "`=`");
    check("point A(p: F64x3) =", (1, 19), "an expression");
    check("point A(p: F64x3) = p", (1, 21), "`;`");
    check("point A(p: F64x3) = p p;", (1, 23), "`;`");
    check("point A(p: F64x3, p: F64x3) = p;", (1, 19), "redefines");
    check(
        "point A(p: F64x3,, q: f64) = p;",
        (1, 18),
        "a parameter name",
    );
    check("point A(p: F64x3) = p.x;", (1, 22), "whole");
    check("point A(p: F64x3) = z;", (1, 21), "unknown name `z`");
    check("point A(p: F64x3) = p / 2;", (1, 23), "division");
    check("point A(p: F64x3) = 1.5 * p;", (1, 21), "float");
    check("point A() = p;", (1, 13), "unknown name");
}

#[test]
fn nesting_is_limited_to_32_brackets() {
    let nested = |depth: usize| {
        format!(
            "point A(p: F64x3) = {}p{};",
            "(".repeat(depth),
            ")".repeat(depth)
        )
    };
    assert_eq!(parse_points(&nested(32), "t.pred").unwrap().len(), 1);
    check(&nested(33), (1, 20 + 33), "nested more than 32 deep");
}

#[test]
fn parse_reports_an_invalid_point_instead_of_skipping_it() {
    let check_parse = |source: &str, (line, column): (usize, usize), fragment: &str| {
        let e = parse(source, "t.pred").expect_err("the source must be rejected");
        assert_eq!((e.line, e.column), (line, column), "{source}: {e}");
        assert!(e.message.contains(fragment), "{source}: {e}");
    };
    check_parse(
        "point A(p: F64x3) = p * p * fn;",
        (1, 23),
        "product of two `F64x3`",
    );
    check_parse(
        "point fn orient(a: Point3) -> Sign { a.x - a.x } ;",
        (1, 7),
        "cannot be used as the name",
    );
    check_parse(
        "point A(p: F64x3, q: F64x3) = p;\nfn f(a: Point3) -> Sign { a.x }",
        (1, 19),
        "unused parameter `q`",
    );
    check_parse(
        "point A(p: F64x3) = p;\npoint A(p: F64x3) = p;\n",
        (2, 7),
        "already defined",
    );
    check_parse("point A(p: F64x3) = p", (1, 21), "`;`");
}

#[test]
fn parse_points_rejects_what_it_would_otherwise_skip_over_a_point() {
    check("x point A(p: F64x3) = p;", (1, 1), "`point` or `fn`");
    check(
        "fn f(a: Point3) -> Sign { a.x }\n;\npoint A(p: F64x3) = p;",
        (2, 1),
        "`point` or `fn`",
    );
    // A function without a body would swallow the declaration after it.
    check(
        "fn f(a: Point3) -> Sign\npoint A(p: F64x3) = p;",
        (2, 1),
        "`{`",
    );
    check("#[exact]", (1, 2), "`fn`");
    check("#[exact] fn", (1, 10), "a function name");
}

#[test]
fn a_point_followed_by_a_predicate_gives_each_its_items() {
    let source = format!("{POINTS}\n{ORIENT3D}");
    assert_eq!(parse_points(&source, "t.pred").unwrap().len(), 2);
    assert_eq!(parse(&source, "t.pred").unwrap().len(), 1);
    let reversed = format!("{ORIENT3D}\n{POINTS}");
    assert_eq!(parse_points(&reversed, "t.pred").unwrap().len(), 2);
    assert_eq!(parse(&reversed, "t.pred").unwrap().len(), 1);
}

#[test]
fn a_function_without_a_body_cannot_swallow_a_point_before_a_later_body() {
    let source =
        "fn f(a: Point3) -> Sign\npoint A(p: F64x3) = p;\nfn g(a: Point3) -> Sign { a.x }\n";
    check(source, (2, 1), "`{`");
    check("fn f(a: Point3) Sign { a.x }", (1, 17), "`->`");
    check("fn f(a: Point3) -> Bool { a.x }", (1, 20), "`Sign`");
    check("fn f -> Sign { a.x }", (1, 6), "`(`");
}

#[test]
fn a_file_of_only_points_has_points_but_no_predicates() {
    assert_eq!(parse(POINTS, "t.pred").unwrap(), []);
    assert_eq!(parse_points(POINTS, "t.pred").unwrap().len(), 2);
}
