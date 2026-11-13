//! The module of the point types: the error program follows the rule
//! operation by operation, the slack factor is the one derived, the helpers
//! are the documented ones, and the declarations that cannot be emitted are
//! rejected with their position.

mod point_text;

use std::collections::BTreeMap;

use implicit_predicates_gen::{
    PointNode, PointType, error_bound, generate, generate_points, parse_points,
};
use num_bigint::BigUint;
use point_text::{ORIENT3D, POINTS, VARIED, declarations, generated, method, point_method};
use proc_macro2::TokenStream;

/// The terms of the error of each node by the rule, written independently
/// of the generator, as the text that `evaluate` must hold.
fn expected_terms(point: &PointType) -> Vec<Vec<String>> {
    let nodes = &point.graph.nodes;
    let zero = |id: usize| nodes[id] == PointNode::Int(0);
    let mut terms: Vec<Vec<String>> = Vec::new();
    for (id, node) in nodes.iter().enumerate() {
        let has = |x: usize, terms: &[Vec<String>]| !terms[x].is_empty();
        let mut t = Vec::new();
        match *node {
            PointNode::Neg(x) if has(x, &terms) => t.push(format!("e{x}")),
            PointNode::Add(x, y) | PointNode::Sub(x, y) => {
                for o in [x, y] {
                    if has(o, &terms) {
                        t.push(format!("e{o}"));
                    }
                }
                if !zero(x) && !zero(y) {
                    t.push(format!("sum_rounding(v{x}, v{y}, v{id})"));
                }
            }
            PointNode::Mul(x, y) => {
                if has(y, &terms) {
                    t.push(format!("covered_product(v{x}.abs(), e{y})"));
                }
                if has(x, &terms) {
                    t.push(format!("covered_product(v{y}.abs(), e{x})"));
                }
                if has(x, &terms) && has(y, &terms) {
                    t.push(format!("covered_product(e{x}, e{y})"));
                }
                if !zero(x) && !zero(y) {
                    t.push(format!("product_rounding(v{x}, v{y}, v{id})"));
                }
            }
            _ => {}
        }
        terms.push(t);
    }
    terms
}

/// The rounding count of each error of `evaluate`, read from its text: one
/// per addition and multiplication, a covered product being two.
fn rounds_from_text(body: &str) -> BTreeMap<usize, u32> {
    let mut rounds: BTreeMap<usize, u32> = BTreeMap::new();
    for line in body.lines() {
        let Some(rest) = line.strip_prefix("let e") else {
            continue;
        };
        if !rest.starts_with(|c: char| c.is_ascii_digit()) {
            continue;
        }
        let (name, value) = rest.trim_end_matches(';').split_once(" = ").unwrap();
        let of = |e: &str| rounds[&e.strip_prefix('e').unwrap().parse::<usize>().unwrap()];
        let mut total: Option<u32> = None;
        for term in value.split(" + ") {
            let r = if term.starts_with("sum_rounding(") {
                2
            } else if term.starts_with("product_rounding(") {
                3
            } else if let Some(args) = term.strip_prefix("covered_product(") {
                let (a, b) = args.strip_suffix(')').unwrap().split_once(", ").unwrap();
                let a = if a.starts_with('v') { 0 } else { of(a) };
                a + of(b) + 2
            } else {
                of(term)
            };
            total = Some(total.map_or(r, |t| t.max(r) + 1));
        }
        rounds.insert(name.parse().unwrap(), total.unwrap());
    }
    rounds
}

#[test]
fn evaluate_follows_the_rule_and_its_rounds_are_the_derived_ones() {
    for (point, stages, _) in declarations() {
        if point.is_explicit() {
            continue;
        }
        let body = method(&stages, &point.name, "evaluate");
        let expected = expected_terms(&point);
        for (id, terms) in expected.iter().enumerate() {
            let line = body
                .lines()
                .find(|l| l.starts_with(&format!("let e{id} = ")));
            match line {
                None => assert!(terms.is_empty(), "{}: e{id}", point.name),
                Some(line) => assert_eq!(
                    line,
                    format!("let e{id} = {};", terms.join(" + ")),
                    "{}",
                    point.name
                ),
            }
        }
        let rounds = rounds_from_text(body);
        let bound = error_bound(&point, "t.pred").unwrap();
        for (k, &root) in point.graph.roots.iter().enumerate() {
            let error = if expected[root].is_empty() {
                assert_eq!(bound.rounds[k], 0);
                "0.0".to_string()
            } else {
                assert_eq!(bound.rounds[k], rounds[&root], "{} {k}", point.name);
                format!("e{root}")
            };
            let finish = format!("finish(approx[{k}], {error}, Self::ERR_SLACK),");
            assert!(
                body.lines().any(|l| l == finish),
                "{}: {finish}",
                point.name
            );
        }
        let slack = format!("const ERR_SLACK: f64 = {:?};", bound.slack);
        assert!(
            stages.lines().any(|l| l == slack),
            "{}: {slack}",
            point.name
        );
    }
}

