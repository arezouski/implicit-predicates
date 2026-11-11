//! `orient3d` and `insphere` on explicit points: `indirect-predicates`
//! (`orient3d` only, so far), the `robust` and `geometry-predicates` crates
//! and an unprotected `f64` determinant, over
//! random, nearly degenerate, exactly degenerate and recorded inputs.
//!
//! Each measurement is one pass over a whole set; the throughput in elements
//! per second is the number of predicate calls per second, so the time per
//! call is its inverse. Every contender is called through a function
//! pointer, at the same cost for all; it is most of the time of the
//! unprotected determinant. The `corpus` sets are a weighted resample of the
//! stored calls (see `indirect_predicates_benches::predicates`). To add a
//! contender, add a row to [`ORIENT3D`] and [`INSPHERE`].

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use indirect_predicates_benches::predicates::{
    InSphereInput, InputSet, Orient3dInput, geometry_insphere_sign, geometry_orient3d_sign,
    indirect_predicates_orient3d_sign, insphere_sets, naive_insphere_sign, naive_orient3d_sign,
    orient3d_sets, robust_insphere_sign, robust_orient3d_sign,
};

/// A contender: a name and a function from the points of a call to the sign.
type Contender<T> = (&'static str, fn(&T) -> i8);

/// The `orient3d` contenders.
const ORIENT3D: &[Contender<Orient3dInput>] = &[
    ("indirect-predicates", indirect_predicates_orient3d_sign),
    ("robust", robust_orient3d_sign),
    ("geometry-predicates", geometry_orient3d_sign),
    ("naive", naive_orient3d_sign),
];

/// The `insphere` contenders.
const INSPHERE: &[Contender<InSphereInput>] = &[
    ("robust", robust_insphere_sign),
    ("geometry-predicates", geometry_insphere_sign),
    ("naive", naive_insphere_sign),
];

fn run<T>(c: &mut Criterion, group_name: &str, sets: &[InputSet<T>], contenders: &[Contender<T>]) {
    let mut group = c.benchmark_group(group_name);
    for set in sets {
        group.throughput(Throughput::Elements(set.cases.len() as u64));
        for (name, predicate) in contenders {
            group.bench_function(format!("{name}/{}", set.name), |b| {
                b.iter(|| {
                    let mut sum = 0i64;
                    for case in &set.cases {
                        sum += i64::from(predicate(black_box(case)));
                    }
                    black_box(sum)
                })
            });
        }
    }
    group.finish();
}

fn predicates(c: &mut Criterion) {
    run(c, "orient3d", &orient3d_sets(), ORIENT3D);
    run(c, "insphere", &insphere_sets(), INSPHERE);
}

criterion_group!(benches, predicates);
criterion_main!(benches);
