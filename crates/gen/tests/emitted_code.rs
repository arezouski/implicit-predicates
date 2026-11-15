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

use implicit_predicates_gen::{
    BIG_INTEGERS_ONLY_DEGREE, Polynomial, canonical, generate, generate_points, parse,
};

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

/// The test of the predicate of degree 5 `(a.x - b.x)^2 (a.y - b.y)^2
/// (a.z - b.z)`, of odd degree, so that it takes the three signs: its chain
/// against the product written by hand on exact dyadic numbers.
const DEGREE_5_TEST: &str = r#"use implicit_predicates::{Lnc, Scratch, Sign};
use implicit_predicates_oracle::Dyadic;

/// The sign of `(a.x - b.x)^2 (a.y - b.y)^2 (a.z - b.z)` at the exact points.
fn oracle(a: &[Dyadic; 3], b: &[Dyadic; 3]) -> Sign {
    let d: Vec<Dyadic> = (0..3).map(|k| &a[k] - &b[k]).collect();
    let value = &(&(&(&d[0] * &d[1]) * &d[0]) * &d[1]) * &d[2];
    Sign::from(value.sign().cmp(&0))
}

fn exact(p: [f64; 3]) -> [Dyadic; 3] {
    p.map(Dyadic::from_f64)
}

const VALUES: [f64; 7] = [0.0, 1.0, -3.0, 1e-300, 1e300, -5e-324, 0.1];

#[test]
fn the_chain_has_the_sign_of_the_oracle() {
    let mut buffer = vec![0.0; 1 << 16];
    let mut scratch = Scratch::borrowed(&mut buffer);
    let mut signs = [0; 3];
    let mut declined = 0;
    for &x in &VALUES {
        for &y in &VALUES {
            for &z in &VALUES {
                let (a, b) = ([x, y, z], [0.1, -0.1, 0.1]);
                let expected = oracle(&exact(a), &exact(b));
                signs[(i8::from(expected) + 1) as usize] += 1;
                let staged = stages::staged([&a, &b]);
                assert!(staged.is_none_or(|s| s == expected), "{a:?}");
                if staged.is_none() {
                    declined += 1;
                }
                assert_eq!(stages::staged_in(&mut scratch, &a, &b), Ok(expected), "{a:?}");
                assert_eq!(stages::exact_stages_explicit_in(&mut scratch, [&a, &b]), Ok(expected));
                let (la, lb) = (Lnc::new_unchecked(a, a, 0.5), Lnc::new_unchecked(b, b, 0.25));
                assert_eq!(stages::staged_in(&mut scratch, &la, &lb), Ok(expected), "{a:?}");
                assert_eq!(stages::exact_stages_in(&mut scratch, &la, &lb), Ok(expected));
                assert_eq!(scratch.used(), 0);
            }
        }
    }
    assert!(signs.iter().all(|&n| n > 0), "{signs:?}");
    assert!(declined > 0, "the filter decides every call");
}
"#;

#[test]
fn a_predicate_of_degree_5_takes_every_sign_on_big_integers() {
    let source = "fn p(a: Point3, b: Point3) -> Sign { \
                  (a.x - b.x) * (a.y - b.y) * (a.x - b.x) * (a.y - b.y) * (a.z - b.z) }";
    let modules = predicate_modules(source);
    assert!(!modules[0].1.contains("expansions"), "{}", modules[0].1);
    compile_and_run("emitted-degree-5", &modules, DEGREE_5_TEST);
}

/// `(ax + ay + az)^8` in differences, by repeated squaring: a form that is
/// not multihomogeneous, with more degree vectors than the bound by parts
/// takes, so that its filter is the global one alone, with no
/// `filter_axes`, and, of degree 8, its only exact stage is the one on big
/// integers.
const OVER_BUDGET: &str = "fn over_budget(a: Point3, b: Point3) -> Sign {
    let s0 = (a.x - b.x) + (a.y - b.y) + (a.z - b.z);
    let s1 = s0 * s0;
    let s2 = s1 * s1;
    s2 * s2
}
";

