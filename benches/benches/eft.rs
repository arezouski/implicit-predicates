//! `two_product`: Dekker's algorithm against the fused multiply-add.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use indirect_predicates_benches::product_pairs;
use indirect_predicates_core::eft;

const SEED: u64 = 0x7770_7072_6f64;

/// Measures `product` over every pair, accumulating `x + y` so the work
/// cannot be optimised away.
fn sweep(pairs: &[(f64, f64)], product: impl Fn(f64, f64) -> (f64, f64)) -> f64 {
    let mut total = 0.0;
    for &(a, b) in pairs {
        let (x, y) = product(black_box(a), black_box(b));
        total += x + y;
    }
    total
}

fn two_product(c: &mut Criterion) {
    let pairs = product_pairs(SEED);
    let mut group = c.benchmark_group("two_product");
    group.bench_function("dekker", |b| {
        b.iter(|| black_box(sweep(&pairs, eft::two_product_dekker)))
    });
    #[cfg(any(
        all(target_arch = "x86_64", target_feature = "fma"),
        all(target_arch = "aarch64", target_feature = "neon")
    ))]
    group.bench_function("fma", |b| {
        b.iter(|| black_box(sweep(&pairs, eft::two_product_fma)))
    });
    group.finish();
}

criterion_group!(benches, two_product);
criterion_main!(benches);
