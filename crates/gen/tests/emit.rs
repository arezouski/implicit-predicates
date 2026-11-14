//! Tests for the code generated for a predicate.

use implicit_predicates_core::eft::{PRODUCT_OPERAND_MAX, PRODUCT_OPERAND_MIN};
use implicit_predicates_gen::{
    BIG_INTEGERS_ONLY_DEGREE, ExactRange, FilterForm, Generated, Leaves, MAX_COORD_COMPONENTS,
    difference_graph, exact_plan_differences, exact_plan_spans_translated, filter_bound,
    filter_form, generate, parse,
};
use proc_macro2::TokenStream;

const ORIENT3D: &str = include_str!("fixtures/orient3d.pred");
const INSPHERE: &str = include_str!("fixtures/insphere.pred");
const POINTS: &str = include_str!("fixtures/points.pred");
const INCIRCLE: &str = include_str!("fixtures/incircle.pred");

/// How the tests say to regenerate the code.
const REGENERATE: &str = "run the tests.";

fn generated(source: &str, file: &str) -> Generated {
    let mut all = generate(source, file, REGENERATE).unwrap();
    assert_eq!(all.len(), 1);
    all.remove(0)
}

/// The text after `pub const NAME: TYPE = ` up to the next `;`.
fn constant<'a>(code: &'a str, name: &str, ty: &str) -> &'a str {
    let prefix = format!("pub const {name}: {ty} = ");
    let start = code
        .find(&prefix)
        .unwrap_or_else(|| panic!("no constant {name}"))
        + prefix.len();
    let end = start + code[start..].find(';').unwrap();
    code[start..end].trim()
}

/// Parses a float literal of the output and checks it is exactly `expected`.
fn assert_literal(text: &str, expected: f64) {
    let value: f64 = text.parse().unwrap_or_else(|_| panic!("{text}"));
    assert_eq!(
        value.to_bits(),
        expected.to_bits(),
        "{text} is not {expected:e}"
    );
}

fn assert_range(code: &str, name: &str, range: ExactRange) {
    let text = constant(code, name, "(f64, f64)");
    let inner = text.strip_prefix('(').unwrap().strip_suffix(')').unwrap();
    let (min, max) = inner.split_once(',').unwrap();
    assert_literal(min.trim(), range.min);
    assert_literal(max.trim(), range.max);
}

#[test]
fn generation_is_deterministic() {
    for (source, file) in [
        (ORIENT3D, "predicates/orient3d.pred"),
        (INSPHERE, "predicates/insphere.pred"),
    ] {
        let first = generate(source, file, REGENERATE).unwrap();
        let second = generate(source, file, REGENERATE).unwrap();
        assert_eq!(first.len(), second.len());
        for (a, b) in first.iter().zip(&second) {
            assert_eq!(a.name, b.name);
            assert_eq!(a.stages, b.stages);
            assert_eq!(a.checks, b.checks);
        }
    }
}

#[test]
fn the_output_parses_as_rust() {
    // The output is lexed here; that it is valid Rust is shown by the command
    // line tool, which formats it with rustfmt, and by the generated file,
    // which is compiled as part of the workspace.
    for (source, file) in [
        (ORIENT3D, "predicates/orient3d.pred"),
        (INSPHERE, "predicates/insphere.pred"),
    ] {
        let g = generated(source, file);
        g.stages
            .parse::<TokenStream>()
            .unwrap_or_else(|e| panic!("{file}: {e}"));
    }
}

/// The literals of an array constant `[a, b, c]`.
fn array(text: &str) -> Vec<&str> {
    let inner = text.strip_prefix('[').unwrap().strip_suffix(']').unwrap();
    inner.split(',').map(str::trim).collect()
}

