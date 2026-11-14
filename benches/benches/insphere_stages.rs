//! Each stage of the generated `insphere` alone.
//!
//! The filter: `filter` on explicit points (`[f64; 3]`) and `filter_points`,
//! the filter with the uncertainty of the errors of the points, on the same
//! points as explicit `AnyPoint`, both on the `random` set, which the filter
//! decides, and on the `near_degenerate` set, which it declines, so that
//! measurement is the cost of declining; then `filter_points` on the
//! recorded calls with implicit points, as a mix and by number of implicit
//! points.
//!
//! The exact stage, on the recorded calls that the filter does not decide,
//! by number of implicit points, zero to five, in one group, `undecided`.
//! `insphere` is of degree five, so the stage on big integers is its only
//! exact stage, for explicit points too. `exact_stages_in` writes the
//! coordinates of the points as big integers to the arena and runs the
//! stage on big integers: the cost of the chain after the filter.
//! `big_integers_in` is measured with the cost of `exact_big` for the five
//! points, as the chain runs it.
//!
//! The recorded calls come from the versioned samples, or from the corpus
//! files of the directory named by `IMPLICIT_PREDICATES_CORPUS_DIR`. A
//! number of implicit points with no such call is not measured.
//!
//! As in the `orient3d_stages` benchmark, each measurement is one pass over
//! a whole set, every stage is called through a function pointer, and the
//! throughput in elements per second is the number of calls per second. An
//! undecided call counts as zero in the consumed sum. The tests of this
//! package check which stage decides each set and every sign.

use std::hint::black_box;

use criterion::measurement::WallTime;
use criterion::{BenchmarkGroup, Criterion, Throughput, criterion_group, criterion_main};
use implicit_predicates::Sign;
use implicit_predicates::stages::insphere;
use implicit_predicates_benches::predicates::{
    InSphereExactSet, InSphereImplicitInput, InSphereInput, InputSet, insphere_any_explicit_sets,
    insphere_big_integers_stage, insphere_exact_sets, insphere_implicit_sets, insphere_sets,
};

/// The sets of explicit points the filter is measured on.
const FILTER_SETS: [&str; 2] = ["random", "near_degenerate"];

/// The consumed value of a result.
fn value(sign: Option<Sign>) -> i64 {
    sign.map_or(0, |s| i64::from(i8::from(s)))
}

/// One measurement of `f` over every case of `cases`.
fn bench<T>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    name: String,
    cases: &[T],
    mut f: impl FnMut(&T) -> Option<Sign>,
) {
    group.throughput(Throughput::Elements(cases.len() as u64));
    group.bench_function(name, |b| {
        b.iter(|| {
            let mut sum = 0i64;
            for case in cases {
                sum += value(f(black_box(case)));
            }
            black_box(sum)
        })
    });
}

fn filter(group: &mut BenchmarkGroup<'_, WallTime>) {
    let sets = insphere_sets();
    // Through a function pointer, as every other stage.
    let stage: fn(&InSphereInput) -> Option<Sign> =
        |[a, b, c, d, e]| insphere::filter([a, b, c, d, e], 0.0);
    for set in sets.iter().filter(|s| FILTER_SETS.contains(&s.name)) {
        bench(group, format!("filter/{}", set.name), &set.cases, stage);
    }
}

fn filter_points(group: &mut BenchmarkGroup<'_, WallTime>) {
    let explicit = insphere_any_explicit_sets();
    let explicit = explicit.iter().filter(|s| FILTER_SETS.contains(&s.name));
    let implicit: Vec<InputSet<InSphereImplicitInput>> = insphere_implicit_sets()
        .into_iter()
        .map(|(set, _)| set)
        .collect();
    let stage: fn(&InSphereImplicitInput) -> Option<Sign> =
        |[a, b, c, d, e]| insphere::filter_points(a, b, c, d, e);
    for set in explicit.chain(&implicit) {
        let name = format!("filter_points/{}", set.name);
        bench(group, name, &set.cases, stage);
    }
}

/// Fewer distinct recorded calls than this make a set unrepresentative.
const FEW_DISTINCT_CALLS: usize = 16;

/// Tells on standard error which numbers of implicit points are not
/// measured and which sets were drawn from very few distinct calls.
fn report_exact_sets(sets: &[InSphereExactSet]) {
    let missing: Vec<String> = (0..=5)
        .filter(|k| !sets.iter().any(|s| s.implicit == *k))
        .map(|k| k.to_string())
        .collect();
    if !missing.is_empty() {
        eprintln!(
            "insphere_stages: no recorded calls that the filter does not decide with {} \
             implicit points: skipped; set IMPLICIT_PREDICATES_CORPUS_DIR to a corpus that \
             has them",
            missing.join(", ")
        );
    }
    for set in sets {
        let distinct: std::collections::HashSet<String> =
            set.points.iter().map(|p| format!("{p:?}")).collect();
        if distinct.len() < FEW_DISTINCT_CALLS {
            eprintln!(
                "insphere_stages: set `{}` is drawn from only {} distinct recorded calls: \
                 its timing is not representative",
                set.name(),
                distinct.len()
            );
        }
    }
}

fn exact_stages(group: &mut BenchmarkGroup<'_, WallTime>) {
    let sets = insphere_exact_sets();
    report_exact_sets(&sets);
    for mut set in sets {
        let name = set.name();
        let scratch = &mut set.scratch;
        bench(
            group,
            format!("exact_stages_in/{name}"),
            &set.points,
            |[a, b, c, d, e]| {
                Some(insphere::exact_stages_in(scratch, a, b, c, d, e).expect("a growable arena"))
            },
        );
        let scratch = &mut set.scratch;
        bench(group, format!("big_integers_in/{name}"), &set.points, |p| {
            Some(insphere_big_integers_stage(scratch, p))
        });
    }
}

fn stages(c: &mut Criterion) {
    let mut group = c.benchmark_group("insphere_stages");
    filter(&mut group);
    filter_points(&mut group);
    exact_stages(&mut group);
    group.finish();
}

criterion_group!(benches, stages);
criterion_main!(benches);
