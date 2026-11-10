//! Statistics of a corpus, computed from its counters.
//!
//! The counters cover every call of the recording run, so every figure here
//! describes the whole run and never only the stored calls.

use crate::{Corpus, Predicate, Stage};
use std::collections::BTreeMap;
use std::fmt::Write;

const PREDICATES: [Predicate; 2] = [Predicate::Orient3d, Predicate::InSphere];
const STAGES: [Stage; 5] = [
    Stage::Unknown,
    Stage::Filtered,
    Stage::Interval,
    Stage::Bigfloat,
    Stage::Exact,
];

/// Call counts by predicate, implicit-point count and deciding stage.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stats {
    /// Calls by (predicate, implicit count).
    total: BTreeMap<(Predicate, u8), u64>,
    /// Calls with a zero sign by (predicate, implicit count).
    zeros: BTreeMap<(Predicate, u8), u64>,
    /// Calls by (predicate, implicit count, stage).
    stages: BTreeMap<(Predicate, u8, Stage), u64>,
}

fn name(p: Predicate) -> &'static str {
    match p {
        Predicate::Orient3d => "orient3d",
        Predicate::InSphere => "insphere",
    }
}

fn stage_name(s: Stage) -> &'static str {
    match s {
        Stage::Unknown => "unknown",
        Stage::Filtered => "filtered",
        Stage::Interval => "interval",
        Stage::Bigfloat => "bigfloat",
        Stage::Exact => "exact",
    }
}

fn ratio(n: u64, d: u64) -> f64 {
    if d == 0 { 0.0 } else { n as f64 / d as f64 }
}

impl Stats {
    /// Builds the statistics from the counters of `corpus`.
    pub fn from_corpus(corpus: &Corpus) -> Stats {
        let mut stats = Stats::default();
        for (key, &n) in &corpus.counters {
            let class = (key.predicate, key.implicit);
            *stats.total.entry(class).or_insert(0) += n;
            if key.sign == 0 {
                *stats.zeros.entry(class).or_insert(0) += n;
            }
            *stats
                .stages
                .entry((key.predicate, key.implicit, key.stage))
                .or_insert(0) += n;
        }
        stats
    }

    /// Adds the counts of `other` to these.
    pub fn merge(&mut self, other: &Stats) {
        for (k, n) in &other.total {
            *self.total.entry(*k).or_insert(0) += n;
        }
        for (k, n) in &other.zeros {
            *self.zeros.entry(*k).or_insert(0) += n;
        }
        for (k, n) in &other.stages {
            *self.stages.entry(*k).or_insert(0) += n;
        }
    }

    fn predicate_total(&self, p: Predicate) -> u64 {
        (0..=p.arity() as u8)
            .map(|i| self.total.get(&(p, i)).copied().unwrap_or(0))
            .sum()
    }

    /// For each implicit count from 0 to the arity of `p`: the count, the
    /// number of calls with it, and their fraction of all calls of `p`.
    pub fn fraction_by_implicit(&self, p: Predicate) -> Vec<(u8, u64, f64)> {
        let all = self.predicate_total(p);
        (0..=p.arity() as u8)
            .map(|i| {
                let n = self.total.get(&(p, i)).copied().unwrap_or(0);
                (i, n, ratio(n, all))
            })
            .collect()
    }

    /// For each implicit count from 0 to the arity of `p`: the count, the
    /// number of calls with a zero sign, and their fraction of the calls of
    /// `p` with that implicit count.
    pub fn zero_fraction_by_implicit(&self, p: Predicate) -> Vec<(u8, u64, f64)> {
        (0..=p.arity() as u8)
            .map(|i| {
                let z = self.zeros.get(&(p, i)).copied().unwrap_or(0);
                let n = self.total.get(&(p, i)).copied().unwrap_or(0);
                (i, z, ratio(z, n))
            })
            .collect()
    }

    /// Renders the statistics as Markdown tables, one per predicate.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        for p in PREDICATES {
            let _ = writeln!(out, "### {}\n", name(p));
            let _ = write!(
                out,
                "| implicit | calls | fraction | zero | zero fraction |"
            );
            for s in STAGES {
                let _ = write!(out, " {} |", stage_name(s));
            }
            let _ = writeln!(out);
            let _ = writeln!(
                out,
                "| --- | ---: | ---: | ---: | ---: |{}",
                " ---: |".repeat(STAGES.len())
            );
            let fractions = self.fraction_by_implicit(p);
            let zero_fractions = self.zero_fraction_by_implicit(p);
            for ((i, n, f), (_, z, zf)) in fractions.into_iter().zip(zero_fractions) {
                let _ = write!(out, "| {i} | {n} | {f:.6} | {z} | {zf:.6} |");
                for s in STAGES {
                    let c = self.stages.get(&(p, i, s)).copied().unwrap_or(0);
                    let _ = write!(out, " {c} |");
                }
                let _ = writeln!(out);
            }
            let _ = writeln!(out);
        }
        out
    }
}
