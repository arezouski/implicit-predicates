//! Tests for the chain generated for points of any type, and for the
//! functions of the checks that run a stage on the edge cases of the
//! filter.

use implicit_predicates_gen::{Generated, generate};
use proc_macro2::TokenStream;

const ORIENT3D: &str = include_str!("fixtures/orient3d.pred");
const INSPHERE: &str = include_str!("fixtures/insphere.pred");
const INCIRCLE: &str = include_str!("fixtures/incircle.pred");

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
            "return exact_stages_explicit_in(scratch, [",
            "exact_stages_in(scratch, p0, p1, p2, p3)",
        ],
    );
    assert!(order.is_sorted(), "{staged_in}");
    // Then the stage on big integers, on the coordinates of the points.
    let explicit = function(&o.stages, "exact_stages_explicit_in");
    assert!(
        explicit.starts_with(
            "pub fn exact_stages_explicit_in(scratch: &mut Scratch<'_>, p: [&[f64; 3]; 4]) \
             -> Result<Sign, ScratchExhausted> {"
        ),
        "{explicit}"
    );
    let order = positions(
        explicit,
        &[
            "fn big(",
            "let q0 = exact_big(scratch, 0, p[0])?;",
            "let q3 = exact_big(scratch, 3, p[3])?;",
            "big_integers_in(scratch, [&q0, &q1, &q2, &q3])\n}",
            "if let Some(sign) = exact_stages(p) {\nreturn Ok(sign);\n}",
            "let mark = scratch.mark();",
            "let result = big(scratch, p);",
            "scratch.reset(mark);\nresult",
        ],
    );
    assert!(order.is_sorted(), "{explicit}");
    let start = o.stages.find("pub fn exact_stages_explicit_in").unwrap();
    assert!(o.stages[..start].ends_with("#[cold]\n#[inline(never)]\n"));
    let exact = function(&o.stages, "exact_stages");
    assert!(
        exact.starts_with("pub fn exact_stages(p: [&[f64; 3]; 4]) -> Option<Sign> {"),
        "{exact}"
    );
    let order = positions(exact, &["exact_differences(p)", "expansions_translated(p)"]);
    assert!(order.is_sorted(), "{exact}");
    // The translated expansions of in-circle do not fit on the stack: its
    // chain for explicit points takes the arena.
    let i = generated(INCIRCLE, "t.pred");
    assert!(
        function(&i.stages, "staged_in").contains("return exact_stages_explicit_in(scratch, [")
    );
    let exact = function(&i.stages, "exact_stages");
    let order = positions(
        exact,
        &[
            "filter_axes(p, [0.0; 3])",
            "exact_differences(p)",
            "staged_in_arena(scratch, p)",
        ],
    );
    assert!(order.is_sorted(), "{exact}");
    let explicit = function(&i.stages, "exact_stages_explicit_in");
    let order = positions(
        explicit,
        &[
            "let q3 = exact_big(scratch, 3, p[3])?;",
            "big_integers_in(scratch, [&q0, &q1, &q2, &q3])",
            "if let Some(sign) = exact_stages(scratch, p)? {\nreturn Ok(sign);\n}",
        ],
    );
    assert!(order.is_sorted(), "{explicit}");
    // In-sphere has no exact stage but the one on big integers, and no
    // `exact_stages`: only the second test of its filter comes before it.
    let s = generated(INSPHERE, "predicates/insphere.pred");
    assert!(!s.stages.contains("exact_stages("));
    let explicit = function(&s.stages, "exact_stages_explicit_in");
    assert_eq!(explicit.matches("if let Some").count(), 1, "{explicit}");
    assert!(explicit.contains("if let Some(sign) = filter_axes(p, [0.0; 3]) {"));
}

