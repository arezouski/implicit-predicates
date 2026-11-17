//! The exact stages of `dihedral_sign`, `dot_sign_3d` and `orient3d` on
//! implicit points (`Lnc` and `Bpt`), on the loads of
//! `implicit_predicates_benches::predicates::dihedral_sign_loads`,
//! `dot_sign_3d_loads` and `orient3d_exact_loads`: calls that the filter,
//! and its second test when the predicate has one, do not decide, nearly
//! degenerate and exactly degenerate, built on small meshes.
//!
//! For each load, five measurements:
//!
//! - `expansions_translated_in`, the stage on expansions over arena spans,
//!   on exact coordinates computed beforehand;
//! - `expansions_with_coordinates`, the same stage with the cost of
//!   `Point::exact` for every point, as the chain runs it;
//! - `big_integers_in`, the stage on big integers, on big-integer
//!   coordinates computed beforehand;
//! - `big_integers_with_coordinates`, the same stage with the cost of
//!   `Point::exact_big` for every point, as the chain runs it;
//! - `exact_stages_in`, the whole exact chain as emitted: the second test
//!   of the filter when the predicate has one, then the stage on
//!   expansions, then, when it declines, the stage on big integers.
//!
//! As in the `orient3d_stages` benchmark, each measurement is one pass over
//! a whole load, every stage is called through a function pointer, and the
//! throughput in elements per second is the number of calls per second. An
//! undecided call counts as zero in the consumed sum. The tests of this
//! package check that the filters decide no call of the loads, that every
//! stage measured here decides every call, and every sign; the peak arena
//! use of each stage on the loads is printed by an ignored test of
//! `tests/exact_loads.rs`.

use std::hint::black_box;

use criterion::measurement::WallTime;
use criterion::{BenchmarkGroup, Criterion, Throughput, criterion_group, criterion_main};
use implicit_predicates::{BigCoords, ExactCoords, Scratch, Sign};
use implicit_predicates_benches::predicates::{
    DIHEDRAL_SIGN, DOT_SIGN_3D, ExactLoad, ExactStages, ORIENT3D, dihedral_sign_loads,
    dot_sign_3d_loads, orient3d_exact_loads,
};

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

/// The five measurements of every load of `loads`.
fn measure<const N: usize>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    stages: &ExactStages<N>,
    loads: &[ExactLoad<N>],
) {
    let predicate = stages.name;
    for load in loads {
        let name = |stage: &str| format!("{predicate}/{stage}/{}", load.name);
        let cases = &load.cases;

        let mut scratch = Scratch::growable(1 << 16);
        let coords: Vec<[ExactCoords; N]> = cases
            .iter()
            .map(|c| {
                stages
                    .exact_coords(&mut scratch, c)
                    .expect("exact coordinates")
            })
            .collect();
        bench(group, name("expansions_translated_in"), &coords, |q| {
            stages.expansions_on(&mut scratch, q)
        });

        let mut scratch = Scratch::growable(1 << 16);
        bench(group, name("expansions_with_coordinates"), cases, |c| {
            stages.expansions(&mut scratch, c)
        });

        let mut scratch = Scratch::growable(1 << 16);
        let big: Vec<[BigCoords; N]> = cases
            .iter()
            .map(|c| stages.big_coords(&mut scratch, c))
            .collect();
        bench(group, name("big_integers_in"), &big, |q| {
            Some(stages.big_integers_on(&mut scratch, q))
        });

        let mut scratch = Scratch::growable(1 << 16);
        bench(group, name("big_integers_with_coordinates"), cases, |c| {
            Some(stages.big_integers(&mut scratch, c))
        });

        let mut scratch = Scratch::growable(1 << 16);
        bench(group, name("exact_stages_in"), cases, |c| {
            Some(stages.chain(&mut scratch, c))
        });
    }
}

fn stages(c: &mut Criterion) {
    let mut group = c.benchmark_group("angle_stages");
    measure(&mut group, &DIHEDRAL_SIGN, &dihedral_sign_loads());
    measure(&mut group, &DOT_SIGN_3D, &dot_sign_3d_loads());
    measure(&mut group, &ORIENT3D, &orient3d_exact_loads());
    group.finish();
}

criterion_group!(benches, stages);
criterion_main!(benches);
