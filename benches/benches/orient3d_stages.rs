//! Each stage of the generated `orient3d` alone: the filter on the `random`
//! set, which it decides, and each exact stage on the `near_degenerate` and
//! `degenerate` sets, which the filter cannot decide.
//!
//! As in the `predicates` benchmark, each measurement is one pass over a
//! whole set, every stage is called through a function pointer, and the
//! throughput in elements per second is the number of calls per second.
//! An undecided call counts as zero in the consumed sum. Every stage
//! decides every case of the sets it is measured on, except
//! `exact_differences` on `near_degenerate`: the moved coordinate makes most
//! differences inexact there, and the stage then returns after its check,
//! so that measurement is mostly the cost of declining. The tests of this
//! package check these facts.

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use implicit_predicates::Sign;
use implicit_predicates::stages::orient3d;
use implicit_predicates_benches::predicates::{Orient3dInput, orient3d_sets};

/// A stage of `orient3d`, as a function of the four points.
type Stage = fn([&[f64; 3]; 4]) -> Option<Sign>;

/// The measured stages, each with the sets it is measured on.
const STAGES: &[(&str, Stage, &[&str])] = &[
    ("filter", |p| orient3d::filter(p, 0.0), &["random"]),
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
    (
        "expansions_untranslated",
        orient3d::expansions_untranslated,
        &["near_degenerate", "degenerate"],
    ),
];

fn points(case: &Orient3dInput) -> [&[f64; 3]; 4] {
    [&case[0], &case[1], &case[2], &case[3]]
}

fn stages(c: &mut Criterion) {
    let sets = orient3d_sets();
    let mut group = c.benchmark_group("orient3d_stages");
    for (name, stage, set_names) in STAGES {
        for set in sets.iter().filter(|s| set_names.contains(&s.name)) {
            group.throughput(Throughput::Elements(set.cases.len() as u64));
            group.bench_function(format!("{name}/{}", set.name), |b| {
                b.iter(|| {
                    let mut sum = 0i64;
                    for case in &set.cases {
                        sum += stage(points(black_box(case))).map_or(0, |s| i64::from(i8::from(s)));
                    }
                    black_box(sum)
                })
            });
        }
    }
    group.finish();
}

criterion_group!(benches, stages);
criterion_main!(benches);
