//! Definitions that parse as Rust but cannot be used: doc comments, which
//! Rust reads as attributes, and predicate names that cannot name the
//! module of a generated file.

use indirect_predicates_gen::parse;

/// Asserts that `source` is rejected at `(line, column)` of `t.pred` with a
/// message containing `fragment`.
fn check(source: &str, (line, column): (usize, usize), fragment: &str) {
    let e = parse(source, "t.pred").expect_err("the source must be rejected");
    assert_eq!(e.file, "t.pred");
    assert_eq!((e.line, e.column), (line, column), "{source}: {e}");
    assert!(e.message.contains(fragment), "{source}: {e}");
}

const DOC_COMMENT: &str = "doc comments are not supported in definitions; use `//`";

#[test]
fn doc_comments_are_rejected_at_their_position() {
    for comment in ["/// Outer.", "//! Inner.", "/** Outer. */", "/*! Inner. */"] {
        // Before the function, inside its body, and before a `let`.
        check(
            &format!("{comment}\nfn p(a: Point3) -> Sign {{ a.x }}"),
            (1, 1),
            DOC_COMMENT,
        );
        check(
            &format!("fn p(a: Point3) -> Sign {{\n    {comment}\n    a.x\n}}"),
            (2, 5),
            DOC_COMMENT,
        );
        check(
            &format!("fn p(a: Point3) -> Sign {{\n  let m = a.x;\n  {comment}\n  m\n}}"),
            (3, 3),
            DOC_COMMENT,
        );
    }
}

#[test]
fn ordinary_comments_that_look_like_doc_comments_are_accepted() {
    for comment in [
        "////",
        "//// Four slashes.",
        "/**/",
        "/*** Three stars. */",
        "// /// x",
    ] {
        let source = format!("{comment}\nfn p(a: Point3) -> Sign {{ a.x }}");
        assert!(parse(&source, "t.pred").is_ok(), "{comment}");
    }
}

#[test]
fn doc_comments_inside_literals_and_comments_do_not_count() {
    let source = "/* /// */ // //!\nfn p(a: Point3) -> Sign { a.x }";
    assert!(parse(source, "t.pred").is_ok());
}

const NAME: &str = "cannot be used as the name of a generated module";

#[test]
fn keywords_are_rejected_as_predicate_names() {
    for name in [
        "mod", "fn", "self", "Self", "super", "crate", "type", "match", "async", "dyn", "try",
        "gen", "abstract", "yield", "macro", "_",
    ] {
        check(
            &format!("fn {name}(a: Point3) -> Sign {{ a.x }}"),
            (1, 4),
            NAME,
        );
    }
}

#[test]
fn raw_identifiers_are_rejected_as_predicate_names() {
    check("fn r#mod(a: Point3) -> Sign { a.x }", (1, 4), NAME);
    check("fn r#orient(a: Point3) -> Sign { a.x }", (1, 4), NAME);
}

#[test]
fn ordinary_names_and_weak_keywords_are_accepted() {
    for name in [
        "orient3d", "insphere", "union", "raw", "safe", "module", "mod_2",
    ] {
        let source = format!("fn {name}(a: Point3) -> Sign {{ a.x }}");
        assert!(parse(&source, "t.pred").is_ok(), "{name}");
    }
}
