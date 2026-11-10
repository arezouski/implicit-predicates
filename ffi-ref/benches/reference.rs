//! The two reference libraries on recorded predicate calls.
//!
//! For each version, the groups are:
//!
//! - `<version>/orient3d` and `<version>/insphere`: every recorded call of
//!   the predicate, by number of implicit points (`0`, `1`, `2`, `3+`), on
//!   points built beforehand;
//! - `<version>/overhead`: a call that does everything an orient3d call
//!   does except evaluating the predicate (the C interface, its guard and
//!   the index lookups), on the same points as the orient3d calls;
//! - `<version>/construct`: building the explicit and LNC points of each
//!   corpus, without freeing them;
//! - `<version>/replay`: building the points of each corpus and evaluating
//!   all of its recorded calls.
//!
//! The throughput in elements per second is the number of calls per second
//! (points per second for `construct`), so the time per call is its
//! inverse. The inputs are the versioned sample corpora, or the corpus files
//! of the directory named by `IPREF_CORPUS_DIR`.

use std::hint::black_box;
use std::path::{Path, PathBuf};

use criterion::{BenchmarkGroup, Criterion, SamplingMode, Throughput, measurement::WallTime};
use indirect_predicates_corpus::{Call, Corpus, PointDef, Predicate};
use indirect_predicates_ffi_ref::{current, legacy};

/// The environment variable naming a directory of corpus files to use
/// instead of the versioned samples.
const CORPUS_DIR: &str = "IPREF_CORPUS_DIR";

/// The implicit point count buckets; the last one holds 3 or more.
const BUCKETS: [&str; 4] = ["0", "1", "2", "3+"];

/// One reference library version, as the benchmarks use it.
trait Reference {
    const NAME: &'static str;
    type Points;
    type Id: Copy;
    fn from_corpus(corpus: &Corpus) -> Built<Self>;
    fn orient3d(points: &Self::Points, p: [Self::Id; 4]) -> i8;
    fn insphere(points: &Self::Points, p: [Self::Id; 5]) -> i8;
    fn overhead(points: &Self::Points, p: [Self::Id; 4]) -> i8;
}

macro_rules! reference {
    ($marker:ident, $module:ident) => {
        enum $marker {}

        impl Reference for $marker {
            const NAME: &'static str = stringify!($module);
            type Points = $module::Points;
            type Id = $module::PointId;
            fn from_corpus(corpus: &Corpus) -> (Self::Points, Vec<Option<Self::Id>>) {
                $module::from_corpus(corpus)
            }
            fn orient3d(points: &Self::Points, [a, b, c, d]: [Self::Id; 4]) -> i8 {
                points.orient3d(a, b, c, d)
            }
            fn insphere(points: &Self::Points, [a, b, c, d, e]: [Self::Id; 5]) -> i8 {
                points.insphere(a, b, c, d, e)
            }
            fn overhead(points: &Self::Points, [a, b, c, d]: [Self::Id; 4]) -> i8 {
                points.overhead(a, b, c, d)
            }
        }
    };
}

reference!(Legacy, legacy);
reference!(Current, current);

/// The points of a corpus built in one version, and the id of each corpus
/// point.
type Built<R> = (<R as Reference>::Points, Vec<Option<<R as Reference>::Id>>);

/// A call on built points: the points and the ids of the call's points.
type Case<'a, R, const N: usize> = (&'a <R as Reference>::Points, [<R as Reference>::Id; N]);

/// The corpus files of `IPREF_CORPUS_DIR`, or the versioned samples, in
/// file name order, with the calls these bindings cannot build (those on
/// LPI or TPI points) removed.
fn corpora() -> Vec<Corpus> {
    let dir = std::env::var_os(CORPUS_DIR).map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../corpus/samples"),
        PathBuf::from,
    );
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|x| x == "ipcorpus"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no corpus file in {}", dir.display());
    files
        .iter()
        .map(|path| {
            let file = std::fs::File::open(path).unwrap();
            let mut corpus = Corpus::read(std::io::BufReader::new(file))
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let before = corpus.calls.len();
            let points = &corpus.points;
            corpus.calls.retain(|call| {
                call.points()
                    .iter()
                    .all(|&i| supported(&points[i as usize]))
            });
            let skipped = before - corpus.calls.len();
            if skipped > 0 {
                eprintln!(
                    "{}: {skipped} calls on LPI or TPI points skipped",
                    corpus.model
                );
            }
            eprintln!("{}: {} calls", corpus.model, corpus.calls.len());
            corpus
        })
        .collect()
}

/// Whether these bindings can build a point: explicit and LNC points.
fn supported(point: &PointDef) -> bool {
    matches!(point, PointDef::Explicit(_) | PointDef::Lnc { .. })
}

