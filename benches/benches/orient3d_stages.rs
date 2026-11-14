//! Each stage of the generated `orient3d` alone.
//!
//! On explicit points (`[f64; 3]`): the filter on the `random` set, which it
//! decides, and each exact stage on the `near_degenerate` and `degenerate`
//! sets, which the filter cannot decide.
//!
//! On points of any type (`AnyPoint`): `filter_points`, the filter with the
//! uncertainty of the errors of the points, on the `random` set as explicit
//! `AnyPoint` and on the recorded calls with implicit points, as a mix and
//! by number of implicit points; and the exact stage over arena spans on
//! the recorded calls that the filter does not decide, by number of
//! implicit points. `expansions_translated_in` is measured on exact
//! coordinates computed beforehand; `exact_stages_in` computes them, then
//! runs the stages in the order of the chain. `big_integers_in` is measured
//! with the cost of `exact_big` for the four points, which writes their
//! coordinates to the arena: on the explicit `near_degenerate` and
//! `degenerate` sets and on the same recorded calls as the stage over spans.
//!
//! As in the `predicates` benchmark, each measurement is one pass over a
//! whole set, every stage is called through a function pointer, and the
//! throughput in elements per second is the number of calls per second.
//! An undecided call counts as zero in the consumed sum. Every stage
//! decides every case of the sets it is measured on, except the filter on
//! the recorded calls and `exact_differences` on `near_degenerate`: the
//! moved coordinate makes most differences inexact there, and the stage then
//! returns after its check, so that measurement is mostly the cost of
//! declining. The tests of this package check these facts.

use std::hint::black_box;

use criterion::measurement::WallTime;
use criterion::{BenchmarkGroup, Criterion, Throughput, criterion_group, criterion_main};
use implicit_predicates::stages::orient3d;
use implicit_predicates::{ExactCoords, Scratch, ScratchExhausted, Sign};
use implicit_predicates_benches::predicates::{
    ImplicitInput, InputSet, Orient3dInput, any_explicit_sets, big_integers_stage, exact_sets,
    implicit_sets, orient3d_sets, out_of_range_sets,
};

/// A stage of `orient3d`, as a function of the four points.
type Stage = fn([&[f64; 3]; 4]) -> Option<Sign>;

/// A stage over arena spans, as a function of the exact coordinates.
type SpansStage = fn(&mut Scratch<'_>, [&ExactCoords; 4]) -> Result<Option<Sign>, ScratchExhausted>;

/// The measured stages, each with the sets it is measured on.
const STAGES: &[(&str, Stage, &[&str])] = &[
    ("filter", |p| orient3d::filter(p, [0.0; 3]), &["random"]),
    (
        "exact_differences",
        orient3d::exact_differences,
        &["near_degenerate", "degenerate"],
    ),
    (
        "expansions_translated",
        orient3d::expansions_translated,
        &["near_degenerate", "degenerate"],
    ),
];

/// The measured stages over arena spans.
const SPANS_STAGES: &[(&str, SpansStage)] = &[(
    "expansions_translated_in",
    orient3d::expansions_translated_in,
)];

fn points(case: &Orient3dInput) -> [&[f64; 3]; 4] {
    [&case[0], &case[1], &case[2], &case[3]]
}

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

/// One measurement of a stage that always decides, over every case of
/// `cases`.
fn bench_sign<T>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    name: String,
    cases: &[T],
    mut f: impl FnMut(&T) -> Sign,
) {
    bench(group, name, cases, |c| Some(f(c)));
}

fn explicit_stages(group: &mut BenchmarkGroup<'_, WallTime>) {
    let sets = orient3d_sets();
    for (name, stage, set_names) in STAGES {
        for set in sets.iter().filter(|s| set_names.contains(&s.name)) {
            bench(group, format!("{name}/{}", set.name), &set.cases, |c| {
                stage(points(c))
            });
        }
    }
}

fn filter_points(group: &mut BenchmarkGroup<'_, WallTime>) {
    let explicit = any_explicit_sets();
    let random = explicit.iter().filter(|s| s.name == "random");
    let sets: Vec<&InputSet<ImplicitInput>> = random.collect();
    let implicit = implicit_sets();
    // Through a function pointer, as every other stage.
    let stage: fn(&ImplicitInput) -> Option<Sign> =
        |[a, b, c, d]| orient3d::filter_points(a, b, c, d);
    for set in sets.into_iter().chain(&implicit) {
        let name = format!("filter_points/{}", set.name);
        bench(group, name, &set.cases, stage);
    }
}

fn spans_stages(group: &mut BenchmarkGroup<'_, WallTime>) {
    for mut set in exact_sets() {
        for (name, stage) in SPANS_STAGES {
            let scratch = &mut set.scratch;
            bench(group, format!("{name}/{}", set.name), &set.coords, |q| {
                stage(scratch, q.each_ref()).expect("a growable arena")
            });
        }
        let scratch = &mut set.scratch;
        let name = format!("exact_stages_in/{}", set.name);
        bench(group, name, &set.points, |[a, b, c, d]| {
            Some(orient3d::exact_stages_in(scratch, a, b, c, d).expect("a growable arena"))
        });
    }
}

fn big_integers(group: &mut BenchmarkGroup<'_, WallTime>) {
    let mut scratch = Scratch::growable(1 << 16);
    for set in any_explicit_sets()
        .iter()
        .filter(|s| matches!(s.name, "near_degenerate" | "degenerate"))
    {
        let name = format!("big_integers_in/{}", set.name);
        bench_sign(group, name, &set.cases, |p| {
            big_integers_stage(&mut scratch, p)
        });
    }
    for mut set in exact_sets() {
        let name = format!("big_integers_in/{}", set.name);
        bench_sign(group, name, &set.points, |p| {
            big_integers_stage(&mut set.scratch, p)
        });
    }
}

/// The whole predicate on explicit points that no stage on the stack decides.
fn out_of_range(group: &mut BenchmarkGroup<'_, WallTime>) {
    for set in out_of_range_sets() {
        let name = format!("orient3d_out_of_range/{}", set.name);
        bench_sign(group, name, &set.cases, |c| {
            implicit_predicates::orient3d(&c[0], &c[1], &c[2], &c[3])
        });
    }
}

fn stages(c: &mut Criterion) {
    let mut group = c.benchmark_group("orient3d_stages");
    explicit_stages(&mut group);
    filter_points(&mut group);
    spans_stages(&mut group);
    big_integers(&mut group);
    out_of_range(&mut group);
    group.finish();
}

criterion_group!(benches, stages);
criterion_main!(benches);
