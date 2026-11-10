//! Weighted resampling of stored calls, so that a set of calls drawn from
//! corpora reflects the mix of calls of the recording runs.
//!
//! A corpus stores a sample of each stratum (predicate, implicit count, zero
//! or non-zero sign): `kept` of the `total` calls of the stratum. A stored
//! call therefore stands for `total / kept` calls of its run, and that is
//! its weight here.

use crate::{Call, Corpus};

/// The weight of every stored call of `corpus`, in order: `total / kept` of
/// its stratum.
///
/// # Panics
///
/// Panics if a stored call has no sampling entry, which a valid corpus never
/// has (see [`Corpus::validate`]).
pub fn weights(corpus: &Corpus) -> Vec<f64> {
    corpus
        .calls
        .iter()
        .map(|call| {
            let s = corpus.sampling[&corpus.stratum_of(call)];
            s.total as f64 / s.kept as f64
        })
        .collect()
}

/// The SplitMix64 generator: small, fast and fully determined by its seed.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A uniform number in `[0, 1)`.
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
}

/// Draws `count` stored calls, with replacement, among the calls of
/// `corpora` for which `eligible` holds, each with probability proportional
/// to its weight (see [`weights`]). Returns `(corpus index, call index)`
/// pairs in the order drawn.
///
/// The result depends only on the corpora, `count`, `seed` and `eligible`.
/// It is empty when no call is eligible.
pub fn weighted_sample(
    corpora: &[Corpus],
    count: usize,
    seed: u64,
    eligible: impl Fn(&Corpus, &Call) -> bool,
) -> Vec<(usize, usize)> {
    let mut calls = Vec::new();
    let mut cumulative = Vec::new();
    let mut sum = 0.0;
    for (c, corpus) in corpora.iter().enumerate() {
        for (i, (call, w)) in corpus.calls.iter().zip(weights(corpus)).enumerate() {
            if eligible(corpus, call) {
                sum += w;
                calls.push((c, i));
                cumulative.push(sum);
            }
        }
    }
    if calls.is_empty() {
        return Vec::new();
    }
    let mut rng = SplitMix64(seed);
    (0..count)
        .map(|_| {
            let target = rng.unit() * sum;
            let k = cumulative.partition_point(|&s| s <= target);
            calls[k.min(calls.len() - 1)]
        })
        .collect()
}