#[test]
fn the_constants_in_the_output_are_the_derived_ones() {
    let g = generated(ORIENT3D, "predicates/orient3d.pred");
    let code = &g.stages;
    let p = parse(ORIENT3D, "t.pred").unwrap().remove(0);
    let diff = difference_graph(&p.main, "t.pred", p.position).unwrap();
    let FilterForm::PerAxis(bound) = filter_form(&diff, "t.pred", p.position).unwrap() else {
        panic!("orient3d is multihomogeneous");
    };
    let constants = bound.constants();
    assert_eq!(constant(code, "DEGREES", "[u32; 3]"), "[1, 1, 1]");
    assert_eq!(constants.degrees, [1, 1, 1]);
    assert_literal(constant(code, "EPSILON", "f64"), constants.epsilon);
    let kappa = array(constant(code, "KAPPA", "[f64; 3]"));
    assert_eq!(kappa.len(), 3);
    for (text, &value) in kappa.iter().zip(&constants.kappa) {
        assert_literal(text, value);
    }
    assert_literal(constant(code, "D_MIN", "f64"), constants.d_min);
    assert_literal(constant(code, "D_MAX", "f64"), constants.d_max);
    // The exact derived values: no numeric test can tell a `KAPPA` of 6
    // from one of 3, the largest error reachable on one axis.
    assert!(bound.epsilon_is_u_times(46));
    for axis in 0..3 {
        assert!(bound.kappa_is(axis, 6), "axis {axis}");
        assert!(!bound.kappa_is(axis, 5) && !bound.kappa_is(axis, 7));
    }
    assert_eq!(constants.d_min, 2f64.powi(-324));
    assert_eq!(constants.d_max, 2f64.powi(339));
    // The same range and epsilon as the global form of the same graph.
    let global = filter_bound(&diff, "t.pred", p.position)
        .unwrap()
        .constants();
    assert_eq!(
        (global.d_min, global.d_max),
        (constants.d_min, constants.d_max)
    );
    assert_eq!(global.epsilon, constants.epsilon);
    let (min, max) = (PRODUCT_OPERAND_MIN, PRODUCT_OPERAND_MAX);
    let one = exact_plan_differences(&diff, Leaves::One, "t.pred", p.position, min, max).unwrap();
    let two = exact_plan_differences(&diff, Leaves::Two, "t.pred", p.position, min, max).unwrap();
    assert_range(code, "DIFFERENCES_RANGE", one.range);
    assert_range(code, "TRANSLATED_RANGE", two.range);
    // The values themselves, as derived for orient3d.
    assert_eq!(one.range.min, 2f64.powi(-188));
    assert_eq!(one.range.max, 2f64.powi(239));
    assert_eq!(two.range.max, 2f64.powi(238));
}

#[test]
fn the_constants_of_a_global_filter_are_the_derived_ones() {
    let g = generated(INSPHERE, "predicates/insphere.pred");
    let code = &g.stages;
    let p = parse(INSPHERE, "t.pred").unwrap().remove(0);
    let diff = difference_graph(&p.main, "t.pred", p.position).unwrap();
    let FilterForm::Global(bound) = filter_form(&diff, "t.pred", p.position).unwrap() else {
        panic!("in-sphere is not multihomogeneous");
    };
    let constants = bound.constants();
    assert_eq!(constant(code, "DEGREE", "u32"), "5");
    assert_literal(constant(code, "EPSILON", "f64"), constants.epsilon);
    assert_literal(constant(code, "KAPPA", "f64"), constants.kappa);
    assert_literal(constant(code, "D_MIN", "f64"), constants.d_min);
    assert_literal(constant(code, "D_MAX", "f64"), constants.d_max);
    assert!(bound.kappa_is(360));
    assert!(!code.contains("DEGREES"));
}

#[test]
fn the_header_says_do_not_edit() {
    let g = generated(ORIENT3D, "predicates/orient3d.pred");
    let mut lines = g.stages.lines();
    assert_eq!(
        lines.next(),
        Some("// Generated by implicit-predicates-gen from predicates/orient3d.pred. Do not edit.")
    );
    // The regeneration text given to `generate`, after "To regenerate,".
    assert_eq!(lines.next(), Some("// To regenerate, run the tests."));
}

