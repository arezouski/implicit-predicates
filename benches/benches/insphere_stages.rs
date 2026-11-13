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
//! The exact stages, on the recorded calls that the filter does not decide,
//! by number of implicit points, zero to five, in two groups (see
//! `InSphereGroup`): the calls that a stage over spans decides, and those
//! that go to the stage on big integers because a component of an exact
//! coordinate is out of range. `insphere` has no stage on the stack, so the
//! explicit calls go through the stages over spans too.
//! `expansions_translated_in` and `expansions_untranslated_in` are measured
//! on the first group only, on exact coordinates computed beforehand.
//! `exact_stages_in` computes the exact coordinates, then runs the stages
//! over spans and, when they decline, the stage on big integers: the cost of
//! the chain after the filter, on both groups. `big_integers_in` is measured
//! with the cost of `exact_big` for the five points, which writes their
//! coordinates to the arena, on both groups: on the first one, it is what
//! the stage on big integers would cost instead of the stages over spans.
//!
//! The recorded calls come from the versioned samples, or from the corpus
//! files of the directory named by `IMPLICIT_PREDICATES_CORPUS_DIR`. A group
//! with no call for some number of implicit points is not measured: the
//! samples have no call of the second group.
//!
//! As in the `orient3d_stages` benchmark, each measurement is one pass over
//! a whole set, every stage is called through a function pointer, and the
//! throughput in elements per second is the number of calls per second. An
//! undecided call counts as zero in the consumed sum. The tests of this
//! package check which stage decides each set and every sign.

use std::hint::black_box;

use criterion::measurement::WallTime;
use criterion::{BenchmarkGroup, Criterion, Throughput, criterion_group, criterion_main};
use implicit_predicates::stages::insphere;
use implicit_predicates::{ExactCoords, Scratch, ScratchExhausted, Sign};
use implicit_predicates_benches::predicates::{
    InSphereGroup, InSphereImplicitInput, InSphereInput, InputSet, insphere_any_explicit_sets,
    insphere_big_integers_stage, insphere_exact_sets, insphere_implicit_sets, insphere_sets,
};

/// A stage over arena spans, as a function of the exact coordinates.
type SpansStage = fn(&mut Scratch<'_>, [&ExactCoords; 5]) -> Result<Option<Sign>, ScratchExhausted>;

/// The measured stages over arena spans.
const SPANS_STAGES: &[(&str, SpansStage)] = &[
    (
        "expansions_translated_in",
        insphere::expansions_translated_in,
    ),
    (
        "expansions_untranslated_in",
        insphere::expansions_untranslated_in,
    ),
];

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

fn exact_stages(group: &mut BenchmarkGroup<'_, WallTime>) {
    for mut set in insphere_exact_sets() {
        let name = set.name();
        if set.group == InSphereGroup::Spans {
            for (stage_name, stage) in SPANS_STAGES {
                let scratch = &mut set.scratch;
                bench(group, format!("{stage_name}/{name}"), &set.coords, |q| {
                    stage(scratch, q.each_ref()).expect("a growable arena")
                });
            }
        }
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
