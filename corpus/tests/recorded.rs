//! End-to-end tests of the recording build of the reference mesher.
//!
//! They need the executables built by `python3 corpus/tools/corpus.py build`
//! and the example models downloaded by `fetch-reference`, so they are
//! ignored by default. Run them with
//! `cargo test -p indirect-predicates-corpus -- --ignored recorded`.

use indirect_predicates_corpus::verify::verify_with;
use indirect_predicates_corpus::{Corpus, Predicate};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// The stratum cap used by the capped run.
const SMALL_CAP: u64 = 10;

const BUILD_HINT: &str = "run `python3 corpus/tools/corpus.py build`";
const FETCH_HINT: &str = "run `python3 corpus/tools/corpus.py fetch-reference`";

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

fn executable(build: &str) -> PathBuf {
    let path = data_dir().join("build").join(build).join("cdt");
    assert!(
        path.is_file(),
        "{} is missing: {BUILD_HINT}",
        path.display()
    );
    path
}

fn models() -> Vec<PathBuf> {
    let dir = data_dir().join("reference/cdt-legacy/Input_file");
    let entries =
        std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}: {FETCH_HINT}", dir.display()));
    let mut models: Vec<PathBuf> = entries
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "off"))
        .collect();
    models.sort();
    assert!(
        !models.is_empty(),
        "no models in {}: {FETCH_HINT}",
        dir.display()
    );
    models
}

/// A scratch directory under `corpus/data/`, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = data_dir().join(format!("scratch/recorded-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Runs `cdt` on a fresh copy of `model` in `dir` and returns the `.tet`
/// output and, for a recording run, the corpus file.
fn run(cdt: &Path, model: &Path, dir: &Path, env: &[(&str, String)]) -> (Vec<u8>, Option<Vec<u8>>) {
    std::fs::create_dir_all(dir).unwrap();
    let name = model.file_name().unwrap().to_str().unwrap();
    let input = dir.join(name);
    std::fs::copy(model, &input).unwrap();
    let corpus = dir.join("calls.ipcorpus");
    let mut command = Command::new(cdt);
    command.arg(&input).current_dir(dir);
    for (key, value) in env {
        command.env(key, value);
    }
    let recording = !env.is_empty();
    if recording {
        command
            .env("IPCORPUS_OUT", &corpus)
            .env("IPCORPUS_MODEL", name);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{} failed on {name}: {}",
        cdt.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let tet = std::fs::read(dir.join(format!("{name}.tet"))).unwrap();
    assert!(!dir.join("calls.ipcorpus.partial").exists());
    let corpus = recording.then(|| std::fs::read(&corpus).unwrap());
    (tet, corpus)
}

/// The outputs of every run on one model.
struct ModelRuns {
    name: String,
    original_tet: Vec<u8>,
    recorded_tet: Vec<u8>,
    corpus: Vec<u8>,
    repeated_corpus: Vec<u8>,
    capped_corpus: Vec<u8>,
}

fn runs() -> &'static [ModelRuns] {
    static RUNS: OnceLock<Vec<ModelRuns>> = OnceLock::new();
    RUNS.get_or_init(|| {
        let original = executable("cdt-original");
        let recording = executable("cdt-recording");
        let scratch = Scratch::new("models");
        let uncapped = [("IPCORPUS_STRATUM_CAP", "100000".to_string())];
        let capped = [("IPCORPUS_STRATUM_CAP", SMALL_CAP.to_string())];
        models()
            .iter()
            .map(|model| {
                let name = model.file_name().unwrap().to_str().unwrap().to_string();
                let dir = scratch.0.join(&name);
                let (original_tet, _) = run(&original, model, &dir.join("original"), &[]);
                let (recorded_tet, corpus) = run(&recording, model, &dir.join("first"), &uncapped);
                let (_, repeated) = run(&recording, model, &dir.join("second"), &uncapped);
                let (_, capped_corpus) = run(&recording, model, &dir.join("capped"), &capped);
                ModelRuns {
                    name,
                    original_tet,
                    recorded_tet,
                    corpus: corpus.unwrap(),
                    repeated_corpus: repeated.unwrap(),
                    capped_corpus: capped_corpus.unwrap(),
                }
            })
            .collect()
    })
}

fn read(bytes: &[u8], name: &str) -> Corpus {
    Corpus::read(bytes).unwrap_or_else(|e| panic!("{name}: the corpus is rejected: {e}"))
}

#[test]
#[ignore = "needs the reference mesher built by corpus/tools/corpus.py"]
fn recorded_mesher_writes_the_same_tet_files() {
    for r in runs() {
        assert!(
            r.original_tet == r.recorded_tet,
            "{}: the .tet files differ",
            r.name
        );
    }
}

