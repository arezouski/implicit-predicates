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

/// Runs `parse` on a thread with a 1 MiB stack, half of the default one, and
/// returns its result. A stack overflow aborts the whole test run.
fn on_small_stack(source: String) -> Result<usize, Error> {
    std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(move || parse(&source, "t.pred").map(|p| p.len()))
        .unwrap()
        .join()
        .unwrap()
}

/// Wraps `body` as the body of a one-parameter predicate.
fn wrap(body: &str) -> String {
    format!("fn p(a: Point3) -> Sign {{ {body} }}")
}

fn assert_rejected(source: String, fragment: &str) {
    let e = on_small_stack(source).expect_err("must be rejected");
    assert!(e.message.contains(fragment), "{e}");
}

const N: usize = 100_000;

#[test]
fn constructs_outside_the_grammar_are_rejected_at_any_length() {
    let cases = [
        wrap(&format!("a{}", "(1)".repeat(N))),
        wrap(&format!("a{}", "[1]".repeat(N))),
        format!("{}fn p(a: Point3) -> Sign {{ a.x }}", "#[x]".repeat(N)),
        format!("{}fn p(a: Point3) -> Sign {{ a.x }}", "#[exact]".repeat(N)),
        wrap(&format!("{}a.x", "let ".repeat(N))),
        wrap(&"{}".repeat(N)),
        wrap(&format!("a{}", "::b".repeat(N))),
        wrap(&format!("a.x{}", ".x".repeat(N))),
        wrap(&format!("a.x{}", " as i32".repeat(N))),
        wrap(&format!("{}a.x", "return ".repeat(N))),
        wrap(&format!("{}a.x", "!".repeat(N))),
        wrap(&format!("a.x{}", "?".repeat(N))),
        wrap(&format!("a.x{}", ".f()".repeat(N))),
        wrap(&format!("({})", "(".repeat(N))),
        format!("{}fn p(a: Point3) -> Sign {{ a.x }}", "fn ".repeat(N)),
        wrap(&format!("a.x{}", " = a.x".repeat(N))),
        wrap(&"-> ".repeat(N)),
        wrap(&format!("a.x{}", ", a.x".repeat(N))),
        wrap(&format!("a.x{}", "; a.x".repeat(N))),
        "(".repeat(N),
        "}".repeat(N),
        "<".repeat(N),
    ];
    for source in cases {
        assert!(on_small_stack(source).is_err());
    }
}

#[test]
fn a_hundred_thousand_minus_signs_and_terms_are_accepted() {
    let signs = wrap(&format!("{}a.x", "-".repeat(N)));
    assert_eq!(on_small_stack(signs).unwrap(), 1);
    let sum = wrap(&format!("a.x{}", " + a.y".repeat(N)));
    assert_eq!(on_small_stack(sum).unwrap(), 1);
    let product = wrap(&format!("a.x{}", " * a.y - a.z".repeat(N)));
    assert_eq!(on_small_stack(product).unwrap(), 1);
}

#[test]
fn nesting_is_accepted_up_to_the_limit_and_rejected_beyond_it() {
    // The braces of the body count: 31 parentheses reach 32 levels.
    let at_the_limit = wrap(&format!("{}a.x{}", "(".repeat(31), ")".repeat(31)));
    assert_eq!(on_small_stack(at_the_limit).unwrap(), 1);
    let worst = wrap(&format!("{}a.x{}", "- -(".repeat(31), ")".repeat(31)));
    assert_eq!(on_small_stack(worst).unwrap(), 1);
    let worst = wrap(&format!(
        "a.x{}{}",
        " * (-a.x + a.y".repeat(31),
        ")".repeat(31)
    ));
    assert_eq!(on_small_stack(worst).unwrap(), 1);
    let beyond = wrap(&format!("{}a.x{}", "(".repeat(32), ")".repeat(32)));
    assert_rejected(
        beyond,
        "nested more than 32 deep; split the expression with `let`",
    );
    // The same, with the brackets only in the token stream and not in a body.
    let beyond = format!("{}{}", "(".repeat(33), ")".repeat(33));
    assert_rejected(beyond, "nested more than 32 deep");
}

#[test]
fn comments_between_tokens_cost_nothing() {
    let hidden = ["/*;*/", "/* /* ; */ ; */", "//;\n", "/*(((((*/"];
    for h in hidden {
        let source = wrap(&format!("a.x{}", format!(" +{h} a.y").repeat(N)));
        assert_eq!(on_small_stack(source).unwrap(), 1, "{h}");
    }
}

#[test]
fn literals_with_hidden_punctuation_are_literals() {
    let hidden = [
        "\"a;\"", "'a'", "';'", "'\\''", "r#\";\"#", "br\"x;\"", "\"(((\"",
    ];
    for h in hidden {
        let source = wrap(&format!("a.x + {h}"));
        assert_rejected(source, "only integer literals");
    }
}

/// Small deterministic generator for the token soups.
struct Lcg(u64);

impl Lcg {
    fn below(&mut self, n: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) as usize) % n
    }
}