#[test]
fn the_stages_over_spans_run_on_the_exact_coordinates_in_order_and_reset_the_arena() {
    let o = generated(ORIENT3D, "predicates/orient3d.pred");
    let spans = function(&o.stages, "exact_stages_in");
    let order = positions(
        spans,
        &[
            "fn spans<",
            "let q0 = exact(scratch, 0, p0)?;",
            "let q3 = exact(scratch, 3, p3)?;",
            "let (Some(q0), Some(q1), Some(q2), Some(q3)) = (q0, q1, q2, q3) else {\n\
             return Ok(None);\n};",
            "let q = [&q0, &q1, &q2, &q3];",
            "expansions_translated_in(scratch, q)\n}",
            "let mark = scratch.mark();",
            "let decided = spans(scratch, p0, p1, p2, p3);",
            "scratch.reset(mark);",
        ],
    );
    assert!(order.is_sorted(), "{spans}");
    let start = o.stages.find("pub fn exact_stages_in").unwrap();
    assert!(o.stages[..start].ends_with("#[cold]\n#[inline(never)]\n"));
    // An exhausted arena is an error; a weight other than one is a panic;
    // any other failure, such as inputs out of range, is `None`, and leaves
    // the stage on big integers to decide.
    let exact = &o.stages[o.stages.find("\nfn exact(").unwrap()..];
    assert!(
        exact.contains(
            "Ok(c) if c.w != Weight::One => no_exact_coordinates(\n\
         \"implicit-predicates: a point has a weight other than one, which the predicates do \
         not support\",\n),\nOk(c) => {\ndebug_check_expansions(scratch, position, &c);\n\
         Ok(Some(c))\n}\nErr(ExactError::Exhausted(e)) => Err(e),\nErr(_) => Ok(None),"
        ),
        "{exact}"
    );
    // In-circle runs the same stage over spans.
    let i = generated(INCIRCLE, "t.pred");
    let spans = function(&i.stages, "exact_stages_in");
    assert!(
        spans.contains("expansions_translated_in(scratch, q)\n}"),
        "{spans}"
    );
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

#[test]
fn the_stages_for_points_of_any_type_end_with_the_stage_on_big_integers() {
    for (source, file, points) in [
        (ORIENT3D, "predicates/orient3d.pred", 4),
        (INCIRCLE, "t.pred", 4),
    ] {
        let g = generated(source, file);
        for name in ["exact_stages_explicit_in", "exact_stages_in", "staged_in"] {
            let f = function(&g.stages, name);
            let signature = &f[..f.find('{').unwrap()];
            assert!(
                signature.ends_with(") -> Result<Sign, ScratchExhausted> "),
                "{file}: {signature}"
            );
        }
        // The coordinates as big integers are asked for only after the
        // stages over spans, from the mark taken on entry.
        let f = function(&g.stages, "exact_stages_in");
        let last = format!("q{}", points - 1);
        let refs: Vec<String> = (0..points).map(|i| format!("&q{i}")).collect();
        let order = positions(
            f,
            &[
                "fn spans<",
                "let q = [&q0, ",
                "expansions_translated_in(scratch, q)",
                "fn big<",
                "let q0 = exact_big(scratch, 0, p0)?;",
                &format!("let {last} = exact_big(scratch, {0}, p{0})?;", points - 1),
                &format!("big_integers_in(scratch, [{}])", refs.join(", ")),
                "let mark = scratch.mark();",
                "let decided = spans(scratch, ",
                "scratch.reset(mark);\nif let Some(sign) = decided? {\nreturn Ok(sign);\n}",
                "let result = big(scratch, ",
                "scratch.reset(mark);\nresult",
            ],
        );
        assert!(order.is_sorted(), "{file}: {f}");
        let first_big = f.find("exact_big").unwrap();
        assert!(
            f[first_big..].find("_in(scratch, q)").is_none(),
            "{file}: {f}"
        );
        assert_eq!(f.matches("scratch.mark()").count(), 1, "{file}: {f}");
        assert_eq!(f.matches("scratch.reset(mark)").count(), 2, "{file}: {f}");
        // A point without exact coordinates as big integers is a panic.
        let exact_big = &g.stages[g.stages.find("\nfn exact_big(").unwrap()..];
        assert!(
            exact_big.contains(
                "Ok(c) => {\ndebug_check_big(scratch, position, &c);\nOk(c)\n}\n\
             Err(ExactError::Exhausted(e)) => Err(e),\n\
             Err(ExactError::Weighted) => no_exact_coordinates(\n\
             \"implicit-predicates: a point has a weight other than one, which the predicates do \
             not support\",\n),\n\
             Err(_) => no_exact_coordinates(\n\
             \"implicit-predicates: a point has no exact coordinates: an input is not finite\",\n),"
            ),
            "{file}: {exact_big}"
        );
        // The weight is reported by `exact` and by `exact_big`.
        for (message, count) in [
            (
                "\"implicit-predicates: a point has a weight other than one, which the predicates \
                 do not support\"",
                2,
            ),
            (
                "\"implicit-predicates: a point has no exact coordinates: an input is not finite\"",
                1,
            ),
        ] {
            assert_eq!(
                g.stages.matches(message).count(),
                count,
                "{file}: {message}"
            );
        }
        assert!(
            g.stages.contains(
                "#[cold]\n#[inline(never)]\nfn no_exact_coordinates(reason: &str) -> ! {\n\
                 panic!(\"{reason}\")\n}"
            ),
            "{file}"
        );
    }
}

#[test]
fn a_chain_on_big_integers_alone_reports_the_weight_from_the_big_integers() {
    // In-sphere has no stage on expansions, so no helper `exact`: the weight
    // other than one is the panic of `exact_big`, with the same message.
    let g = generated(INSPHERE, "predicates/insphere.pred");
    assert!(!g.stages.contains("\nfn exact("));
    assert!(!g.stages.contains(".exact(scratch)"));
    let message = "\"implicit-predicates: a point has a weight other than one, which the predicates \
                   do not support\"";
    assert_eq!(g.stages.matches(message).count(), 1);
    let exact_big = &g.stages[g.stages.find("\nfn exact_big(").unwrap()..];
    assert!(
        exact_big.contains(&format!(
            "Err(ExactError::Weighted) => no_exact_coordinates(\n{message},\n),"
        )),
        "{exact_big}"
    );
}

/// The text of the private helper `name` in the unformatted module `code`,
/// from its signature to the closing brace followed by a blank line.
fn helper<'a>(code: &'a str, name: &str) -> &'a str {
    let start = code
        .find(&format!("\nfn {name}("))
        .unwrap_or_else(|| panic!("no helper {name}"));
    let body = &code[start..];
    &body[..body.find("\n}\n\n").unwrap_or(body.len())]
}

