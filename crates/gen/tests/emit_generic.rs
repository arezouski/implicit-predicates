//! Tests for the chain generated for points of any type, and for the
//! functions of the checks that run a stage on the edge cases of the
//! filter.

use implicit_predicates_gen::{Generated, generate};
use proc_macro2::TokenStream;

const ORIENT3D: &str = include_str!("fixtures/orient3d.pred");
const INSPHERE: &str = include_str!("fixtures/insphere.pred");

/// How the tests say to regenerate the code.
const REGENERATE: &str = "run the tests.";

fn generated(source: &str, file: &str) -> Generated {
    let mut all = generate(source, file, REGENERATE).unwrap();
    assert_eq!(all.len(), 1);
    all.remove(0)
}

/// The text of the function `name` in the unformatted module `code`, from
/// its signature to the closing brace followed by a blank line, or to the
/// end of the module.
fn function<'a>(code: &'a str, name: &str) -> &'a str {
    let start = code
        .find(&format!("pub fn {name}"))
        .unwrap_or_else(|| panic!("no function {name}"));
    let body = &code[start..];
    &body[..body.find("\n}\n\n").unwrap_or(body.len())]
}

/// The positions of `texts` in `code`, which must all be there.
fn positions(code: &str, texts: &[&str]) -> Vec<usize> {
    texts
        .iter()
        .map(|text| code.find(text).unwrap_or_else(|| panic!("{text}: {code}")))
        .collect()
}

#[test]
fn the_generic_chain_takes_one_type_parameter_per_point() {
    for (source, file, generics) in [
        (
            ORIENT3D,
            "predicates/orient3d.pred",
            "<P0: Point, P1: Point, P2: Point, P3: Point>",
        ),
        (
            INSPHERE,
            "predicates/insphere.pred",
            "<P0: Point, P1: Point, P2: Point, P3: Point, P4: Point>",
        ),
    ] {
        let g = generated(source, file);
        g.stages
            .parse::<TokenStream>()
            .unwrap_or_else(|e| panic!("{file}: {e}"));
        for name in ["filter_points", "exact_stages_in", "staged_in"] {
            assert!(
                g.stages.contains(&format!("pub fn {name}{generics}(")),
                "{file}: {name}"
            );
        }
        let points = generics.matches("Point").count();
        let explicit: Vec<String> = (0..points).map(|i| format!("p{i}.is_explicit()")).collect();
        let explicit = explicit.join(" & ");
        assert!(
            function(&g.stages, "filter_points").contains(&explicit),
            "{file}"
        );
        assert!(
            function(&g.stages, "staged_in").contains(&explicit),
            "{file}"
        );
    }
}

#[test]
fn the_filter_on_points_has_no_uncertainty_for_explicit_points() {
    for (source, file, zero, uncertainty) in [
        (
            ORIENT3D,
            "predicates/orient3d.pred",
            "[0.0; 3]",
            "uncertainty(",
        ),
        (
            INSPHERE,
            "predicates/insphere.pred",
            "0.0",
            "uncertainty_max(",
        ),
    ] {
        let g = generated(source, file);
        let f = function(&g.stages, "filter_points");
        let order = positions(
            f,
            &[
                ".is_explicit()",
                &format!("{{\n{zero}\n}} else {{"),
                &format!("implicit_predicates_core::filter::{uncertainty}"),
                "filter(p, e)",
            ],
        );
        assert!(order.is_sorted(), "{file}: {f}");
        // `#[inline(always)]` made the filter of explicit points slower.
        let start = g.stages.find("pub fn filter_points").unwrap();
        assert!(g.stages[..start].ends_with("#[inline]\n"), "{file}");
    }
}