#[test]
fn token_soups_never_panic() {
    const TOKENS: &[&str] = &[
        "fn",
        "let",
        "p",
        "a",
        "b",
        "x",
        "y",
        "z",
        "Point3",
        "Sign",
        "exact",
        "(",
        ")",
        "{",
        "}",
        "[",
        "]",
        "#",
        ";",
        ":",
        ",",
        "=",
        "+",
        "-",
        "*",
        "/",
        ".",
        "->",
        "1",
        "2",
        "1.5",
        "1e3",
        "0x1F",
        "99999999999999999999",
        "3u8",
        "'a",
        "'",
        "\"s\"",
        "r#\"x\"#",
        "!",
        "&",
        "|",
        "<",
        ">",
        "?",
        "::",
        "..",
        "return",
        "as",
        "if",
        "pub",
        "self",
        "\u{e9}",
        "\u{1F600}",
        "/*",
        "*/",
        "//",
        "\n",
        "\\",
        "$",
        "@",
        "~",
        "^",
        "%",
    ];
    let mut rng = Lcg(0x1234_5678_9abc_def0);
    let mut sources = Vec::new();
    for _ in 0..6000 {
        let len = 1 + rng.below(40);
        let soup: Vec<&str> = (0..len).map(|_| TOKENS[rng.below(TOKENS.len())]).collect();
        let soup = soup.join(" ");
        sources.push(match rng.below(3) {
            0 => soup,
            1 => format!("fn p(a: Point3, b: Point3) -> Sign {{ {soup} }}"),
            _ => format!("fn p(a: Point3) -> Sign {{ let m = a.x; {soup} }}"),
        });
    }
    std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(move || {
            for source in &sources {
                let _ = parse(source, "t.pred");
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn brackets_hidden_in_comments_and_literals_do_not_count_but_real_ones_do() {
    let n = 100_000;
    let comments = format!(
        "/* {} */ // {}\n/* /* {} */ {} */\n",
        "(".repeat(n),
        "{".repeat(n),
        "[".repeat(n),
        "(".repeat(n)
    );
    let source = format!("{comments}fn p(a: Point3) -> Sign {{ a.x }}");
    assert!(parse(&source, "t.pred").is_ok());
    // Literals full of brackets are not definitions, but they are not too deep.
    let literals = format!(
        "\"{}\" '(' r#\"{}\"# b\"{}\" '\\'' \"\\\"{}\"",
        "(".repeat(n),
        "[".repeat(n),
        "{".repeat(n),
        "(".repeat(n)
    );
    let e = rejected(&literals);
    assert!(!e.message.contains("nested too deeply"), "{e}");
    // Real brackets after a comment that hides some, with nested block comments.
    let source = format!(
        "/* /* ( */ ( */ fn p(a: Point3) -> Sign {{ {}a.x{} }}",
        "(".repeat(n),
        ")".repeat(n)
    );
    let e = rejected(&source);
    assert!(e.message.contains("nested more than 32 deep"), "{e}");
    for source in ["(".repeat(n), "[".repeat(n), "{".repeat(n)] {
        assert!(parse(&source, "t.pred").is_err());
    }
    // An unterminated comment or literal is an error, not a crash.
    for source in ["/* /* */", "\"abc\\", "'\\", "r##\"abc\"#", "fn p() { 'a"] {
        assert!(parse(source, "t.pred").is_err(), "{source}");
    }
}

/// A definition with the shape of an in-sphere predicate: five points, rows of
/// differences against the last one with a lifted column, 2x2 minors bound
/// with `let`, and a final cofactor expression.
fn insphere_like() -> String {
    let rows = ["a", "b", "c", "d"];
    let cols = ["x", "y", "z", "w"];
    let mut src = String::from(
        "fn insphere(a: Point3, b: Point3, c: Point3, d: Point3, e: Point3) -> Sign {\n",
    );
    for r in rows {
        for c in ["x", "y", "z"] {
            src += &format!("    let {r}{c} = {r}.{c} - e.{c};\n");
        }
        src += &format!("    let {r}w = {r}x*{r}x + {r}y*{r}y + {r}z*{r}z;\n");
    }
    let pairs = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
    for (top, bottom) in [(0, 1), (2, 3)] {
        for (i, j) in pairs {
            let (r, s) = (rows[top], rows[bottom]);
            let (ci, cj) = (cols[i], cols[j]);
            src += &format!("    let m{top}{ci}{cj} = {r}{ci}*{s}{cj} - {r}{cj}*{s}{ci};\n");
        }
    }
    src += "    m0xy*m2zw - m0xz*m2yw + m0xw*m2yz + m0yz*m2xw - m0yw*m2xz + m0zw*m2xy\n}\n";
    src
}

#[test]
fn an_insphere_shaped_definition_is_accepted() {
    let preds = parse(&insphere_like(), "insphere.pred").unwrap();
    assert_eq!(preds[0].params.len(), 5);
    assert_eq!(
        count(&preds[0].main, |n| matches!(n, Node::Coord { .. })),
        15
    );
}
