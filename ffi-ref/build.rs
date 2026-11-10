//! Builds each reference library version, with the shim that gives it a C
//! interface, into its own shared library, and checks that the two export
//! nothing but their own C interface.
//!
//! Nothing is downloaded here: the reference sources must already be in
//! `IPREF_REFERENCE_DIR` (default `../corpus/data/reference`).

use std::collections::BTreeSet;
use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, exit};

const SHIM_FILES: [&str; 5] = [
    "shim/ipref.h",
    "shim/guard.hpp",
    "shim/point_store.hpp",
    "shim/cdt_legacy.cpp",
    "shim/ip_current.cpp",
];

/// One library version to build.
struct Version {
    /// Prefix word of its C interface and library name.
    name: &'static str,
    /// Our shim source, relative to the manifest directory.
    shim: &'static str,
    /// Include directories, relative to the reference directory.
    includes: &'static [&'static str],
    /// The compile options of the library's own CMake build for x86_64 with
    /// AVX2 (its default), in the order CMake passes them for a Release
    /// build.
    options: &'static [&'static str],
}

const VERSIONS: [Version; 2] = [
    Version {
        name: "legacy",
        shim: "shim/cdt_legacy.cpp",
        includes: &["cdt-legacy/include"],
        options: &[
            "-O3",
            "-DNDEBUG",
            "-std=gnu++20",
            "-O2",
            "-frounding-math",
            "-mavx2",
            "-mfma",
        ],
    },
    Version {
        name: "current",
        shim: "shim/ip_current.cpp",
        includes: &[
            "indirect-predicates",
            "indirect-predicates/include",
            "nfg/include",
        ],
        options: &[
            "-O3",
            "-DNDEBUG",
            "-std=gnu++20",
            "-O3",
            "-frounding-math",
            "-ffp-contract=off",
            "-mavx2",
            "-mfma",
        ],
    },
];

/// Options of our own, needed to build a shared library whose only exported
/// symbols are its C interface. They do not change the generated arithmetic.
const LIBRARY_OPTIONS: [&str; 5] = [
    "-fPIC",
    "-fvisibility=hidden",
    "-fvisibility-inlines-hidden",
    "-shared",
    "-Wl,-z,defs",
];

/// Files whose presence shows that a reference checkout is there. Their
/// directories hold every header the libraries include, and are watched
/// for changes.
const REFERENCE_MARKERS: [&str; 3] = [
    "cdt-legacy/include/implicit_point.h",
    "indirect-predicates/include/implicit_point.h",
    "nfg/include/numerics.h",
];

fn fail(message: &str) -> ! {
    eprintln!("error: {message}");
    exit(1);
}