/// The text of the function `name` in the unformatted module `code`, from
/// its signature to the closing brace followed by a blank line.
fn function<'a>(code: &'a str, name: &str) -> &'a str {
    let start = code
        .find(&format!("pub fn {name}("))
        .or_else(|| code.find(&format!("pub fn {name}<")))
        .unwrap_or_else(|| panic!("no function {name}"));
    let body = &code[start..];
    &body[..body.find("\n}\n\n").unwrap()]
}

/// One factor per axis and one more on x and on y: degree 5, the lowest
/// that has only the stage on big integers.
const DEGREE_5: &str = "fn p(a: Point3, b: Point3) -> Sign { \
                        (a.x - b.x) * (a.y - b.y) * (a.z - b.z) * (a.x - b.x) * (a.y - b.y) }";

/// [`DEGREE_5`] without its last factor: degree 4, which keeps the stages on
/// expansions.
const DEGREE_4: &str = "fn p(a: Point3, b: Point3) -> Sign { \
                        (a.x - b.x) * (a.y - b.y) * (a.z - b.z) * (a.x - b.x) }";

/// A form of degree 18, with a product operand of degree 17, which no plan
/// over spans can take.
const DEGREE_18: &str = "fn p(a: Point3, b: Point3) -> Sign { let x = a.x - b.x; \
                         let t1 = x * x; let t2 = t1 * t1; let t3 = t2 * t2; let t4 = t3 * t3; \
                         let t5 = t4 * x; t5 * x }";

#[test]
fn big_integers_alone_start_at_degree_5() {
    assert_eq!(BIG_INTEGERS_ONLY_DEGREE, 5);
    // The README quotes it twice; the CHANGELOG, outside the package, is
    // checked in `emitted_code.rs`.
    let readme = include_str!("../README.md")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    for quoted in [
        format!("total degree {BIG_INTEGERS_ONLY_DEGREE} or more (`BIG_INTEGERS_ONLY_DEGREE`)"),
        format!("for a main form of total degree {BIG_INTEGERS_ONLY_DEGREE} or more, none"),
    ] {
        assert!(readme.contains(&quoted), "{quoted}");
    }
    assert_eq!(readme.matches(" degree 5 ").count(), 2, "an untied mention");

    let degree = |source: &str| {
        let p = parse(source, "t.pred").unwrap().remove(0);
        let diff = difference_graph(&p.main, "t.pred", p.position).unwrap();
        diff.degrees[diff.root]
    };
    assert_eq!(degree(DEGREE_4), BIG_INTEGERS_ONLY_DEGREE - 1);
    assert_eq!(degree(DEGREE_5), BIG_INTEGERS_ONLY_DEGREE);
    assert_eq!(degree(INSPHERE), BIG_INTEGERS_ONLY_DEGREE);
    assert_eq!(degree(DEGREE_18), 18);
    // Degree 4 keeps every stage on expansions.
    let g = generated(DEGREE_4, "t.pred");
    for stage in [
        "exact_differences",
        "expansions_translated",
        "expansions_translated_in",
    ] {
        assert!(g.stages.contains(&format!("pub fn {stage}(")), "{stage}");
    }
}