/// The test of [`OVER_BUDGET`]: its chain against the power written by hand
/// on exact dyadic numbers, on points whose sum of differences is zero,
/// tiny or not, where the global filter decides some calls and declines
/// others.
const OVER_BUDGET_TEST: &str = r#"use implicit_predicates::{Lnc, Scratch, Sign};
use implicit_predicates_oracle::Dyadic;

/// The sign of `((a.x - b.x) + (a.y - b.y) + (a.z - b.z))^8` at the exact
/// points.
fn oracle(a: [f64; 3], b: [f64; 3]) -> Sign {
    let d = |k: usize| &Dyadic::from_f64(a[k]) - &Dyadic::from_f64(b[k]);
    let s0 = &(&d(0) + &d(1)) + &d(2);
    let s1 = &s0 * &s0;
    let s2 = &s1 * &s1;
    let value = &s2 * &s2;
    Sign::from(value.sign().cmp(&0))
}

const VALUES: [f64; 7] = [0.0, 1.0, -1.0, 0.1, -0.2, 1e-30, 3.0];

#[test]
fn the_chain_has_the_sign_of_the_oracle() {
    let mut buffer = vec![0.0; 1 << 16];
    let mut scratch = Scratch::borrowed(&mut buffer);
    let mut signs = [0; 3];
    let (mut decided, mut declined) = (0, 0);
    for &x in &VALUES {
        for &y in &VALUES {
            for &z in &VALUES {
                let (a, b) = ([x, y, z], [0.1, -0.2, 1e-30]);
                let expected = oracle(a, b);
                signs[(i8::from(expected) + 1) as usize] += 1;
                let filtered = stages::filter([&a, &b], 0.0);
                assert!(filtered.is_none_or(|s| s == expected), "{a:?}");
                assert_eq!(stages::staged([&a, &b]), filtered, "{a:?}");
                if filtered.is_some() {
                    decided += 1;
                } else {
                    declined += 1;
                }
                assert_eq!(stages::staged_in(&mut scratch, &a, &b), Ok(expected), "{a:?}");
                assert_eq!(stages::exact_stages_explicit_in(&mut scratch, [&a, &b]), Ok(expected));
                let (la, lb) = (Lnc::new_unchecked(a, a, 0.5), Lnc::new_unchecked(b, b, 0.25));
                assert_eq!(stages::staged_in(&mut scratch, &la, &lb), Ok(expected), "{a:?}");
                assert_eq!(stages::exact_stages_in(&mut scratch, &la, &lb), Ok(expected));
                assert_eq!(scratch.used(), 0);
            }
        }
    }
    assert!(signs[1] > 0 && signs[2] > 0 && signs[0] == 0, "{signs:?}");
    assert!(decided > 0 && declined > 0, "{decided} {declined}");
}
"#;

#[test]
fn a_form_beyond_the_budget_of_the_bound_by_parts_runs_the_global_filter_alone() {
    let mut all = generate(OVER_BUDGET, "t.pred", REGENERATE).unwrap();
    let g = all.remove(0);
    assert!(
        g.warnings
            .iter()
            .any(|w| w.contains("has no `filter_axes`")),
        "{:?}",
        g.warnings
    );
    for text in [&g.stages, &g.checks] {
        for absent in ["filter_axes", "AXES_", "expansions"] {
            assert!(!text.contains(absent), "{absent}");
        }
    }
    let modules = vec![("stages".into(), g.stages), ("checks".into(), g.checks)];
    compile_and_run("emitted-over-budget", &modules, OVER_BUDGET_TEST);
}

/// The in-circle predicate of the projections of four points on the plane
/// `z = 0`, of degree 4: its exact differences fit on the stack, and its
/// translated expansions do not, so that the stage over spans takes their
/// place in the chain for explicit points.
const INCIRCLE: &str = include_str!("fixtures/incircle.pred");