fn reference_dir(manifest: &Path) -> PathBuf {
    println!("cargo:rerun-if-env-changed=IPREF_REFERENCE_DIR");
    let dir = env::var_os("IPREF_REFERENCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("../corpus/data/reference"));
    let missing: Vec<_> = REFERENCE_MARKERS
        .iter()
        .map(|marker| dir.join(marker))
        .filter(|path| !path.is_file())
        .collect();
    if !missing.is_empty() {
        let list: Vec<_> = missing.iter().map(|p| p.display().to_string()).collect();
        fail(&format!(
            "reference sources not found; run: python3 corpus/tools/corpus.py fetch-reference\n\
             (cdt-legacy, indirect-predicates and nfg are expected in {}; missing: {})",
            dir.display(),
            list.join(", ")
        ));
    }
    for marker in REFERENCE_MARKERS {
        let headers = dir.join(marker).parent().unwrap().to_path_buf();
        println!("cargo:rerun-if-changed={}", headers.display());
    }
    dir
}

/// Compiles and links one version, returning the path of its library.
fn build(
    version: &Version,
    compiler: &Path,
    manifest: &Path,
    reference: &Path,
    out: &Path,
) -> PathBuf {
    let script = out.join(format!("ipref_{}.map", version.name));
    let script_text = format!(
        "{{\n  global: ipref_{}_*;\n  local: *;\n}};\n",
        version.name
    );
    std::fs::write(&script, script_text)
        .unwrap_or_else(|e| fail(&format!("{}: {e}", script.display())));
    let library = out.join(format!("libipref_{}.so", version.name));

    let mut command = Command::new(compiler);
    command.args(version.options).args(LIBRARY_OPTIONS);
    command.arg(format!("-Wl,--version-script={}", script.display()));
    command.arg(format!("-I{}", manifest.join("shim").display()));
    for include in version.includes {
        command.arg(format!("-I{}", reference.join(include).display()));
    }
    command
        .arg(manifest.join(version.shim))
        .arg("-o")
        .arg(&library);
    println!(
        "cargo:warning=ipref {}: {} {}",
        version.name,
        compiler.display(),
        version.options.join(" ")
    );
    let status = command
        .status()
        .unwrap_or_else(|e| fail(&format!("cannot run {}: {e}", compiler.display())));
    if !status.success() {
        fail(&format!(
            "building the {} library failed: {command:?}",
            version.name
        ));
    }
    library
}

/// The names of the symbols a shared library defines in its dynamic symbol
/// table: the only symbols the dynamic linker can bind across libraries.
fn exported_symbols(library: &Path) -> BTreeSet<String> {
    let output = Command::new("nm")
        .args(["-D", "--defined-only"])
        .arg(library)
        .output()
        .unwrap_or_else(|e| fail(&format!("cannot run nm: {e}")));
    if !output.status.success() {
        fail(&format!("nm failed on {}", library.display()));
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split_whitespace().nth(2))
        .map(str::to_owned)
        .collect()
}

/// The functions of the C interface of each version, after its prefix
/// `ipref_<version>_`: six for the bindings, one benchmark hook and three
/// test hooks.
const INTERFACE: [&str; 10] = [
    "points_new",
    "points_free",
    "explicit",
    "lnc",
    "orient3d",
    "insphere",
    "bench_overhead",
    "test_error_routine",
    "test_return_rounding_upward",
    "test_enter_rounding_upward",
];

/// Fails unless every library exports exactly the functions of its own C
/// interface, which also means no symbol is exported by two of them.
fn check_isolation(libraries: &[(&Version, PathBuf)]) {
    let mut seen = BTreeSet::new();
    for (version, library) in libraries {
        let expected: BTreeSet<String> = INTERFACE
            .iter()
            .map(|function| format!("ipref_{}_{function}", version.name))
            .collect();
        let symbols = exported_symbols(library);
        if symbols != expected {
            let extra: Vec<_> = symbols.difference(&expected).collect();
            let missing: Vec<_> = expected.difference(&symbols).collect();
            fail(&format!(
                "{} does not export exactly its C interface: extra {extra:?}, missing {missing:?}",
                library.display()
            ));
        }
        let shared: Vec<_> = symbols.intersection(&seen).collect();
        if !shared.is_empty() {
            fail(&format!("symbols defined by both libraries: {shared:?}"));
        }
        seen.extend(symbols);
    }
}

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    println!("cargo:rerun-if-changed=build.rs");
    for file in SHIM_FILES {
        println!("cargo:rerun-if-changed={file}");
    }
    if env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("x86_64")
        || env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux")
    {
        fail("the reference libraries are built for x86_64 Linux only");
    }
    let reference = reference_dir(&manifest);
    // The cc crate only finds the C++ compiler (honouring CXX); the options
    // are exactly the ones listed above.
    let compiler = cc::Build::new()
        .cpp(true)
        .get_compiler()
        .path()
        .to_path_buf();

    let libraries: Vec<_> = VERSIONS
        .iter()
        .map(|version| {
            (
                version,
                build(version, &compiler, &manifest, &reference, &out),
            )
        })
        .collect();
    check_isolation(&libraries);

    println!("cargo:rustc-link-search=native={}", out.display());
    for version in &VERSIONS {
        println!("cargo:rustc-link-lib=dylib=ipref_{}", version.name);
    }
    // Tests, benchmarks and examples of this package find the libraries
    // without LD_LIBRARY_PATH.
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", out.display());
}
