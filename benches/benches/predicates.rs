//! `orient3d` and `insphere` on explicit points: `implicit-predicates`
//! (`orient3d` only), the `robust` and `geometry-predicates` crates
//! and an unprotected `f64` determinant, over
//! random, nearly degenerate, exactly degenerate and recorded inputs. Then
//! `orient3d` of `implicit-predicates` on points of type `AnyPoint`: the
//! same explicit sets (group `orient3d_any_explicit`), which shows the cost
//! of a point type whose kind is known only at run time, and the recorded
//! calls with implicit points, as a mix and by number of implicit points
//! (group `orient3d_implicit`). Last, the whole chain of `insphere` of
//! `implicit-predicates` on the recorded calls with implicit points, by the
//! same mix and number of implicit points (group `insphere_implicit`), with
//! two contenders: the chain with an arena kept by the benchmark and the
//! public `insphere`, which keeps its own arena for each thread.
//!
//! The public `dot_sign_3d` and `dihedral_sign`, the whole chain, filter
//! included, on loads of explicit points (groups `dot_sign_3d` and
//! `dihedral_sign`, with an unprotected `f64` expression as the lower
//! bound) and of `Lnc`, `Bpt` and mixed points (groups `dot_sign_3d_implicit`
//! and `dihedral_sign_implicit`), random, nearly right and exactly right;
//! then `orient3d` and `insphere` on `Bpt` (groups `orient3d_bpt` and
//! `insphere_bpt`). The implicit loads come with the short parameters of a
//! mesh (`mesh`) and with random 53-bit parameters (`params53`), the adverse
//! case. No other library has these two angle predicates, so there is no
//! external contender for them.
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
use implicit_predicates::AnyPoint;
use implicit_predicates_benches::predicates::{
    ImplicitInput, InSphereImplicitInput, InSphereInput, InputSet, Orient3dInput, PublicLoad,
    any_explicit_sets, dihedral_sign_public_loads, dot_sign_3d_public_loads,
    geometry_insphere_sign, geometry_orient3d_sign, implicit_predicates_dihedral_sign_sign,
    implicit_predicates_dot_sign_3d_sign, implicit_predicates_insphere_any_sign,
    implicit_predicates_insphere_public_sign, implicit_predicates_orient3d_any_sign,
    implicit_predicates_orient3d_sign, implicit_sets, insphere_bpt_loads, insphere_implicit_sets,
    insphere_sets, naive_dihedral_sign_sign, naive_dot_sign_3d_sign, naive_insphere_sign,
    naive_orient3d_sign, orient3d_bpt_loads, orient3d_sets, robust_insphere_sign,
    robust_orient3d_sign,
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

/// The `insphere` contenders on points of any type.
const INSPHERE_ANY: &[Contender<InSphereImplicitInput>] = &[
    ("implicit-predicates", implicit_predicates_insphere_any_sign),
    (
        "implicit-predicates-public",
        implicit_predicates_insphere_public_sign,
    ),
];

/// The `dot_sign_3d` contenders on loads of explicit points.
const DOT_SIGN_3D_EXPLICIT: &[Contender<[AnyPoint; 3]>] = &[
    ("implicit-predicates", implicit_predicates_dot_sign_3d_sign),
    ("naive", naive_dot_sign_3d_sign),
];

/// The `dot_sign_3d` contenders on loads of implicit points.
const DOT_SIGN_3D_IMPLICIT: &[Contender<[AnyPoint; 3]>] =
    &[("implicit-predicates", implicit_predicates_dot_sign_3d_sign)];

/// The `dihedral_sign` contenders on loads of explicit points.
const DIHEDRAL_SIGN_EXPLICIT: &[Contender<[AnyPoint; 4]>] = &[
    (
        "implicit-predicates",
        implicit_predicates_dihedral_sign_sign,
    ),
    ("naive", naive_dihedral_sign_sign),
];

/// The `dihedral_sign` contenders on loads of implicit points.
const DIHEDRAL_SIGN_IMPLICIT: &[Contender<[AnyPoint; 4]>] = &[(
    "implicit-predicates",
    implicit_predicates_dihedral_sign_sign,
)];

/// The `insphere` contenders.
const INSPHERE: &[Contender<InSphereInput>] = &[
    ("robust", robust_insphere_sign),
    ("geometry-predicates", geometry_insphere_sign),
    ("naive", naive_insphere_sign),
];

/// The sets as the name and the cases of each.
fn named<T>(sets: &[InputSet<T>]) -> Vec<(&str, &[T])> {
    sets.iter().map(|s| (s.name, s.cases.as_slice())).collect()
}

/// The loads that have `explicit` points only, or those that have not.
fn loads_of<const N: usize>(
    loads: &[PublicLoad<N>],
    explicit: bool,
) -> Vec<(&str, &[[AnyPoint; N]])> {
    loads
        .iter()
        .filter(|l| l.name.starts_with("explicit/") == explicit)
        .map(|l| (l.name.as_str(), l.cases.as_slice()))
        .collect()
}

fn run<T>(c: &mut Criterion, group_name: &str, sets: &[(&str, &[T])], contenders: &[Contender<T>]) {
    let mut group = c.benchmark_group(group_name);
    for &(set_name, cases) in sets {
        group.throughput(Throughput::Elements(cases.len() as u64));
        for (name, predicate) in contenders {
            group.bench_function(format!("{name}/{set_name}"), |b| {
                b.iter(|| {
                    let mut sum = 0i64;
                    for case in cases {
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
    run(c, "orient3d", &named(&orient3d_sets()), ORIENT3D);
    run(c, "insphere", &named(&insphere_sets()), INSPHERE);
    run(
        c,
        "orient3d_any_explicit",
        &named(&any_explicit_sets()),
        ORIENT3D_ANY,
    );
    run(
        c,
        "orient3d_implicit",
        &named(&implicit_sets()),
        ORIENT3D_ANY,
    );
    let insphere_implicit: Vec<_> = insphere_implicit_sets()
        .into_iter()
        .map(|(set, _)| set)
        .collect();
    run(
        c,
        "insphere_implicit",
        &named(&insphere_implicit),
        INSPHERE_ANY,
    );

    let loads = dot_sign_3d_public_loads();
    run(
        c,
        "dot_sign_3d",
        &loads_of(&loads, true),
        DOT_SIGN_3D_EXPLICIT,
    );
    run(
        c,
        "dot_sign_3d_implicit",
        &loads_of(&loads, false),
        DOT_SIGN_3D_IMPLICIT,
    );
    let loads = dihedral_sign_public_loads();
    run(
        c,
        "dihedral_sign",
        &loads_of(&loads, true),
        DIHEDRAL_SIGN_EXPLICIT,
    );
    run(
        c,
        "dihedral_sign_implicit",
        &loads_of(&loads, false),
        DIHEDRAL_SIGN_IMPLICIT,
    );
    let loads = orient3d_bpt_loads();
    run(c, "orient3d_bpt", &loads_of(&loads, false), ORIENT3D_ANY);
    let loads = insphere_bpt_loads();
    run(c, "insphere_bpt", &loads_of(&loads, false), INSPHERE_ANY);
}

criterion_group!(benches, predicates);
criterion_main!(benches);