#[test]
#[ignore = "needs the reference mesher built by corpus/tools/corpus.py"]
fn recorded_corpora_are_valid() {
    for r in runs() {
        let corpus = read(&r.corpus, &r.name);
        assert_eq!(corpus.model, r.name);
        assert!(!corpus.calls.is_empty(), "{}: no calls recorded", r.name);
        read(&r.capped_corpus, &r.name);
    }
}

/// Describes a call with the coordinates of its points, for failure messages.
fn describe(corpus: &Corpus, call: usize) -> String {
    let c = &corpus.calls[call];
    let points: Vec<String> = c
        .points()
        .iter()
        .map(|&p| format!("{:?}", corpus.points[p as usize]))
        .collect();
    format!(
        "call {call} {:?} sign {} stage {:?}, points {points:?}",
        c.predicate, c.sign, c.stage
    )
}

#[test]
#[ignore = "needs the reference mesher built by corpus/tools/corpus.py"]
fn recorded_signs_match_the_exact_signs_up_to_a_uniform_factor() {
    // A factor fits a predicate when `verify_with` finds no mismatch of that
    // predicate in any model; exactly one of +1 and -1 must fit each.
    let predicates = [Predicate::Orient3d, Predicate::InSphere];
    let mut failures: BTreeMap<(Predicate, i8), String> = BTreeMap::new();
    for r in runs() {
        let corpus = read(&r.corpus, &r.name);
        for factor in [1, -1] {
            let v = verify_with(&corpus, factor, factor);
            for m in &v.mismatches {
                let predicate = corpus.calls[m.call].predicate;
                failures.entry((predicate, factor)).or_insert_with(|| {
                    format!(
                        "{}: {}, exact {}",
                        r.name,
                        describe(&corpus, m.call),
                        m.exact
                    )
                });
            }
        }
    }
    for predicate in predicates {
        let fitting: Vec<i8> = [1, -1]
            .into_iter()
            .filter(|&f| !failures.contains_key(&(predicate, f)))
            .collect();
        assert_eq!(
            fitting.len(),
            1,
            "{predicate:?}: factors that fit {fitting:?}; first mismatch per factor: {:?}",
            [1, -1].map(|f| failures.get(&(predicate, f)))
        );
        eprintln!("{predicate:?}: recorded sign = {} * exact sign", fitting[0]);
    }
}

#[test]
#[ignore = "needs the reference mesher built by corpus/tools/corpus.py"]
fn recorded_cap_bounds_kept_calls_and_keeps_totals() {
    for r in runs() {
        let full = read(&r.corpus, &r.name);
        let capped = read(&r.capped_corpus, &r.name);
        assert_eq!(
            capped.counters, full.counters,
            "{}: counters changed",
            r.name
        );
        assert_eq!(
            capped.sampling.keys().collect::<Vec<_>>(),
            full.sampling.keys().collect::<Vec<_>>()
        );
        for (stratum, s) in &capped.sampling {
            assert_eq!(
                s.total, full.sampling[stratum].total,
                "{}: {stratum:?}",
                r.name
            );
            assert_eq!(s.kept, s.total.min(SMALL_CAP), "{}: {stratum:?}", r.name);
        }
    }
}

#[test]
#[ignore = "needs the reference mesher built by corpus/tools/corpus.py"]
fn recorded_runs_are_deterministic() {
    for r in runs() {
        assert!(r.corpus == r.repeated_corpus, "{}: two runs differ", r.name);
    }
}

/// Four coplanar vertices: the mesher evaluates orient3d on them, finds no
/// volume and reports an error through `ip_error`, which exits with status 0.
const FLAT_MODEL: &str = "OFF\n4 2 0\n0 0 0\n1 0 0\n1 1 0\n0 1 0\n3 0 1 2\n3 0 2 3\n";

#[test]
#[ignore = "needs the reference mesher built by corpus/tools/corpus.py"]
fn recorded_run_that_reports_an_error_leaves_no_corpus() {
    let recording = executable("cdt-recording");
    let scratch = Scratch::new("flat");
    let model = scratch.0.join("flat.off");
    std::fs::write(&model, FLAT_MODEL).unwrap();
    let out = scratch.0.join("calls.ipcorpus");
    let partial = scratch.0.join("calls.ipcorpus.partial");
    std::fs::write(&out, b"stale").unwrap();
    std::fs::write(&partial, b"stale").unwrap();
    let output = Command::new(recording)
        .arg(&model)
        .current_dir(&scratch.0)
        .env("IPCORPUS_OUT", &out)
        .env("IPCORPUS_MODEL", "flat")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(stderr.contains("do not define a volume"), "{stderr}");
    let calls: u64 = stderr
        .split("recording abandoned after ")
        .nth(1)
        .and_then(|rest| rest.split(' ').next())
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("no abandon message: {stderr}"));
    assert!(calls > 0, "the error came before any predicate call");
    assert!(!out.exists() && !partial.exists());
}
