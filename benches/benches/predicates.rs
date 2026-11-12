//! `orient3d` and `insphere` on explicit points: `implicit-predicates`
//! (`orient3d` only, so far), the `robust` and `geometry-predicates` crates
//! and an unprotected `f64` determinant, over
//! random, nearly degenerate, exactly degenerate and recorded inputs. Then
//! `orient3d` of `implicit-predicates` on points of type `AnyPoint`: the
//! same explicit sets (group `orient3d_any_explicit`), which shows the cost
//! of a point type whose kind is known only at run time, and the recorded
//! calls with implicit points, as a mix and by number of implicit points
//! (group `orient3d_implicit`).
//!
//! Each measurement is one pass over a whole set; the throughput in elements
//! per second is the number of predicate calls per second, so the time per
//! call is its inverse. Every contender is called through a function
//! pointer, at the same cost for all; it is most of the time of the
//! unprotected determinant. The `corpus` sets are a weighted resample of the
//! stored calls (see `implicit_predicates_benches::predicates`). To add a
//! contender, add a row to [`ORIENT3D`] and [`INSPHERE`].

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use implicit_predicates_benches::predicates::{
    ImplicitInput, InSphereInput, InputSet, Orient3dInput, any_explicit_sets,
    geometry_insphere_sign, geometry_orient3d_sign, implicit_predicates_orient3d_any_sign,
    implicit_predicates_orient3d_sign, implicit_sets, insphere_sets, naive_insphere_sign,
    naive_orient3d_sign, orient3d_sets, robust_insphere_sign, robust_orient3d_sign,
};

/// A contender: a name and a function from the points of a call to the sign.
type Contender<T> = (&'static str, fn(&T) -> i8);

/// The `orient3d` contenders.
const ORIENT3D: &[Contender<Orient3dInput>] = &[
    ("implicit-predicates", implicit_predicates_orient3d_sign),
    ("robust", robust_orient3d_sign),
    ("geometry-predicates", geometry_orient3d_sign),
    ("naive", naive_orient3d_sign),
];

/// The `orient3d` contenders on points of any type.
const ORIENT3D_ANY: &[Contender<ImplicitInput>] =
    &[("implicit-predicates", implicit_predicates_orient3d_any_sign)];

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
    run(
        c,
        "orient3d_any_explicit",
        &any_explicit_sets(),
        ORIENT3D_ANY,
    );
    run(c, "orient3d_implicit", &implicit_sets(), ORIENT3D_ANY);
}

criterion_group!(benches, predicates);
criterion_main!(benches);