/// The test of [`INCIRCLE`]: its chain against the determinant written by
/// hand on exact dyadic numbers, on points of a circle and near one, moved
/// so that their differences are not exact in `f64`; on points of the circle
/// scaled by a power of two, which must be zero; and on points outside the
/// range of the stage over spans, which the chain hands over to the stage on
/// big integers; and on points that break the contract of `Point`, which
/// the checks of debug builds report.
const INCIRCLE_TEST: &str = r#"use implicit_predicates::{
    BigCoords, ExactCoords, ExactError, Lnc, Point, Scratch, Sign,
};
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

const CIRCLE: [[f64; 2]; 6] = [[5.0, 0.0], [3.0, 4.0], [0.0, 5.0], [-4.0, 3.0], [-3.0, -4.0], [4.0, -3.0]];

/// Four points of `CIRCLE`, without the point `skip`, scaled by `scale`,
/// moved by `offset` and with the `x` of the last one moved by `nudge`
/// units in the last place.
fn on_circle(scale: f64, offset: f64, skip: usize, nudge: i32) -> [[f64; 3]; 4] {
    let mut p = [[0.0f64; 3]; 4];
    let points = (0..6).filter(|&i| i != skip).take(4);
    for (slot, i) in p.iter_mut().zip(points) {
        let [x, y] = CIRCLE[i];
        *slot = [x * scale + offset, y * scale - offset, 0.5];
    }
    for _ in 0..nudge.unsigned_abs() {
        p[3][0] = if nudge > 0 { p[3][0].next_up() } else { p[3][0].next_down() };
    }
    p
}

#[test]
fn exactly_concyclic_points_are_zero() {
    // Integer points of a circle scaled by a power of two, not moved: the
    // determinant is zero, whatever form computes it.
    let mut buffer = vec![0.0; 1 << 16];
    let mut scratch = Scratch::borrowed(&mut buffer);
    for exponent in [0, -30, 40, 242, -200] {
        let scale = 2f64.powi(exponent);
        for skip in 0..6 {
            let p = on_circle(scale, 0.0, skip, 0);
            assert_eq!(oracle(exact(&p)), Sign::Zero, "{p:?}");
            let [a, b, c, d] = &p;
            assert_eq!(stages::staged_in(&mut scratch, a, b, c, d), Ok(Sign::Zero), "{p:?}");
            let [a, b, c, d] = p.map(|q| Lnc::new_unchecked(q, q, 0.625));
            assert_eq!(stages::staged_in(&mut scratch, &a, &b, &c, &d), Ok(Sign::Zero), "{p:?}");
        }
    }
}

