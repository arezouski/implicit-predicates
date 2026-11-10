//! Exact 3x3 determinant on the arena, over random and nearly singular
//! matrices.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use indirect_predicates_benches::{Matrix3, near_singular_matrices, random_matrices};
use indirect_predicates_core::{Scratch, Sign, expansion};

const SEED: u64 = 0x6465_7433;

/// Pushes the exact value of `x*y - z*w` and returns its span.
fn minor(
    scratch: &mut Scratch<'_>,
    [x, y]: [f64; 2],
    [z, w]: [f64; 2],
) -> indirect_predicates_core::Span {
    let mut term = [0.0; 2];
    let len = expansion::from_two_product(x, y, &mut term);
    let first = scratch.push(&term[..len]).expect("capacity");
    let len = expansion::from_two_product(z, w, &mut term);
    let second = scratch.push(&term[..len]).expect("capacity");
    scratch.diff(first, second).expect("capacity")
}

/// Sign of the determinant of `m`, computed exactly.
fn det3_sign(scratch: &mut Scratch<'_>, m: &Matrix3) -> Sign {
    let mark = scratch.mark();
    let [a, b, c] = *m;
    let m0 = minor(scratch, [b[1], c[2]], [b[2], c[1]]);
    let m1 = minor(scratch, [b[0], c[2]], [b[2], c[0]]);
    let m2 = minor(scratch, [b[0], c[1]], [b[1], c[0]]);
    let t0 = scratch.scale(m0, a[0]).expect("capacity");
    let t1 = scratch.scale(m1, -a[1]).expect("capacity");
    let t2 = scratch.scale(m2, a[2]).expect("capacity");
    let partial = scratch.sum(t0, t1).expect("capacity");
    let det = scratch.sum(partial, t2).expect("capacity");
    let sign = scratch.sign(det);
    scratch.reset(mark);
    sign
}

fn det3(c: &mut Criterion) {
    let mut buf = vec![0.0; 1024];
    let mut scratch = Scratch::borrowed(&mut buf);
    let mut group = c.benchmark_group("det3");
    let sets = [
        ("random", random_matrices(SEED)),
        ("near_singular", near_singular_matrices(SEED)),
    ];
    for (name, matrices) in &sets {
        group.bench_function(*name, |b| {
            b.iter(|| {
                let mut positives = 0usize;
                for m in matrices {
                    if det3_sign(&mut scratch, black_box(m)) == Sign::Positive {
                        positives += 1;
                    }
                }
                positives
            })
        });
    }
    group.finish();
}

criterion_group!(benches, det3);
criterion_main!(benches);