#[test]
fn a_predicate_of_degree_5_or_more_has_only_the_stage_on_big_integers() {
    // In-sphere is not multihomogeneous, so it has the second test of its
    // filter, `filter_axes`; the others have the per-axis filter alone.
    for (source, file, points, zero, axes) in [
        (INSPHERE, "predicates/insphere.pred", 5, "0.0", true),
        (DEGREE_5, "t.pred", 2, "[0.0; 3]", false),
        (DEGREE_18, "t.pred", 2, "[0.0; 3]", false),
    ] {
        let g = generated(source, file);
        let code = &g.stages;
        code.parse::<TokenStream>().unwrap();
        for name in [
            "filter",
            "filter_points",
            "big_integers_in",
            "staged",
            "exact_stages_explicit_in",
            "exact_stages_in",
            "staged_in",
        ] {
            function(code, name);
        }
        assert!(code.contains("pub const BIG_INTEGERS_EXPLICIT_ARENA: usize = "));
        // No stage on expansions, nothing that only they use, and no
        // function that would always decline.
        for absent in [
            "expansions",
            "exact_differences",
            "SPANS_",
            "RANGE",
            "MAX_COORD_COMPONENTS",
            "staged_in_arena",
            "pub fn exact_stages(",
            "fn exact(",
            "in_range(",
            "fn product(",
            "ExactCoords",
            "Span",
            "Weight::",
            "eft::",
        ] {
            assert!(!code.contains(absent), "{file}: {absent}");
        }
        let names = &code[code.find("use implicit_predicates_core::{").unwrap()..];
        assert_eq!(
            &names[..names.find(";\n").unwrap()],
            "use implicit_predicates_core::{Big, BigCoords, ExactError, Point, Scratch, \
             ScratchExhausted, Sign}",
            "{file}"
        );
        // The filter alone, or its two tests, without an arena.
        let staged = function(code, "staged");
        let second = if axes {
            "\n.or_else(|| filter_axes(p, [0.0; 3]))"
        } else {
            ""
        };
        assert_eq!(
            staged,
            format!(
                "pub fn staged(p: [&[f64; 3]; {points}]) -> Option<Sign> {{\nfilter(p, {zero}){second}"
            ),
            "{file}"
        );
        assert_eq!(code.contains("filter_axes"), axes, "{file}");
        // The chains after the filter go to the stage on big integers, on the
        // coordinates of the points written to the arena from its mark.
        let explicit = function(code, "exact_stages_explicit_in");
        let first = if axes {
            "if let Some(sign) = filter_axes(p, [0.0; 3]) {\nreturn Ok(sign);\n}\nlet mark"
        } else {
            "let mark"
        };
        let order = [
            "fn big(",
            "let q0 = exact_big(scratch, p[0])?;",
            "big_integers_in(scratch, [&q0, ",
            first,
            "let mark = scratch.mark();\nlet result = big(scratch, p);\nscratch.reset(mark);\nresult",
        ]
        .map(|text| {
            explicit
                .find(text)
                .unwrap_or_else(|| panic!("{file}: {text}: {explicit}"))
        });
        assert!(order.is_sorted(), "{file}: {explicit}");
        let generic = function(code, "exact_stages_in");
        assert!(!generic.contains("fn spans"), "{file}: {generic}");
        assert!(!generic.contains("decided"), "{file}: {generic}");
        let first = if axes {
            "if let Some(sign) = filter_axes_points(p0, "
        } else {
            "let mark"
        };
        let order = [
            "fn big<",
            "let q0 = exact_big(scratch, p0)?;",
            "big_integers_in(scratch, [&q0, ",
            first,
            "let mark = scratch.mark();\nlet result = big(scratch, ",
            "scratch.reset(mark);\nresult",
        ]
        .map(|text| {
            generic
                .find(text)
                .unwrap_or_else(|| panic!("{file}: {text}: {generic}"))
        });
        assert!(order.is_sorted(), "{file}: {generic}");
        let chain = function(code, "staged_in");
        for text in [
            "filter_points(",
            "return exact_stages_explicit_in(scratch, [",
            "exact_stages_in(scratch, ",
        ] {
            assert!(chain.contains(text), "{file}: {text}: {chain}");
        }
    }
}

