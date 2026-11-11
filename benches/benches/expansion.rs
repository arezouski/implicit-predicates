//! `sum`, `scale` and `product` on expansions of fixed length, against
//! `geometry-predicates` where it has an equivalent.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use geometry_predicates::predicates::{fast_expansion_sum_zeroelim, scale_expansion_zeroelim};
use implicit_predicates_benches::{
    FIRST_OPERAND_SEED, LENGTHS, PRODUCT_LENGTHS, SCALE_SEED, SECOND_OPERAND_SEED,
    product_capacity, product_operands, scale_factor, valid_expansion,
};
use implicit_predicates_core::{Scratch, expansion};

fn sum(c: &mut Criterion) {
    let mut group = c.benchmark_group("sum");
    for n in LENGTHS {
        let e = valid_expansion(FIRST_OPERAND_SEED, n);
        let f = valid_expansion(SECOND_OPERAND_SEED, n);
        let mut out = vec![0.0; 2 * n];
        group.bench_function(BenchmarkId::new("ours", n), |b| {
            b.iter(|| expansion::sum(black_box(&e), black_box(&f), black_box(&mut out)))
        });
        group.bench_function(BenchmarkId::new("geometry_predicates", n), |b| {
            b.iter(|| {
                fast_expansion_sum_zeroelim(black_box(&e), black_box(&f), black_box(&mut out))
            })
        });
    }
    group.finish();
}

fn scale(c: &mut Criterion) {
    let mut group = c.benchmark_group("scale");
    let factor = scale_factor(SCALE_SEED);
    for n in LENGTHS {
        let e = valid_expansion(FIRST_OPERAND_SEED, n);
        let mut out = vec![0.0; 2 * n];
        group.bench_function(BenchmarkId::new("ours", n), |b| {
            b.iter(|| expansion::scale(black_box(&e), black_box(factor), black_box(&mut out)))
        });
        group.bench_function(BenchmarkId::new("geometry_predicates", n), |b| {
            b.iter(|| {
                scale_expansion_zeroelim(black_box(&e), black_box(factor), black_box(&mut out))
            })
        });
    }
    group.finish();
}

fn product(c: &mut Criterion) {
    let mut group = c.benchmark_group("product");
    for n in PRODUCT_LENGTHS {
        let (e, f) = product_operands(n);
        let mut buf = vec![0.0; product_capacity(n)];
        let mut scratch = Scratch::borrowed(&mut buf);
        let a = scratch.push(&e).expect("capacity");
        let b = scratch.push(&f).expect("capacity");
        let mark = scratch.mark();
        group.bench_function(BenchmarkId::new("ours", n), |bencher| {
            bencher.iter(|| {
                let p = scratch
                    .product(black_box(a), black_box(b))
                    .expect("capacity");
                let len = p.len();
                scratch.reset(mark);
                len
            })
        });
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = sum, scale, product
}
criterion_main!(benches);