/// The smallest `f64` `S = 1 + k 2^-52` with `S (1 - 2^-53)^h >= 1`, found
/// with integers: `(2^52 + k) (2^53 - 1)^h >= 2^52 2^(53 h)`.
fn slack_by_integers(h: u32) -> f64 {
    let power = (BigUint::from(1u8) << 53u32) - 1u32;
    let left = power.pow(h);
    let right = BigUint::from(1u8) << (52 + 53 * h);
    let k = (0u64..)
        .find(|&k| (BigUint::from(1u64 << 52) + k) * &left >= right)
        .unwrap();
    1.0 + k as f64 * f64::EPSILON
}

#[test]
fn the_error_bound_of_lnc_has_six_roundings_and_its_derived_slack_factor() {
    let points = parse_points(POINTS, "t.pred").unwrap();
    let lnc = points.iter().find(|p| p.name == "Lnc").unwrap();
    let bound = error_bound(lnc, "t.pred").unwrap();
    // d = p - q: 2 for its rounding term; m = t d: 2 + 2 for |t| e_d,
    // 3 for its rounding term, 1 to add them; q + m: 2 for its rounding
    // term, 1 to add it.
    assert_eq!(bound.rounds, [6; 3]);
    assert_eq!(bound.slack, slack_by_integers(6));
    // Six roundings need 1 + 4 * 2^-52; nothing smaller is acceptable.
    assert!(bound.slack >= 1.0 + 4.0 * f64::EPSILON, "{}", bound.slack);
    assert_eq!(bound.slack, 1.0 + 4.0 * f64::EPSILON);

    let explicit = points.iter().find(|p| p.name == "Explicit").unwrap();
    let bound = error_bound(explicit, "t.pred").unwrap();
    assert_eq!(bound.rounds, [0; 3]);
    assert_eq!(bound.slack, 1.0);
}

#[test]
fn the_slack_factor_is_the_least_for_its_rounds() {
    for point in parse_points(VARIED, "t.pred").unwrap() {
        let bound = error_bound(&point, "t.pred").unwrap();
        let most = bound.rounds.into_iter().max().unwrap();
        assert_eq!(bound.slack, slack_by_integers(most), "{}", point.name);
    }
}

#[test]
fn the_modules_lex_as_rust_and_are_deterministic() {
    for source in [POINTS, VARIED] {
        let g = generated(source);
        assert_eq!(g.name, "points");
        g.stages.parse::<TokenStream>().unwrap();
        g.checks.parse::<TokenStream>().unwrap();
        assert_eq!(generated(source), g);
        assert!(g.stages.starts_with(
            "// Generated by implicit-predicates-gen from predicates/points.pred. Do not edit.\n"
        ));
    }
}

#[test]
fn the_explicit_type_wraps_its_coordinates_and_has_no_checks() {
    let g = generated("point Explicit(p: F64x3) = p;");
    assert!(
        g.stages
            .contains("#[repr(transparent)]\npub struct Explicit([f64; 3]);")
    );
    assert!(
        g.stages
            .contains("fn err(&self) -> &[f64; 3] {\n&[0.0; 3]\n}")
    );
    assert!(
        g.stages
            .contains("fn is_explicit(&self) -> bool {\ntrue\n}")
    );
    assert!(g.stages.contains(
        "pub enum AnyPoint {\n/// A point of type [`Explicit`].\nExplicit(Explicit),\n}"
    ));
    assert!(g.checks.is_empty());
}

#[test]
fn the_where_clause_is_checked_exactly_one_parameter_at_a_time() {
    let g = generated(POINTS);
    let new = method(&g.stages, "Lnc", "new");
    assert!(
        new.contains("if t < 0.0 {\nreturn Err(PointError::ParamOutOfRange);\n}"),
        "{new}"
    );
    assert!(
        new.contains("if t > 1.0 {\nreturn Err(PointError::ParamOutOfRange);\n}"),
        "{new}"
    );
    // 2 <= t + 1 is t >= 1; 0 <= s is s >= 0.
    let g = generated(VARIED);
    let new = method(&g.stages, "Mix", "new");
    assert!(new.contains("if s < 0.0 {"), "{new}");
    assert!(new.contains("if t < 1.0 {"), "{new}");
    assert!(!method(&g.stages, "Twice", "new").contains("ParamOutOfRange"));
}