#[test]
fn outside_the_range_over_spans_the_chain_falls_to_the_big_integers() {
    // Above the top of `SPANS_TRANSLATED_RANGE` and `DIFFERENCES_RANGE`, and
    // below their bottom, with offsets that make the differences inexact:
    // the filter declines points this close to a circle, the stages on
    // expansions decline the magnitudes, and the stage on big integers
    // decides.
    let (min, max) = stages::SPANS_TRANSLATED_RANGE;
    assert!(min / 4096.0 < stages::DIFFERENCES_RANGE.0 && max * 256.0 > stages::DIFFERENCES_RANGE.1);
    let mut buffer = vec![0.0; 1 << 16];
    let mut scratch = Scratch::borrowed(&mut buffer);
    let (mut explicit, mut implicit) = (0, 0);
    let mut signs = [0; 3];
    for scale in [max * 256.0, min / 4096.0] {
        for offset in [0.0, scale / 3.0, scale * 0.1] {
            for skip in 0..6 {
                for nudge in [0, 1, -1] {
                    let p = on_circle(scale, offset, skip, nudge);
                    let expected = oracle(exact(&p));
                    let r = p.each_ref();
                    if stages::staged(&mut scratch, r) == Ok(None) {
                        assert_eq!(stages::exact_stages_explicit_in(&mut scratch, r), Ok(expected), "{p:?}");
                        let [a, b, c, d] = &p;
                        assert_eq!(stages::staged_in(&mut scratch, a, b, c, d), Ok(expected), "{p:?}");
                        explicit += 1;
                        signs[(i8::from(expected) + 1) as usize] += 1;
                    }
                    let l = p.map(|q| Lnc::new_unchecked(q, q, 0.625));
                    let mark = scratch.mark();
                    let q = l.each_ref().map(|x| x.exact(&mut scratch).unwrap());
                    let declined = stages::expansions_translated_in(&mut scratch, q.each_ref()) == Ok(None);
                    scratch.reset(mark);
                    if declined && stages::filter_points(&l[0], &l[1], &l[2], &l[3]).is_none() {
                        let [a, b, c, d] = &l;
                        assert_eq!(stages::exact_stages_in(&mut scratch, a, b, c, d), Ok(expected), "{p:?}");
                        assert_eq!(stages::staged_in(&mut scratch, a, b, c, d), Ok(expected), "{p:?}");
                        implicit += 1;
                    }
                    assert_eq!(scratch.used(), 0);
                }
            }
        }
    }
    assert!(explicit > 0 && implicit > 0, "{explicit} {implicit}");
    assert!(signs.iter().all(|&n| n > 0), "{signs:?}");
}

/// The point `at`, which breaks the contract of `Point` as `lie` says: with
/// 0, its error bounds are negative; with 1, it releases the spans of its
/// exact coordinates before it returns them, so that they lie above the top
/// of the arena; with 2, it does the same with its coordinates as big
/// integers.
struct Liar {
    at: [f64; 3],
    lie: u8,
}

impl Point for Liar {
    fn approx(&self) -> &[f64; 3] {
        &self.at
    }

    fn err(&self) -> &[f64; 3] {
        if self.lie == 0 { &[-1.0; 3] } else { &[0.0; 3] }
    }

    fn is_explicit(&self) -> bool {
        false
    }

    fn exact(&self, scratch: &mut Scratch<'_>) -> Result<ExactCoords, ExactError> {
        let mark = scratch.mark();
        let c = self.at.exact(scratch)?;
        if self.lie == 1 {
            scratch.reset(mark);
        }
        Ok(c)
    }

    fn exact_big(&self, scratch: &mut Scratch<'_>) -> Result<BigCoords, ExactError> {
        let mark = scratch.mark();
        let c = self.at.exact_big(scratch)?;
        if self.lie == 2 {
            scratch.reset(mark);
        }
        Ok(c)
    }
}

#[test]
fn a_point_that_breaks_the_contract_panics_in_debug_builds() {
    // Concyclic points, which the filter declines; the large ones are out
    // of the range of the stage over spans, so they reach the stage on big
    // integers.
    let large = stages::SPANS_TRANSLATED_RANGE.1 * 256.0;
    for (lie, scale, obligation) in [
        (0, 1.0, "an error bound is NaN or negative"),
        (1, 1.0, "is not a valid expansion"),
        (2, large, "as a big integer is not in its unique representation"),
    ] {
        let p = on_circle(scale, 0.0, 0, 0);
        let liar = Liar { at: p[3], lie };
        let mut buffer = vec![0.0; 1 << 16];
        let mut scratch = Scratch::borrowed(&mut buffer);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            stages::staged_in(&mut scratch, &p[0], &p[1], &p[2], &liar)
        }));
        // The emitted crate is tested in the dev profile, with debug
        // assertions.
        let payload = result.expect_err("no panic");
        let message = payload.downcast_ref::<String>().expect("a message");
        assert!(message.starts_with("breaks the contract of `Point`: argument 3: "), "{message}");
        assert!(message.contains(obligation), "{lie}: {message}");
    }
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