#[test]
fn the_chain_checks_the_contract_of_the_points_in_debug_builds_only() {
    for (source, file, points, axes, spans) in [
        (ORIENT3D, "predicates/orient3d.pred", 4, false, true),
        (INSPHERE, "predicates/insphere.pred", 5, true, false),
        (INCIRCLE, "t.pred", 4, true, true),
    ] {
        let g = generated(source, file);
        // Each point is checked, with its position, before the filter reads
        // it.
        let mut filters = vec!["filter_points"];
        if axes {
            filters.push("filter_axes_points");
        }
        for name in filters {
            let f = function(&g.stages, name);
            let checks: Vec<String> = (0..points)
                .map(|i| format!("debug_check_err({i}, p{i});\n"))
                .collect();
            let mut texts: Vec<&str> = checks.iter().map(String::as_str).collect();
            texts.push("let p = [p0.approx()");
            let order = positions(f, &texts);
            assert!(order.is_sorted(), "{file}: {f}");
        }
        // The coordinates are checked as soon as a point gives them.
        let exact_big = helper(&g.stages, "exact_big");
        assert!(
            exact_big.contains("Ok(c) => {\ndebug_check_big(scratch, position, &c);\nOk(c)\n}"),
            "{file}: {exact_big}"
        );
        let explicit = function(&g.stages, "exact_stages_explicit_in");
        let generic = function(&g.stages, "exact_stages_in");
        for i in 0..points {
            let q = format!("let q{i} = exact_big(scratch, {i}, p[{i}])?;");
            assert!(explicit.contains(&q), "{file}: {explicit}");
            let q = format!("let q{i} = exact_big(scratch, {i}, p{i})?;");
            assert!(generic.contains(&q), "{file}: {generic}");
            let q = format!("let q{i} = exact(scratch, {i}, p{i})?;");
            assert_eq!(generic.contains(&q), spans, "{file}: {generic}");
        }
        assert_eq!(g.stages.contains("\nfn debug_check_expansions("), spans);
        if spans {
            let exact = helper(&g.stages, "exact");
            assert!(
                exact.contains(
                    "Ok(c) => {\ndebug_check_expansions(scratch, position, &c);\nOk(Some(c))\n}"
                ),
                "{file}: {exact}"
            );
        }
        // The checks are debug assertions on the functions of the core
        // that need no arena, and their messages name the position.
        let mut helpers = vec![
            ("debug_check_err", "err_is_valid(p.err())", 2),
            ("debug_check_big", "big_is_canonical(scratch, b)", 1),
        ];
        if spans {
            helpers.push((
                "debug_check_expansions",
                "expansion_is_valid(scratch, span)",
                1,
            ));
        }
        for (name, call, assertions) in helpers {
            let h = helper(&g.stages, name);
            let start = g.stages.find(&format!("\nfn {name}(")).unwrap();
            assert!(
                g.stages[..start].ends_with("#[inline(always)]"),
                "{file}: {name}"
            );
            assert!(
                h.contains(&format!("implicit_predicates_core::contract::{call}")),
                "{file}: {h}"
            );
            assert_eq!(
                h.matches("debug_assert!(").count(),
                assertions,
                "{file}: {h}"
            );
            assert_eq!(h.matches("assert!(").count(), assertions, "{file}: {h}");
            assert_eq!(
                h.matches("\"breaks the contract of `Point`: argument {position}: ")
                    .count(),
                assertions,
                "{file}: {h}"
            );
            assert!(
                !h.contains("scratch.push") && !h.contains("&mut Scratch"),
                "{file}: {h}"
            );
        }
    }
}