#[test]
fn incircle_chains_the_translated_form_over_spans() {
    let g = generated(INCIRCLE, "t.pred");
    assert_eq!(g.name, "incircle");
    // The exact differences of in-circle fit on the stack and its translated
    // expansions do not, so its chain takes an arena and runs the translated
    // form over spans after the exact differences.
    assert!(g.stages.contains("pub fn exact_differences("));
    assert!(g.stages.contains("DIFFERENCES_RANGE"));
    assert!(!g.stages.contains("expansions_translated("));
    assert!(!g.stages.contains("pub const TRANSLATED_RANGE"));
    assert!(g.stages.contains("pub fn expansions_translated_in("));
    let staged = &g.stages[g.stages.find("pub fn staged").unwrap()..];
    let signature = &staged[..staged.find('{').unwrap()];
    assert_eq!(
        signature,
        "pub fn staged(scratch: &mut Scratch<'_>, p: [&[f64; 3]; 4]) \
         -> Result<Option<Sign>, ScratchExhausted> "
    );
    let body = &staged[..staged.find("\n}\n\n").unwrap()];
    let order = [
        "filter(p, 0.0)",
        "exact_differences(p)",
        "staged_in_arena(scratch, p)",
    ]
    .map(|text| body.find(text).unwrap_or_else(|| panic!("{text}: {body}")));
    assert!(order.is_sorted(), "{body}");
    let arena = &g.stages[g.stages.find("fn staged_in_arena").unwrap()..];
    let arena = &arena[..arena.find("\n}\n\n").unwrap()];
    let order = [
        "let mark = scratch.mark();",
        "exact(scratch, p[0])?",
        "expansions_translated_in(scratch, q)",
        "scratch.reset(mark);",
    ]
    .map(|text| {
        arena
            .rfind(text)
            .unwrap_or_else(|| panic!("{text}: {arena}"))
    });
    // The mark is taken before `run` is called, after it is defined.
    assert!(
        order[1] < order[0] && order[2] < order[0] && order[0] < order[3],
        "{arena}"
    );
}

#[test]
fn orient3d_keeps_its_two_stages_on_the_stack_and_its_chain() {
    let g = generated(ORIENT3D, "predicates/orient3d.pred");
    for stage in [
        "exact_differences",
        "expansions_translated",
        "expansions_translated_in",
    ] {
        assert!(g.stages.contains(&format!("pub fn {stage}(")), "{stage}");
    }
    assert!(
        g.stages
            .contains("pub fn staged(p: [&[f64; 3]; 4]) -> Option<Sign> {")
    );
    assert!(!g.stages.contains("staged_in_arena"));
}

#[test]
fn the_output_has_no_untranslated_stage() {
    // The `#[exact]` form is only checked against the main form: no stage,
    // range or check is emitted for it.
    for (source, file) in [
        (ORIENT3D, "predicates/orient3d.pred"),
        (INSPHERE, "predicates/insphere.pred"),
    ] {
        let g = generated(source, file);
        for (what, text) in [("stages", &g.stages), ("checks", &g.checks)] {
            assert!(
                !text.to_lowercase().contains("untranslated"),
                "{file}: {what}"
            );
        }
    }
}

#[test]
fn the_ranges_over_spans_in_the_output_are_the_derived_ones() {
    let g = generated(ORIENT3D, "predicates/orient3d.pred");
    let code = &g.stages;
    assert_eq!(constant(code, "MAX_COORD_COMPONENTS", "usize"), "16");
    assert_eq!(MAX_COORD_COMPONENTS, 16);
    let p = parse(ORIENT3D, "t.pred").unwrap().remove(0);
    let diff = difference_graph(&p.main, "t.pred", p.position).unwrap();
    let (min, max) = (PRODUCT_OPERAND_MIN, PRODUCT_OPERAND_MAX);
    let translated =
        exact_plan_spans_translated(&diff, 16, "t.pred", p.position, min, max).unwrap();
    assert_range(code, "SPANS_TRANSLATED_RANGE", translated.range);
    // The values themselves, as derived for orient3d.
    assert_eq!(translated.range.min, 2f64.powi(-188));
    assert_eq!(translated.range.max, 2f64.powi(234));
}

#[test]
fn the_ranges_over_spans_do_not_depend_on_the_point_types_of_the_file() {
    // The same predicate, alone and in a file that declares point types.
    let alone = generated(ORIENT3D, "predicates/orient3d.pred");
    let with_points = format!("{POINTS}\n{ORIENT3D}");
    let mut all = generate(&with_points, "predicates/orient3d.pred", REGENERATE).unwrap();
    assert_eq!(all.len(), 1);
    let with_points = all.remove(0);
    for name in ["MAX_COORD_COMPONENTS", "SPANS_TRANSLATED_RANGE"] {
        let ty = if name == "MAX_COORD_COMPONENTS" {
            "usize"
        } else {
            "(f64, f64)"
        };
        assert_eq!(
            constant(&alone.stages, name, ty),
            constant(&with_points.stages, name, ty),
            "{name}"
        );
    }
    assert_eq!(alone.stages, with_points.stages);
}