/// A form of degree 2 that is not multihomogeneous, with the degree vectors
/// (2, 0, 0) and (0, 1, 1), so that its filter is the global one and its
/// second test, `filter_axes`, has no certain zero: no degree vector has a
/// positive degree in every axis.
const ZERO_IN_AN_AXIS: &str = "fn p(a: Point3, b: Point3, c: Point3) -> Sign {\n    \
                               (a.x - b.x) * (a.x - c.x) + (a.y - b.y) * (a.z - c.z)\n}\n";

/// The test of [`ZERO_IN_AN_AXIS`]: its chain and its two tests of the
/// filter against the form written by hand on exact dyadic numbers, on
/// points of every scale and on points whose box is much shorter on one axis
/// than on the others, which `filter_axes` decides when `filter` does not;
/// its emitted checks; the ends of the range of `filter_axes` on each axis;
/// and inputs that overflow, are not finite or have an uncertainty that is
/// not finite and non-negative, which it never decides.
const ZERO_IN_AN_AXIS_TEST: &str = r#"use implicit_predicates::{Lnc, Scratch, Sign};
use implicit_predicates_oracle::Dyadic;

/// The sign of `(a.x - b.x) (a.x - c.x) + (a.y - b.y) (a.z - c.z)` at the
/// exact points `p`.
fn oracle(p: [[Dyadic; 3]; 3]) -> Sign {
    let [a, b, c] = p;
    let value = &(&(&a[0] - &b[0]) * &(&a[0] - &c[0])) + &(&(&a[1] - &b[1]) * &(&a[2] - &c[2]));
    Sign::from(value.sign().cmp(&0))
}

fn exact(p: &[[f64; 3]; 3]) -> [[Dyadic; 3]; 3] {
    p.map(|q| q.map(Dyadic::from_f64))
}

const VALUES: [f64; 9] = [0.0, 1.0, -3.0, 0.1, 1e-300, 1e300, -5e-324, 3.0000000000000004, 1e-200];

/// Points whose differences are `x` and `x + 2^-30 x` on x, `y` on y and
/// `z` on z.
fn shaped(x: f64, y: f64, z: f64) -> [[f64; 3]; 3] {
    [[x, y, z], [0.0, 0.0, 0.5], [-x * f64::powi(2.0, -30), 0.5, 0.0]]
}

/// Every way of running the chain or a stage of it on `p` agrees with the
/// oracle; returns whether `filter_axes` decides it and `filter` does not.
fn check(scratch: &mut Scratch<'_>, p: &[[f64; 3]; 3]) -> (Sign, bool) {
    let expected = oracle(exact(p));
    let r = p.each_ref();
    let axes = stages::filter_axes(r, [0.0; 3]);
    assert!(axes.is_none_or(|s| s == expected && s != Sign::Zero), "{p:?}");
    let filter = stages::filter(r, 0.0);
    assert!(filter.is_none_or(|s| s == expected), "{p:?}");
    assert!(stages::staged(r).is_none_or(|s| s == expected), "{p:?}");
    let [a, b, c] = p;
    assert_eq!(stages::filter_axes_points(a, b, c), axes, "{p:?}");
    assert_eq!(stages::staged_in(scratch, a, b, c), Ok(expected), "{p:?}");
    assert_eq!(stages::exact_stages_explicit_in(scratch, r), Ok(expected), "{p:?}");
    assert_eq!(stages::exact_stages_in(scratch, a, b, c), Ok(expected), "{p:?}");
    // Points on a line whose parents are both the point: exact, but not
    // explicit, so their uncertainty is computed from their errors.
    let [a, b, c] = p.map(|q| Lnc::new_unchecked(q, q, 0.625));
    assert_eq!(stages::staged_in(scratch, &a, &b, &c), Ok(expected), "{p:?}");
    assert_eq!(stages::exact_stages_in(scratch, &a, &b, &c), Ok(expected), "{p:?}");
    if let Some(sign) = stages::filter_axes_points(&a, &b, &c) {
        assert_eq!(sign, expected, "{p:?}");
    }
    assert_eq!(scratch.used(), 0);
    (expected, filter.is_none() && axes.is_some())
}

