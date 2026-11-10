//! Exact 3x3 determinant on the arena, over random, nearly singular and
//! exactly singular matrices.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use indirect_predicates_benches::{
    DET3_CAPACITY, MATRICES_SEED, det3_sign, near_singular_matrices, random_matrices,
    singular_matrices,
};
use indirect_predicates_core::{Scratch, Sign};

fn det3(c: &mut Criterion) {
    let mut buf = vec![0.0; DET3_CAPACITY];
    let mut scratch = Scratch::borrowed(&mut buf);
    let mut group = c.benchmark_group("det3");
    let sets = [
        ("random", random_matrices(MATRICES_SEED)),
        ("near_singular", near_singular_matrices(MATRICES_SEED)),
        ("singular", singular_matrices(MATRICES_SEED)),
    ];
    for (name, matrices) in &sets {
        group.bench_function(*name, |b| {
            b.iter(|| {
                let mut nonzero = 0usize;
                for m in matrices {
                    if det3_sign(&mut scratch, black_box(m)) != Sign::Zero {
                        nonzero += 1;
                    }
                }
                nonzero
            })
        });
    }
    group.finish();
}

criterion_group!(benches, det3);
criterion_main!(benches);