#[test]
fn the_chain_tries_the_exact_differences_before_the_translated_form() {
    let o = generated(ORIENT3D, "predicates/orient3d.pred");
    let staged = &o.stages[o.stages.find("pub fn staged").unwrap()..];
    let body = &staged[..staged.find("\n}").unwrap()];
    let order = [
        "filter(p, [0.0; 3])",
        "exact_differences(p)",
        "expansions_translated(p)",
    ]
    .map(|stage| {
        body.find(stage)
            .unwrap_or_else(|| panic!("{stage}: {body}"))
    });
    assert!(order.is_sorted(), "{body}");
}

#[test]
fn shared_subexpressions_are_emitted_once() {
    // The leaf `c.y - d.y` is shared by two minors of orient3d: the stage
    // on two components computes it once and reads it twice.
    let g = generated(ORIENT3D, "predicates/orient3d.pred");
    let stage = &g.stages[g.stages.find("pub fn expansions_translated(").unwrap()..];
    let stage = &stage[..stage.find("\n}\n\n").unwrap()];
    let leaf = "let l7 = expansion::from_two_diff(p[2][1], p[3][1], &mut e7);";
    assert_eq!(g.stages.matches(leaf).count(), 1);
    assert_eq!(stage.matches(leaf).count(), 1, "{stage}");
    assert_eq!(stage.matches("&e7[..l7]").count(), 2, "{stage}");
    // In-sphere: the minor `cy * dz - cz * dy` is shared by two cofactors,
    // and the leaf `c.y - e.y` by many products.
    let s = generated(INSPHERE, "predicates/insphere.pred");
    let filter = &s.stages[s.stages.find("pub fn filter").unwrap()..];
    let filter = &filter[..filter.find("\n}").unwrap()];
    assert_eq!(filter.matches("p[2][1] - p[4][1]").count(), 1, "{filter}");
}

#[test]
fn a_definition_error_is_reported_with_its_position() {
    let e = generate(
        "fn p(a: Point3, b: Point3) -> Sign {\n    a.x * b.y\n}",
        "predicates/p.pred",
        REGENERATE,
    )
    .unwrap_err();
    assert_eq!(e.file, "predicates/p.pred");
    assert_eq!((e.line, e.column), (1, 4), "{e}");
}

#[test]
fn the_header_names_the_file_and_tells_how_to_regenerate() {
    let all = generate(
        ORIENT3D,
        "defs/o.pred",
        "run, from the root:\nmake predicates",
    )
    .unwrap();
    let expected = "// Generated by implicit-predicates-gen from defs/o.pred. Do not edit.\n\
                    // To regenerate, run, from the root:\n\
                    // make predicates\n\n";
    for code in [&all[0].stages, &all[0].checks] {
        assert!(code.starts_with(expected), "{}", &code[..200]);
    }
}

#[test]
fn an_exact_form_with_a_constant_that_is_not_exact_in_f64_is_accepted() {
    // 2^53 + 1 has no `f64`. The exact form is only checked to be the
    // polynomial of the main form: no stage is planned on it, so nothing
    // rejects its constants.
    let source = "fn p(a: Point3, b: Point3) -> Sign { (a.x - b.x) * (a.y - b.y) }\n\
                  #[exact]\n\
                  fn p(a: Point3, b: Point3) -> Sign { \
                  9007199254740993 * (a.x - b.x) * (a.y - b.y) \
                  - 9007199254740992 * (a.x - b.x) * (a.y - b.y) }\n";
    assert_ne!(9007199254740993u64 as f64 as u64, 9007199254740993);
    let g = generated(source, "t.pred");
    assert!(!g.stages.contains("9007199254740993"));
}