#[test]
fn the_chain_has_the_sign_of_the_oracle() {
    let mut buffer = vec![0.0; 1 << 16];
    let mut scratch = Scratch::borrowed(&mut buffer);
    let mut signs = [0; 3];
    for &x in &VALUES {
        for &y in &VALUES {
            for &z in &VALUES {
                for p in [[[x, y, 0.5], [0.25, -0.5, z], [1.0, z, y]], shaped(x, y, z)] {
                    let (sign, _) = check(&mut scratch, &p);
                    signs[(i8::from(sign) + 1) as usize] += 1;
                }
            }
        }
    }
    assert!(signs.iter().all(|&n| n > 0), "{signs:?}");
    // A box much shorter on x than on z: the value, about `x^2 + y z`, is far
    // below the global bound, of order `u z^2`, and above the bound by parts.
    let mut only_axes = 0;
    for scale in [1.0, 1e-100, 1e100] {
        for (x, y) in [(1e-10, 1e-20), (-1e-10, 1e-20), (1e-12, -1e-30), (3e-9, 1e-18)] {
            let (_, decided) = check(&mut scratch, &shaped(x * scale, y * scale, scale));
            only_axes += usize::from(decided);
        }
    }
    assert!(only_axes >= 6, "{only_axes}");
    let [decided, _] = checks::check_bound_edge_cases(|p| stages::staged(p.each_ref()));
    assert!(decided > 0);
    let counts = checks::check_axes_edge_cases(|p, e| stages::filter_axes(p.each_ref(), e));
    for [decided, declined] in counts {
        assert!(decided > 0 && declined > 0, "{counts:?}");
    }
}

/// Points whose leaves are all `v` on axis `k` and 1 on the other two
/// axes, for `v > 0`: the value, `v^2 + 1` or `1 + v`, is positive.
fn at_size(k: usize, v: f64) -> [[f64; 3]; 3] {
    let mut p = [[1.0, 1.0, 1.0], [0.0, 0.0, 0.5], [0.0, 0.5, 0.0]];
    p[0][k] = v;
    p
}

#[test]
fn the_range_of_filter_axes_is_checked_on_every_axis() {
    let (low, high) = (stages::AXES_D_MIN, stages::AXES_D_MAX);
    assert!(low < 1.0 && 1.0 < high);
    for k in 0..3 {
        for (v, decides) in [
            (low, true),
            (low.next_up(), true),
            (low.next_down(), false),
            (high, true),
            (high.next_down(), true),
            (high.next_up(), false),
        ] {
            let p = at_size(k, v);
            let expected = oracle(exact(&p));
            assert_eq!(expected, Sign::Positive, "{k} {v:e}");
            let decision = stages::filter_axes(p.each_ref(), [0.0; 3]);
            assert_eq!(decision, decides.then_some(expected), "axis {k}, size {v:e}: {p:?}");
            // An uncertainty that takes the size of that axis out of the
            // range.
            let mut e = [0.0; 3];
            e[k] = f64::MAX;
            assert_eq!(stages::filter_axes(p.each_ref(), e), None, "{k} {v:e}");
        }
    }
}

#[test]
fn filter_axes_never_returns_zero() {
    // Flat axes and exact zeros: the form has a degree vector of degree zero
    // in each axis, so a flat axis does not make it zero, and nothing does
    // with certainty.
    let flat = [
        [[0.0; 3]; 3],
        [[1.0, 2.0, 3.0], [1.0, 2.0, 5.0], [1.0, 7.0, 3.0]],
        [[1.0, 2.0, 3.0], [1.0, 5.0, 3.0], [4.0, 2.0, 3.0]],
        [[2.0, 1.0, 1.0], [2.0, 1.0, 0.0], [5.0, 0.0, 1.0]],
    ];
    for p in flat {
        for e in [[0.0; 3], [0.0, 0.25, 0.0]] {
            let decision = stages::filter_axes(p.each_ref(), e);
            assert_ne!(decision, Some(Sign::Zero), "{p:?} {e:?}");
            if e == [0.0; 3] {
                assert!(decision.is_none_or(|s| s == oracle(exact(&p))), "{p:?}");
            }
        }
    }
}

