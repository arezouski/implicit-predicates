//! `two_product`: Dekker's algorithm against the fused multiply-add.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use indirect_predicates_benches::{PAIRS_SEED, product_pairs};
use indirect_predicates_core::eft;

/// Applies `product` to every pair and stores each result in `out`.
///
/// The results are independent of one another (there is no accumulator
/// carrying a dependency between iterations), so the measurement is the
/// throughput of the product, not the latency of an addition chain.
fn sweep(pairs: &[(f64, f64)], out: &mut [(f64, f64)], product: impl Fn(f64, f64) -> (f64, f64)) {
    for (slot, &(a, b)) in out.iter_mut().zip(pairs) {
        *slot = product(a, b);
    }
}

fn two_product(c: &mut Criterion) {
    let pairs = product_pairs(PAIRS_SEED);
    let mut out = vec![(0.0, 0.0); pairs.len()];
    let mut group = c.benchmark_group("two_product");
    group.bench_function("dekker", |b| {
        b.iter(|| {
            sweep(black_box(&pairs), &mut out, eft::two_product_dekker);
            black_box(&mut out);
        })
    });
    #[cfg(any(
        all(target_arch = "x86_64", target_feature = "fma"),
        all(target_arch = "aarch64", target_feature = "neon")
    ))]
    group.bench_function("fma", |b| {
        b.iter(|| {
            sweep(black_box(&pairs), &mut out, eft::two_product_fma);
            black_box(&mut out);
        })
    });
    group.finish();
}

criterion_group!(benches, two_product);
criterion_main!(benches);
