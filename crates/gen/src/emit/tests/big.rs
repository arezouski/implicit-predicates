//! The stage on big integers has the value of the source polynomial, uses
//! only the big-integer operations of the arena and leaves the arena as it
//! was; the size of its arena for explicit points is emitted.

use super::readback::{function, interpret_big_stage};
use super::spans::doubled;
use super::{INSPHERE, NEGATIONS, ORIENT3D, expand};
use crate::parse::parse;

/// The code generated for every predicate of `source`.
fn generated(source: &str) -> Vec<super::super::Generated> {
    super::super::generate(source, "t.pred", "run the tests.").unwrap()
}

/// The value of the constant `BIG_INTEGERS_EXPLICIT_ARENA` in `stages`.
fn arena(stages: &str) -> usize {
    stages
        .lines()
        .find_map(|l| l.strip_prefix("pub const BIG_INTEGERS_EXPLICIT_ARENA: usize = "))
        .and_then(|v| v.strip_suffix(';'))
        .expect("the constant of the arena")
        .parse()
        .unwrap()
}

#[test]
fn the_text_of_the_stage_on_big_integers_has_the_value_of_the_source_polynomial() {
    let mut checked = 0;
    for source in [ORIENT3D, INSPHERE, NEGATIONS] {
        for (p, g) in parse(source, "t.pred")
            .unwrap()
            .iter()
            .zip(generated(source))
        {
            let code = function(&g.stages, "big_integers_in");
            // The readback takes coordinate `k` of point `i` as `c[i][k]`,
            // so it does not see how `c` is built from the points.
            assert!(
                code.contains("\nlet c = p.map(|q| [q.x, q.y, q.z]);\n"),
                "{}: {code}",
                p.name
            );
            assert_eq!(
                expand(&interpret_big_stage(code)),
                expand(&p.main),
                "{}",
                p.name
            );
            checked += 1;
        }
    }
    // orient3d, in-sphere, n and linear.
    assert_eq!(checked, 4);
}

#[test]
fn the_stage_on_big_integers_uses_only_the_big_operations_of_the_arena() {
    for source in [ORIENT3D, INSPHERE, NEGATIONS] {
        for g in generated(source) {
            let code = function(&g.stages, "big_integers_in");
            for forbidden in [
                "expansion::",
                "eft::",
                "[0.0;",
                "scratch.sum(",
                "scratch.product(",
                "scratch.diff(",
                "scratch.push(",
            ] {
                assert!(!code.contains(forbidden), "{} {forbidden}: {code}", g.name);
            }
            assert!(code.contains("scratch.big_"), "{}: {code}", g.name);
        }
    }
}

#[test]
fn the_stage_resets_the_arena_on_every_path() {
    for source in [ORIENT3D, INSPHERE, NEGATIONS] {
        for g in generated(source) {
            let code = function(&g.stages, "big_integers_in");
            // One mark, taken outside the nested evaluation, so that every
            // `?` inside it returns to the reset.
            assert_eq!(code.matches("scratch.mark()").count(), 1, "{}", g.name);
            assert_eq!(code.matches("scratch.reset(").count(), 1, "{}", g.name);
            assert_eq!(code.matches("fn evaluate(").count(), 1, "{}", g.name);
            assert!(
                !code.lines().any(|l| l.trim_start().starts_with("return")),
                "{}: {code}",
                g.name
            );
            let evaluate_end = code.find("\n}\n").unwrap();
            let mark = code.find("let mark = scratch.mark();").unwrap();
            let call = code.find("let sign = evaluate(scratch, c);").unwrap();
            let reset = code.find("scratch.reset(mark);").unwrap();
            assert!(
                evaluate_end < mark && mark < call && call < reset,
                "{}",
                g.name
            );
            assert!(
                code.ends_with("scratch.reset(mark);\nsign"),
                "{}: {code}",
                g.name
            );
            // The `?` of the operations are all inside the nested function.
            assert!(!code[evaluate_end..].contains('?'), "{}", g.name);
        }
    }
}

#[test]
fn the_stage_on_big_integers_is_cold_and_takes_the_big_coordinates() {
    for (source, points) in [(ORIENT3D, 4), (INSPHERE, 5)] {
        let g = &generated(source)[0];
        let signature = format!(
            "#[cold]\n#[inline(never)]\npub fn big_integers_in(scratch: &mut Scratch<'_>, \
             p: [&BigCoords; {points}]) -> Result<Sign, ScratchExhausted> {{\n"
        );
        assert!(g.stages.contains(&signature), "{}", g.name);
        assert!(g.stages.contains(", BigCoords, "), "{}", g.name);
    }
}

#[test]
fn the_arena_of_the_explicit_stage_is_emitted() {
    let orient3d = arena(&generated(ORIENT3D)[0].stages);
    // It fits in the arena that a thread starts with.
    assert!((500..=16_384).contains(&orient3d), "{orient3d}");
    let insphere = arena(&generated(INSPHERE)[0].stages);
    assert!(insphere > orient3d, "{insphere}");
    for source in [ORIENT3D, INSPHERE, NEGATIONS] {
        for g in generated(source) {
            let at = g
                .stages
                .find("pub const BIG_INTEGERS_EXPLICIT_ARENA: usize")
                .unwrap();
            let doc_start = g.stages[..at].rfind("\n\n").unwrap();
            let doc: String = g.stages[doc_start..at]
                .lines()
                .filter_map(|l| l.strip_prefix("///"))
                .collect::<Vec<_>>()
                .join("");
            let doc = doc.split_whitespace().collect::<Vec<_>>().join(" ");
            assert!(
                doc.contains("`f64` slots of the arena, one 64-bit word each"),
                "{}: {doc}",
                g.name
            );
        }
    }
}

#[test]
fn huge_temporaries_do_not_make_the_generator_panic() {
    // A form whose temporaries over expansions double forty times: the
    // stage on big integers either is emitted or the definition is an
    // error with a position. (Today the plan over spans rejects it first.)
    let source = doubled(40);
    match super::super::generate(&source, "t.pred", "run the tests.") {
        Ok(all) => {
            let stages = &all[0].stages;
            assert!(stages.contains("pub fn big_integers_in("));
            assert!(arena(stages) > 0);
        }
        Err(e) => {
            assert_eq!(e.file, "t.pred");
            assert!(e.line > 0 && e.column > 0, "{e}");
        }
    }
    // Thirty-three doublings are still generated: each one adds a bit to
    // the bound of the coefficients, and a word to the arena only now and
    // then.
    let all = generated(&doubled(33));
    let stages = &all[0].stages;
    assert!(stages.contains("pub fn big_integers_in("));
    assert!((500..=16_384).contains(&arena(stages)), "{}", arena(stages));
}