fn rejected(source: &str, (line, column): (usize, usize), fragment: &str) {
    let e = generate_points(source, "t.pred", "x").expect_err("the source must be rejected");
    assert_eq!((e.line, e.column), (line, column), "{source}: {e}");
    assert!(e.message.contains(fragment), "{source}: {e}");
}

#[test]
fn declarations_that_cannot_be_emitted_are_rejected_with_their_position() {
    rejected(
        "point A(approx: F64x3) = approx;",
        (1, 9),
        "cannot name a parameter",
    );
    rejected(
        "point A(p: F64x3, new: f64) = p * new;",
        (1, 19),
        "cannot name a parameter",
    );
    // Constants, variants and types of the generated code, and of the
    // prelude, start with an uppercase letter; a parameter cannot.
    for name in [
        "Ok",
        "Err",
        "Some",
        "None",
        "UNIT",
        "TINY",
        "SUM_OPERAND_LIMIT",
        "ERR_SLACK",
        "P",
        "_t",
    ] {
        rejected(
            &format!("point A(p: F64x3, {name}: f64) = p * {name};"),
            (1, 19),
            "must start with a lowercase letter",
        );
    }
    rejected(
        "point AnyPoint(p: F64x3) = p;",
        (1, 7),
        "cannot name a point type",
    );
    rejected(
        "point UNIT(p: F64x3) = p;",
        (1, 7),
        "cannot name a point type",
    );
    // Names of the prelude that the module uses.
    for name in ["Result", "Ok", "Err", "Option", "Some", "None", "From"] {
        rejected(
            &format!("point {name}(p: F64x3, t: f64) = t * p;"),
            (1, 7),
            "cannot name a point type",
        );
    }
    // Primitive types, modules and functions start with a lowercase letter.
    for name in ["f64", "bool", "usize", "core", "summable", "lnc"] {
        rejected(
            &format!("point {name}(p: F64x3, t: f64) = t * p;"),
            (1, 7),
            "must start with an uppercase letter",
        );
    }
    rejected(
        "point A(p: F64x3, u: f64, v: f64) = p * u * v\n    where u + v <= 1;",
        (2, 11),
        "only a comparison of one parameter",
    );
    rejected(
        "point A(p: F64x3, t: f64) = p * t where t <= t + 1;",
        (1, 41),
        "only a comparison of one parameter",
    );
    rejected(
        "point A(p: F64x3, t: f64) = p * t where t + t <= 1;",
        (1, 41),
        "only a comparison of one parameter",
    );
    rejected(
        "point A(p: F64x3, t: f64) = p * t where t <= 9007199254740993;",
        (1, 41),
        "at most 2^53",
    );
    rejected(
        "point A(p: F64x3) = 9007199254740993 * p;",
        (1, 7),
        "not exactly an `f64`",
    );
    rejected(
        "point LncA(p: F64x3) = -p;\npoint Lnc_a(p: F64x3) = --p;",
        (2, 7),
        "exact_lnc_a",
    );
    // Products of expansions square the length of the exact coordinates.
    let deep = "point A(p: F64x3, s: f64, t: f64) = \
                (s - t) * (s - t) * (s - t) * (s - t) * (s - t) * (s - t) * (s - t) * p;";
    rejected(deep, (1, 7), "on the stack");
    // A long chain of roundings.
    let long = format!("point A(p: F64x3) = p{};", " + p".repeat(2000));
    rejected(&long, (1, 7), "roundings");
}

#[test]
fn a_file_of_only_predicates_has_no_point_module_and_one_of_only_points_no_predicates() {
    assert_eq!(generate_points(ORIENT3D, "t.pred", "x").unwrap(), None);
    assert_eq!(generate(POINTS, "t.pred", "x").unwrap(), []);
    assert!(generate_points(POINTS, "t.pred", "x").unwrap().is_some());
}

#[test]
fn a_very_large_declaration_is_rejected_quickly_without_panicking() {
    let source = format!(
        "point A(p: F64x3, t: f64) = p{};",
        " + t * p".repeat(250_000)
    );
    let start = std::time::Instant::now();
    let e = generate_points(&source, "t.pred", "x").unwrap_err();
    assert!(e.message.contains("roundings"), "{e}");
    assert!(start.elapsed().as_secs() < 30);
}