#[test]
fn the_chain_for_points_of_any_type_takes_the_exact_stages_of_staged_for_explicit_points() {
    let o = generated(ORIENT3D, "predicates/orient3d.pred");
    let staged_in = function(&o.stages, "staged_in");
    let order = positions(
        staged_in,
        &[
            "filter_points(p0, p1, p2, p3)",
            "return Ok(exact_stages([",
            "exact_stages_in(scratch, p0, p1, p2, p3)",
        ],
    );
    assert!(order.is_sorted(), "{staged_in}");
    let exact = function(&o.stages, "exact_stages");
    assert!(
        exact.starts_with("pub fn exact_stages(p: [&[f64; 3]; 4]) -> Option<Sign> {"),
        "{exact}"
    );
    let order = positions(
        exact,
        &[
            "exact_differences(p)",
            "expansions_translated(p)",
            "expansions_untranslated(p)",
        ],
    );
    assert!(order.is_sorted(), "{exact}");
    // In-sphere has no stage on the stack: its chain for explicit points
    // takes the arena.
    let i = generated(INSPHERE, "predicates/insphere.pred");
    assert!(function(&i.stages, "staged_in").contains("return exact_stages(scratch, ["));
    let exact = function(&i.stages, "exact_stages");
    assert!(exact.contains("staged_in_arena(scratch, p)"), "{exact}");
}

#[test]
fn the_stages_over_spans_run_on_the_exact_coordinates_in_order_and_reset_the_arena() {
    let o = generated(ORIENT3D, "predicates/orient3d.pred");
    let spans = function(&o.stages, "exact_stages_in");
    let order = positions(
        spans,
        &[
            "fn run<",
            "let Some(q0) = exact(scratch, p0)? else {\nreturn Ok(None);\n};",
            "let Some(q3) = exact(scratch, p3)? else {\nreturn Ok(None);\n};",
            "let q = [&q0, &q1, &q2, &q3];",
            "if let Some(sign) = expansions_translated_in(scratch, q)? {",
            "expansions_untranslated_in(scratch, q)\n}",
            "let mark = scratch.mark();",
            "let result = run(scratch, p0, p1, p2, p3);",
            "scratch.reset(mark);",
        ],
    );
    assert!(order.is_sorted(), "{spans}");
    let start = o.stages.find("pub fn exact_stages_in").unwrap();
    assert!(o.stages[..start].ends_with("#[cold]\n#[inline(never)]\n"));
    // Out of range is `None`; an exhausted arena is an error.
    let exact = &o.stages[o.stages.find("\nfn exact(").unwrap()..];
    assert!(exact.contains("Err(ExactError::OutOfRange) => Ok(None),"));
    assert!(exact.contains("Err(ExactError::Exhausted(e)) => Err(e),"));
    // Without an exact form, only the translated form.
    let i = generated(INSPHERE, "predicates/insphere.pred");
    let spans = function(&i.stages, "exact_stages_in");
    assert!(
        spans.contains("expansions_translated_in(scratch, q)\n}"),
        "{spans}"
    );
    assert!(!spans.contains("untranslated"), "{spans}");
}

#[test]
fn the_checks_hold_a_check_of_each_kind_of_edge_case_for_the_arity_of_the_predicate() {
    let o = generated(ORIENT3D, "predicates/orient3d.pred");
    let c = &o.checks;
    assert!(c.contains("pub fn exact_at(c: &[[Dyadic; 3]; 4]) -> Sign {"));
    assert!(c.contains(
        "pub fn check_bound_edge_cases(mut decide: impl FnMut(&[[f64; 3]; 4]) -> Option<Sign>) \
         -> [usize; 2] {"
    ));
    let uncertainty = function(c, "check_uncertainty_edge_cases");
    assert!(uncertainty.contains("impl FnMut(&[[f64; 3]; 4], &[[f64; 3]; 4]) -> Option<Sign>"));
    // Every corner of the boxes of the twelve coordinates.
    assert!(
        uncertainty.contains("for corner in 0..1u64 << (3 * 4) {"),
        "{uncertainty}"
    );
    assert!(
        uncertainty.contains("assert_eq!(exact_at(&c), sign"),
        "{uncertainty}"
    );
    assert!(
        uncertainty.contains("assert_ne!(sign, Sign::Zero"),
        "{uncertainty}"
    );
    let i = generated(INSPHERE, "predicates/insphere.pred");
    assert!(
        i.checks
            .contains("pub fn exact_at(c: &[[Dyadic; 3]; 5]) -> Sign {")
    );
    assert!(
        i.checks
            .contains("impl FnMut(&[[f64; 3]; 5]) -> Option<Sign>")
    );
    // The global filter has no uncertainty edge cases.
    assert!(!i.checks.contains("check_uncertainty_edge_cases"));
    for g in [&o, &i] {
        g.checks.parse::<TokenStream>().unwrap();
    }
}
