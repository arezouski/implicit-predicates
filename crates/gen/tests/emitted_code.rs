//! What the generator emits for definitions other than the ones of the
//! `implicit-predicates` crate compiles, without warnings, and its checks run:
//! each test writes the generated modules into a small crate that depends
//! on the crates of this workspace, and runs its tests with Cargo.
//!
//! The crates share one target directory, so the dependencies are built
//! once; Cargo is run offline, with the lock file of the workspace.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use implicit_predicates_gen::{Polynomial, canonical, generate, generate_points, parse};

/// How the tests say to regenerate the code.
const REGENERATE: &str = "run the tests.";

/// The root of the workspace.
fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Writes a crate named `name` with the modules `modules` (name and text)
/// and the integration test `test`, runs its tests and asserts that they
/// pass.
fn compile_and_run(name: &str, modules: &[(String, String)], test: &str) {
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR")).join("emitted");
    let root = tmp.join(name);
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("tests")).unwrap();
    let crates = workspace().join("crates");
    let dependency = |dir: &str| {
        let path = crates.join(dir);
        format!("{{ path = {:?} }}", path.to_str().unwrap())
    };
    fs::write(
        root.join("Cargo.toml"),
        format!(
            "[package]\nname = \"{name}\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\
             publish = false\n\n[workspace]\n\n[dependencies]\n\
             implicit-predicates = {}\nimplicit-predicates-core = {}\n\
             implicit-predicates-oracle = {}\n",
            dependency("implicit-predicates"),
            dependency("core"),
            dependency("oracle"),
        ),
    )
    .unwrap();
    fs::copy(workspace().join("Cargo.lock"), root.join("Cargo.lock")).unwrap();
    let mut lib = String::from("#![deny(warnings)]\n");
    for (module, text) in modules {
        lib.push_str(&format!("pub mod {module};\n"));
        fs::write(root.join("src").join(format!("{module}.rs")), text).unwrap();
    }
    fs::write(root.join("src/lib.rs"), lib).unwrap();
    fs::write(
        root.join("tests/run.rs"),
        format!(
            "#![deny(warnings)]\n\nuse {}::*;\n\n{test}",
            name.replace('-', "_")
        ),
    )
    .unwrap();
    let output = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(["test", "--offline", "--quiet", "--manifest-path"])
        .arg(root.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", tmp.join("target"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{name}:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The one predicate of the file `path`: its name, its parameters, and the
/// canonical polynomials of its main form and of its `#[exact]` form, if
/// it has one.
fn canonical_forms(path: &Path) -> (String, Vec<String>, Polynomial, Option<Polynomial>) {
    let source = fs::read_to_string(path).unwrap();
    let file = path.to_str().unwrap();
    let mut all = parse(&source, file).unwrap();
    assert_eq!(all.len(), 1, "{file}");
    let p = all.remove(0);
    let main = canonical(&p.main, file, p.position).unwrap();
    let exact = p
        .exact
        .as_ref()
        .map(|exact| canonical(exact, file, p.exact_position.unwrap()).unwrap());
    (p.name, p.params, main, exact)
}

#[test]
fn the_predicates_of_the_library_are_those_of_the_fixtures() {
    // The tests of the generator read the fixtures, which the package of the
    // generator holds; the library has its own definitions. Each form of
    // the library has the polynomial of a fixture: `orient3d` both of the
    // fixture `orient3d.pred`, `insphere` the main form of `insphere.pred`
    // and, as its exact form, the one of `insphere_raw.pred`.
    let library = workspace().join("crates/implicit-predicates/predicates");
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let (name, params, main, exact) = canonical_forms(&library.join("orient3d.pred"));
    let fixture = canonical_forms(&fixtures.join("orient3d.pred"));
    assert_eq!((&name, &params), (&fixture.0, &fixture.1));
    assert_eq!(main, fixture.2);
    assert_eq!(exact, fixture.3);
    assert!(exact.is_some());

    let (name, params, main, exact) = canonical_forms(&library.join("insphere.pred"));
    let fixture = canonical_forms(&fixtures.join("insphere.pred"));
    let raw = canonical_forms(&fixtures.join("insphere_raw.pred"));
    assert_eq!((&name, &params), (&fixture.0, &fixture.1));
    assert_eq!((&name, &params), (&raw.0, &raw.1));
    assert_eq!(main, fixture.2);
    assert_eq!(exact, Some(raw.2));
    assert_eq!((fixture.3, raw.3), (None, None));
}

/// The modules of the stages and of the checks of the one predicate of
/// `source`, named `stages` and `checks`.
fn predicate_modules(source: &str) -> Vec<(String, String)> {
    let mut all = generate(source, "t.pred", REGENERATE).unwrap();
    assert_eq!(all.len(), 1);
    let g = all.remove(0);
    vec![("stages".into(), g.stages), ("checks".into(), g.checks)]
}

/// The test that runs every stage and the uncertainty checks of a
/// predicate of `points` points, and asserts that the kinds of uncertainty
/// in `kinds` have cases.
fn predicate_test(kinds: &[usize]) -> String {
    format!(
        "use implicit_predicates_core::filter::uncertainty;\n\n\
         #[test]\n\
         fn the_checks_run() {{\n\
         let [decided, _] = checks::check_bound_edge_cases(|p| stages::staged(p.each_ref()));\n\
         assert!(decided > 0);\n\
         let counts = checks::check_uncertainty_edge_cases(|p, e| {{\n\
         stages::filter(p.each_ref(), uncertainty(e.each_ref()))\n\
         }});\n\
         for kind in {kinds:?} {{\n\
         assert!(counts[kind][0] + counts[kind][1] > 0, \"{{counts:?}}\");\n\
         }}\n\
         }}\n"
    )
}

#[test]
fn the_checks_of_a_predicate_of_degree_zero_in_one_axis_run() {
    // Degrees (1, 1, 0): the cases with an uncertainty on every axis have
    // none on z.
    let source = "fn orient2d(a: Point3, b: Point3, c: Point3) -> Sign {\n    \
                  (a.x - c.x) * (b.y - c.y) - (a.y - c.y) * (b.x - c.x)\n}\n";
    compile_and_run(
        "emitted-orient2d",
        &predicate_modules(source),
        &predicate_test(&[0, 1, 3]),
    );
}

/// A predicate of `points` points, a multiple of 6, with the same degree
/// in each axis in every term: a sum of products of one difference per
/// axis, each over its own points.
fn many_points(points: usize) -> String {
    let params: Vec<String> = (0..points).map(|i| format!("p{i}: Point3")).collect();
    let terms: Vec<String> = (0..points / 6)
        .map(|k| {
            let b = 6 * k;
            format!(
                "(p{b}.x - p{}.x) * (p{}.y - p{}.y) * (p{}.z - p{}.z)",
                b + 1,
                b + 2,
                b + 3,
                b + 4,
                b + 5
            )
        })
        .collect();
    format!(
        "fn many({}) -> Sign {{\n    {}\n}}\n",
        params.join(", "),
        terms.join(" - ")
    )
}

#[test]
fn the_checks_of_a_predicate_of_24_points_compile_and_run() {
    // 72 coordinates: `2^72` corners per case cannot be checked one by one.
    compile_and_run(
        "emitted-many",
        &predicate_modules(&many_points(24)),
        &predicate_test(&[0, 1, 2, 3]),
    );
}

/// The test of a predicate of two points `a` and `b` whose chain has no
/// arena: `staged` is the filter alone, so the stage on big integers decides
/// what it declines. Its sign is the sign of `(a.x - b.x)^18` by `oracle`,
/// a power written by hand on exact dyadic numbers, from the exact `x` of
/// each point.
const DEGREE_18_TEST: &str = r#"use implicit_predicates::{Lnc, Scratch, Sign};
use implicit_predicates_oracle::Dyadic;

/// The sign of `(a - b)^18`.
fn oracle(a: &Dyadic, b: &Dyadic) -> Sign {
    let d = a - b;
    let mut power = Dyadic::from_f64(1.0);
    for _ in 0..18 {
        power = &power * &d;
    }
    Sign::from(power.sign().cmp(&0))
}

/// `q + t (p - q)`, exactly: the `x` of a point on a line.
fn lnc_x(p: f64, q: f64, t: f64) -> Dyadic {
    let (p, q, t) = (Dyadic::from_f64(p), Dyadic::from_f64(q), Dyadic::from_f64(t));
    &q + &(&t * &(&p - &q))
}

const VALUES: [f64; 12] = [
    0.0, 1.0, -1.0, 3.0, 3.0000000000000004, 0.1, 1e-300, -5e-324, 1e300, f64::MAX, -f64::MAX,
    0.30000000000000004,
];

#[test]
fn the_chain_has_the_sign_of_the_oracle() {
    let mut buffer = vec![0.0; 1 << 16];
    let mut scratch = Scratch::borrowed(&mut buffer);
    let mut declined = 0;
    for &x in &VALUES {
        for &y in &VALUES {
            let (a, b) = ([x, 0.5, -2.0], [y, 0.25, 7.0]);
            let expected = oracle(&Dyadic::from_f64(x), &Dyadic::from_f64(y));
            let staged = stages::staged([&a, &b]);
            assert!(staged.is_none_or(|s| s == expected), "{a:?} {b:?}");
            if staged.is_none() {
                declined += 1;
            }
            assert_eq!(stages::staged_in(&mut scratch, &a, &b), Ok(expected));
            assert_eq!(stages::exact_stages_explicit_in(&mut scratch, [&a, &b]), Ok(expected));
            assert_eq!(stages::exact_stages_in(&mut scratch, &a, &b), Ok(expected));
            // A point on a line, near `a`, and one whose parents are both `b`.
            if x.next_up().is_finite() {
                let line = Lnc::new_unchecked(a, [x.next_up(), 1.0, 1.0], 0.375);
                let exact = oracle(&lnc_x(a[0], x.next_up(), 0.375), &Dyadic::from_f64(y));
                let at_b = Lnc::new_unchecked(b, b, 0.75);
                assert_eq!(stages::staged_in(&mut scratch, &line, &at_b), Ok(exact));
                assert_eq!(stages::exact_stages_in(&mut scratch, &line, &at_b), Ok(exact));
            }
            assert_eq!(scratch.used(), 0);
        }
    }
    assert!(declined > 0, "the filter decides every call");
}
"#;

#[test]
fn a_predicate_of_degree_18_decides_every_call_on_big_integers() {
    // The form of degree 18, with a product operand of degree 17, that no
    // plan over spans can take: its chain is the filter and the stage on big
    // integers.
    let source = "fn p(a: Point3, b: Point3) -> Sign { let x = a.x - b.x; \
                  let t1 = x * x; let t2 = t1 * t1; let t3 = t2 * t2; let t4 = t3 * t3; \
                  let t5 = t4 * x; t5 * x }";
    // Its checks have no edge case of the filter, so the test does not run
    // them; the generator warns about it.
    let modules = predicate_modules(source);
    assert!(!modules[0].1.contains("expansions"), "{}", modules[0].1);
    compile_and_run("emitted-degree-18", &modules, DEGREE_18_TEST);
}

/// The in-circle predicate of the projections of four points on the plane
/// `z = 0`, of degree 4: its exact differences fit on the stack, and its
/// translated expansions do not, so that the stage over spans takes their
/// place in the chain for explicit points.
const INCIRCLE: &str = include_str!("fixtures/incircle.pred");

/// The test of [`INCIRCLE`]: its chain against the determinant written by
/// hand on exact dyadic numbers, on points of a circle and near one, moved
/// so that their differences are not exact in `f64`.
const INCIRCLE_TEST: &str = r#"use implicit_predicates::{Lnc, Scratch, Sign};
use implicit_predicates_oracle::Dyadic;

/// The sign of the in-circle determinant of the projections of `p`: the
/// rows `(x - d.x, y - d.y, (x - d.x)^2 + (y - d.y)^2)` of `a`, `b` and `c`.
fn oracle(p: [[Dyadic; 2]; 4]) -> Sign {
    let r: Vec<[Dyadic; 3]> = p[..3]
        .iter()
        .map(|q| {
            let (x, y) = (&q[0] - &p[3][0], &q[1] - &p[3][1]);
            let lift = &(&x * &x) + &(&y * &y);
            [x, y, lift]
        })
        .collect();
    let minor = |i: usize, j: usize, k: usize, l: usize| &(&r[i][k] * &r[j][l]) - &(&r[j][k] * &r[i][l]);
    let det = &(&(&r[0][2] * &minor(1, 2, 0, 1)) + &(&r[1][2] * &minor(2, 0, 0, 1)))
        + &(&r[2][2] * &minor(0, 1, 0, 1));
    Sign::from(det.sign().cmp(&0))
}

fn exact(p: &[[f64; 3]; 4]) -> [[Dyadic; 2]; 4] {
    p.map(|q| [Dyadic::from_f64(q[0]), Dyadic::from_f64(q[1])])
}

#[test]
fn the_chain_has_the_sign_of_the_oracle() {
    let mut buffer = vec![0.0; 1 << 16];
    let mut scratch = Scratch::borrowed(&mut buffer);
    let circle = [[5.0, 0.0], [3.0, 4.0], [0.0, 5.0], [-4.0, 3.0], [-3.0, -4.0], [4.0, -3.0]];
    let mut on_spans = 0;
    let mut signs = [0; 3];
    for scale in [1.0, 1e-9, 3.0e7, 1e40] {
        for offset in [0.0, 0.1, 1.0 / 3.0, 12345.678] {
            for skip in 0..6 {
                for nudge in [0.0, 1.0, -1.0] {
                    let mut p = [[0.0f64; 3]; 4];
                    let points = (0..6).filter(|&i| i != skip).take(4);
                    for (slot, i) in p.iter_mut().zip(points) {
                        let [x, y] = circle[i];
                        *slot = [x * scale + offset, y * scale - offset, 0.5];
                    }
                    p[3][0] += nudge * p[3][0].abs().max(1.0) * f64::EPSILON;
                    let expected = oracle(exact(&p));
                    signs[(i8::from(expected) + 1) as usize] += 1;
                    let r = p.each_ref();
                    let staged = stages::staged(&mut scratch, r).unwrap();
                    assert_eq!(staged, Some(expected), "{p:?}");
                    if stages::filter(r, 0.0).is_none() && stages::exact_differences(r).is_none() {
                        on_spans += 1;
                    }
                    let [a, b, c, d] = &p;
                    assert_eq!(stages::staged_in(&mut scratch, a, b, c, d), Ok(expected));
                    assert_eq!(stages::exact_stages_explicit_in(&mut scratch, r), Ok(expected));
                    let [a, b, c, d] = p.map(|q| Lnc::new_unchecked(q, q, 0.625));
                    assert_eq!(stages::staged_in(&mut scratch, &a, &b, &c, &d), Ok(expected));
                    assert_eq!(stages::exact_stages_in(&mut scratch, &a, &b, &c, &d), Ok(expected));
                    assert_eq!(scratch.used(), 0);
                }
            }
        }
    }
    assert!(on_spans > 0, "no call reaches the stage over spans");
    assert!(signs.iter().all(|&n| n > 0), "{signs:?}");
    let [decided, _] = checks::check_bound_edge_cases(|p| {
        stages::staged(&mut scratch, p.each_ref()).unwrap()
    });
    assert!(decided > 0);
}
"#;

#[test]
fn a_predicate_of_degree_4_runs_the_stage_over_spans_in_place_of_a_stage_on_the_stack() {
    let modules = predicate_modules(INCIRCLE);
    let stages = &modules[0].1;
    for emitted in [
        "pub fn exact_differences(",
        "pub fn expansions_translated_in(",
        "fn staged_in_arena(",
    ] {
        assert!(stages.contains(emitted), "{emitted}");
    }
    assert!(!stages.contains("pub fn expansions_translated("));
    compile_and_run("emitted-incircle", &modules, INCIRCLE_TEST);
}

#[test]
fn point_types_with_parameters_named_like_local_names_compile() {
    // Names that the emitted functions use for their own variables, and
    // snake case names with digits and underscores.
    let source = "point Mix(c: F64x3, s: F64x3, x: f64, scratch: f64, mark: f64, \
                  t__: f64, t_a1: f64) = s + x*(c - s) + scratch*mark*t__*t_a1*c\n    where 0 <= x, x <= 1;\n";
    let g = generate_points(source, "t.pred", REGENERATE)
        .unwrap()
        .unwrap();
    compile_and_run(
        "emitted-points",
        &[
            ("points".into(), g.stages),
            ("points_checks".into(), g.checks),
        ],
        "use implicit_predicates_core::{Point, Scratch};\n\n\
         #[test]\n\
         fn the_points_are_built_and_written() {\n\
         let p = points::Mix::new([1.0, 2.0, 3.0], [0.5, 0.25, 4.0], 0.5, 2.0, 0.125, 1.5, 0.75).unwrap();\n\
         let exact = points_checks::exact_mix(\n\
         [[1.0, 2.0, 3.0], [0.5, 0.25, 4.0]],\n\
         [0.5, 2.0, 0.125, 1.5, 0.75],\n\
         );\n\
         let mut buffer = [0.0; 4096];\n\
         let mut scratch = Scratch::borrowed(&mut buffer);\n\
         let coords = p.exact(&mut scratch).unwrap();\n\
         let spans = [coords.x, coords.y, coords.z];\n\
         for k in 0..3 {\n\
         let sum = implicit_predicates_oracle::Dyadic::sum_of(scratch.get(spans[k]));\n\
         assert_eq!(sum, exact[k]);\n\
         }\n\
         }\n",
    );
}