/// The bucket of a call: its number of implicit points, capped at 3.
fn bucket(corpus: &Corpus, call: &Call) -> usize {
    let implicit = call
        .points()
        .iter()
        .filter(|&&i| !matches!(corpus.points[i as usize], PointDef::Explicit(_)))
        .count();
    implicit.min(BUCKETS.len() - 1)
}

/// The first `N` points of a call.
fn call_ids<Id: Copy, const N: usize>(ids: &[Option<Id>], call: &Call) -> [Id; N] {
    std::array::from_fn(|k| ids[call.points[k] as usize].expect("explicit or LNC point"))
}

/// Measures `f` over `cases`, one bench named `name`.
fn bench_cases<P, I: Copy>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    name: &str,
    cases: &[(&P, I)],
    f: fn(&P, I) -> i8,
) {
    group.throughput(Throughput::Elements(cases.len() as u64));
    group.bench_function(name, |b| {
        b.iter(|| {
            let mut sum = 0i64;
            for (points, ids) in cases {
                sum += i64::from(f(points, black_box(*ids)));
            }
            black_box(sum)
        })
    });
}

/// The calls of `predicate` with `N` points, by bucket, on built points.
fn bucketed<'a, R: Reference, const N: usize>(
    corpora: &[Corpus],
    built: &'a [Built<R>],
    predicate: Predicate,
) -> Vec<Vec<Case<'a, R, N>>> {
    let mut buckets: Vec<Vec<_>> = BUCKETS.iter().map(|_| Vec::new()).collect();
    for (corpus, (points, ids)) in corpora.iter().zip(built) {
        for call in corpus.calls.iter().filter(|c| c.predicate == predicate) {
            buckets[bucket(corpus, call)].push((points, call_ids(ids, call)));
        }
    }
    buckets
}

/// The `<version>/<predicate>` group.
fn by_bucket<P, I: Copy>(
    c: &mut Criterion,
    name: &str,
    buckets: &[Vec<(&P, I)>],
    f: fn(&P, I) -> i8,
) {
    let mut group = c.benchmark_group(name);
    for (label, cases) in BUCKETS.iter().zip(buckets) {
        if cases.is_empty() {
            eprintln!("{name}: no calls with {label} implicit points; skipped");
        } else {
            bench_cases(&mut group, label, cases, f);
        }
    }
    group.finish();
}

/// A group with one bench per corpus, each of which takes milliseconds.
fn per_corpus<'c>(c: &'c mut Criterion, name: &str) -> BenchmarkGroup<'c, WallTime> {
    let mut group = c.benchmark_group(name);
    group.sampling_mode(SamplingMode::Flat).sample_size(20);
    group
}

fn bench_version<R: Reference>(c: &mut Criterion, corpora: &[Corpus]) {
    let built: Vec<_> = corpora.iter().map(R::from_corpus).collect();
    let orient3d = bucketed::<R, 4>(corpora, &built, Predicate::Orient3d);
    let insphere = bucketed::<R, 5>(corpora, &built, Predicate::InSphere);
    by_bucket(c, &format!("{}/orient3d", R::NAME), &orient3d, R::orient3d);
    by_bucket(c, &format!("{}/insphere", R::NAME), &insphere, R::insphere);

    let all_orient3d: Vec<_> = orient3d.concat();
    let mut group = c.benchmark_group(format!("{}/overhead", R::NAME));
    bench_cases(&mut group, "no-op", &all_orient3d, R::overhead);
    group.finish();
    drop(built);

    let mut group = per_corpus(c, &format!("{}/construct", R::NAME));
    for corpus in corpora {
        let built = corpus.points.iter().filter(|p| supported(p)).count();
        group.throughput(Throughput::Elements(built as u64));
        group.bench_function(&corpus.model, |b| {
            b.iter_with_large_drop(|| R::from_corpus(black_box(corpus)))
        });
    }
    group.finish();

    let mut group = per_corpus(c, &format!("{}/replay", R::NAME));
    for corpus in corpora {
        group.throughput(Throughput::Elements(corpus.calls.len() as u64));
        group.bench_function(&corpus.model, |b| {
            b.iter(|| {
                let (points, ids) = R::from_corpus(black_box(corpus));
                let mut sum = 0i64;
                for call in &corpus.calls {
                    sum += i64::from(match call.predicate {
                        Predicate::Orient3d => R::orient3d(&points, call_ids(&ids, call)),
                        Predicate::InSphere => R::insphere(&points, call_ids(&ids, call)),
                    });
                }
                black_box(sum)
            })
        });
    }
    group.finish();
}

fn main() {
    let corpora = corpora();
    let mut c = Criterion::default().configure_from_args();
    bench_version::<Legacy>(&mut c, &corpora);
    bench_version::<Current>(&mut c, &corpora);
    c.final_summary();
}