#[test]
fn the_helpers_of_the_error_bound_are_the_documented_ones() {
    let g = generated(POINTS);
    for helper in [
        "fn covered_product(a: f64, b: f64) -> f64 {\n\
         if a == 0.0 || b == 0.0 { 0.0 } else { a * b + TINY }\n}",
        "fn sum_rounding(x: f64, y: f64, s: f64) -> f64 {\n\
         if x != 0.0 && y != 0.0 { covered_product(UNIT, s.abs()) } else { 0.0 }\n}",
        "fn product_rounding(x: f64, y: f64, p: f64) -> f64 {\n\
         if x != 0.0 && y != 0.0 { covered_product(UNIT, p.abs()) + TINY } else { 0.0 }\n}",
        "fn finish(approx: f64, err: f64, slack: f64) -> f64 {\n\
         let bound = err * slack;\n\
         if !(approx.is_finite() && bound.is_finite()) {\nf64::INFINITY\n} \
         else if err == 0.0 {\n0.0\n} else {\nbound.next_up()\n}\n}",
        "const UNIT: f64 = 1.1102230246251565e-16;",
        "const TINY: f64 = 5e-324;",
        "const SUM_OPERAND_LIMIT: f64 = 1.1235582092889474e307;",
    ] {
        assert!(g.stages.contains(helper), "{helper}");
    }
    assert_eq!(1.1102230246251565e-16, 2f64.powi(-53));
    assert_eq!(5e-324, f64::from_bits(1));
    assert_eq!(1.1235582092889474e307, 2f64.powi(1020));
}

#[test]
fn parameter_names_must_be_snake_case() {
    // Names that the emitted code would have to allow `non_snake_case` for.
    for (name, column) in [("tA", 19), ("t__a", 19), ("tAb", 19)] {
        rejected(
            &format!("point A(p: F64x3, {name}: f64) = p * {name};"),
            (1, column),
            "snake case",
        );
    }
    for name in ["t_a", "t1", "t__", "t_1_"] {
        let source = format!("point A(p: F64x3, {name}: f64) = p * {name};");
        assert!(generate_points(&source, "t.pred", "x").is_ok(), "{name}");
    }
}

#[test]
fn a_type_whose_exact_coordinates_can_be_too_long_for_the_stages_on_expansions_gets_a_warning() {
    let warnings = |source: &str| {
        generate_points(source, "t.pred", "run the generator")
            .unwrap()
            .expect("the source declares points")
            .warnings
    };
    // 2 components for `p - q`, doubled by each of the three scalings, plus
    // one for `q`: 17, one more than the stages on expansions accept.
    let cube = warnings("point Cube(p: F64x3, q: F64x3, s: f64) = q + s*s*s*(p - q);");
    assert_eq!(cube.len(), 1, "{cube:?}");
    for part in [
        "`Cube`",
        "more than 16",
        "x up to 17, y up to 17, z up to 17",
        "the stage on big integers, which is slower, decides it",
    ] {
        assert!(cube[0].contains(part), "{cube:?}");
    }
    // One scaling fewer: 9 components.
    let square = warnings("point Square(p: F64x3, q: F64x3, s: f64) = q + s*s*(p - q);");
    assert_eq!(square, Vec::<String>::new());
    assert_eq!(warnings(POINTS), Vec::<String>::new());
    // Of the varied types, only the one with a product of expansions.
    let varied = warnings(VARIED);
    assert_eq!(varied.len(), 1, "{varied:?}");
    assert!(varied[0].contains("`Products`"), "{varied:?}");
}

#[test]
fn the_error_of_the_constructors_can_grow() {
    let module = generated(POINTS).stages;
    assert!(
        module.contains("#[non_exhaustive]\npub enum PointError {"),
        "{module}"
    );
}

#[test]
fn every_type_gives_its_big_coordinates_and_checks_finiteness_before_the_arena() {
    for source in [POINTS, VARIED] {
        let g = generated(source);
        for point in parse_points(source, "t.pred").unwrap() {
            let name = &point.name;
            let in_trait = point_method(&g.stages, name, "exact_big");
            assert!(in_trait.starts_with("fn exact_big("), "{name}");
            if point.is_explicit() {
                continue;
            }
            assert!(in_trait.contains("Self::write_exact_big("), "{name}");
            let body = method(&g.stages, name, "write_exact_big");
            let check = body
                .find("if !(c.iter().flatten().all(|x| x.is_finite())")
                .unwrap_or_else(|| panic!("{name}: no finiteness check\n{body}"));
            assert!(
                body[check..].contains("return Err(ExactError::OutOfRange);"),
                "{name}"
            );
            let first = body.find("scratch.").expect("an arena operation");
            assert!(
                check < first,
                "{name}: the check follows an arena operation"
            );
        }
        let any = point_method(&g.stages, "AnyPoint", "exact_big");
        for point in parse_points(source, "t.pred").unwrap() {
            assert!(
                any.contains(&format!(
                    "AnyPoint::{0}(p) => p.exact_big(scratch),",
                    point.name
                )),
                "{}",
                point.name
            );
        }
    }
}