#[test]
fn an_overflow_or_an_input_that_is_not_finite_never_decides() {
    let big = 1e200;
    let normal = [[3.0, 1.0, 2.0], [0.5, -1.0, 0.25], [1.0, 0.0, -2.0]];
    assert!(stages::filter_axes(normal.each_ref(), [0.0; 3]).is_some());
    let cases = [
        // With `e[k] = 0.0`: `x^2` overflows, so do the partial products of
        // the term of epsilon of x, and the value is infinite.
        ([[big, 1.0, 1.0], [0.0, 0.0, 0.5], [0.0, 0.5, 0.0]], [0.0; 3]),
        // Products of opposite signs that overflow: the value is NaN.
        ([[big, big, big], [0.0, 0.0, 0.0], [2.0 * big, 0.0, 0.0]], [0.0; 3]),
        // Infinite sizes: the terms of kappa multiply `e[k] = 0.0` by them.
        ([[f64::INFINITY, 1.0, 1.0], [0.0, 0.0, 0.5], [0.0, 0.5, 0.0]], [0.0; 3]),
        ([[1.0, f64::INFINITY, 1.0], [0.0, 0.0, 0.5], [0.0, 0.5, 0.0]], [0.0; 3]),
        // A NaN coordinate, which the maxima can skip.
        ([[3.0, 1.0, 2.0], [f64::NAN, -1.0, 0.25], [1.0, 0.0, -2.0]], [0.0; 3]),
        // Uncertainties that are not finite and non-negative.
        (normal, [f64::NAN, 0.0, 0.0]),
        (normal, [0.0, -1e-300, 0.0]),
        (normal, [0.0, 0.0, f64::INFINITY]),
        (normal, [0.0, 0.0, f64::MAX]),
    ];
    for (p, e) in cases {
        assert_eq!(stages::filter_axes(p.each_ref(), e), None, "{p:?} {e:?}");
    }
}
"#;

#[test]
fn a_form_that_is_not_multihomogeneous_has_a_second_test_with_one_size_per_axis() {
    let modules = predicate_modules(ZERO_IN_AN_AXIS);
    let stages = &modules[0].1;
    // `filter_axes` right after `filter` in each function of the chain.
    for (function, text) in [
        (
            "staged",
            "-> Option<Sign> {\nfilter(p, 0.0)\n.or_else(|| filter_axes(p, [0.0; 3]))\n.or_else(|| exact_differences(p))",
        ),
        (
            "exact_stages",
            "-> Option<Sign> {\nfilter_axes(p, [0.0; 3])\n.or_else(|| exact_differences(p))",
        ),
        (
            "exact_stages_in",
            "}\nif let Some(sign) = filter_axes_points(p0, p1, p2) {\nreturn Ok(sign);\n}\nlet mark",
        ),
    ] {
        assert!(stages.contains(text), "{function}: {stages}");
    }
    let start = stages.find("pub fn filter_axes(").unwrap();
    let filter_axes = &stages[start..][..stages[start..].find("\n}\n").unwrap()];
    assert!(!filter_axes.contains("Sign::Zero"), "{filter_axes}");
    compile_and_run("emitted-zero-in-an-axis", &modules, ZERO_IN_AN_AXIS_TEST);
}

