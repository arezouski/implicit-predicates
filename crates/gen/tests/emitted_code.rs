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

use implicit_predicates_gen::{generate, generate_points};

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