/// A form of degree 4 that is not multihomogeneous and has positive degree
/// in every axis in each of its degree vectors, (2, 1, 1) and (1, 2, 1), so
/// that `filter_axes` has the certain zero. Every product starts with a
/// difference on x, so with x flat the computed value is exactly zero even
/// when a factor on y is huge.
const CERTAIN_ZERO: &str = "fn q(a: Point3, b: Point3, c: Point3) -> Sign {\n    \
                            (a.x - b.x) * (a.x - b.x) * (a.y - b.y) * (a.z - b.z) + \
                            (a.x - b.x) * (a.y - c.y) * (a.y - c.y) * (a.z - c.z)\n}\n";

/// The test of [`CERTAIN_ZERO`]: `filter_axes` decides zero on a flat axis
/// with every guard met, and declines when one fails: a size that is not
/// finite, a NaN coordinate, an uncertainty on the flat axis, one that is not
/// finite and non-negative.
const CERTAIN_ZERO_TEST: &str = r#"use implicit_predicates::Sign;

#[test]
fn filter_axes_decides_zero_only_under_its_guards() {
    // x is flat: a.x == b.x, and every term has a factor a.x - b.x.
    let flat = [[2.0, 1.0, 3.0], [2.0, -1.0, 0.5], [7.0, 0.25, 1.0]];
    assert_eq!(stages::filter_axes(flat.each_ref(), [0.0; 3]), Some(Sign::Zero));
    assert_eq!(stages::filter_axes(flat.each_ref(), [0.0, 0.5, 0.25]), Some(Sign::Zero));
    assert_eq!(stages::staged(flat.each_ref()), Some(Sign::Zero));
    // A size on y that overflows, with a finite uncertainty, while the
    // computed value is still exactly zero: `d_y` is not finite.
    let huge = [[2.0, 1e300, 3.0], [2.0, 0.0, 0.5], [7.0, 0.0, 1.0]];
    assert_eq!(1e300 + f64::MAX, f64::INFINITY);
    assert_eq!(stages::filter_axes(huge.each_ref(), [0.0, f64::MAX, 0.0]), None);
    assert_eq!(stages::filter_axes(huge.each_ref(), [0.0; 3]), Some(Sign::Zero));
    // A NaN coordinate, which the maxima can skip, makes the value NaN.
    let nan = [[2.0, 1.0, 3.0], [2.0, f64::NAN, 0.5], [7.0, 0.25, 1.0]];
    assert_eq!(stages::filter_axes(nan.each_ref(), [0.0; 3]), None);
    // An uncertainty on the flat axis: the axis is not flat any more, and
    // the value, zero, is not above the bound.
    assert_eq!(stages::filter_axes(flat.each_ref(), [0.125, 0.0, 0.0]), None);
    // Uncertainties that are not finite and non-negative, on another axis.
    for bad in [-1e-300, f64::NAN, f64::INFINITY] {
        assert_eq!(stages::filter_axes(flat.each_ref(), [0.0, bad, 0.0]), None, "{bad}");
        assert_eq!(stages::filter_axes(flat.each_ref(), [0.0, 0.0, bad]), None, "{bad}");
    }
    // `filter_axes_points` with no uncertainty, on explicit points.
    let [a, b, c] = &flat;
    assert_eq!(stages::filter_axes_points(a, b, c), Some(Sign::Zero));
}
"#;

#[test]
fn the_certain_zero_of_filter_axes_has_every_guard() {
    let modules = predicate_modules(CERTAIN_ZERO);
    assert!(
        modules[0]
            .1
            .contains("let zero = valid & finite & flat & (det == 0.0);")
    );
    compile_and_run("emitted-certain-zero", &modules, CERTAIN_ZERO_TEST);
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

#[test]
fn the_changelog_quotes_the_degree_of_big_integers_alone() {
    // The CHANGELOG is not in the package of the generator, so this file,
    // which reads the workspace, checks it.
    let text = fs::read_to_string(workspace().join("CHANGELOG.md")).unwrap();
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let quoted =
        format!("a total degree of {BIG_INTEGERS_ONLY_DEGREE} or more (`BIG_INTEGERS_ONLY_DEGREE`");
    assert!(text.contains(&quoted), "{quoted}");
}
