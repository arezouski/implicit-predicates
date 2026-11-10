//! Extraction of a small corpus out of a large one.

use crate::{Call, Corpus, CounterKey, PointDef, Sampling, Stratum};
use std::collections::BTreeMap;

fn parents(point: &PointDef) -> Vec<u32> {
    match point {
        PointDef::Explicit(_) => Vec::new(),
        PointDef::Lnc { p, q, .. } => vec![*p, *q],
        PointDef::Lpi { line, plane } => line.iter().chain(plane).copied().collect(),
        PointDef::Tpi { planes } => planes.iter().flatten().copied().collect(),
    }
}

fn remap(point: &PointDef, map: &[u32]) -> PointDef {
    let m = |i: &u32| map[*i as usize];
    match point {
        PointDef::Explicit(c) => PointDef::Explicit(*c),
        PointDef::Lnc { p, q, t } => PointDef::Lnc {
            p: m(p),
            q: m(q),
            t: *t,
        },
        PointDef::Lpi { line, plane } => PointDef::Lpi {
            line: line.map(|i| m(&i)),
            plane: plane.map(|i| m(&i)),
        },
        PointDef::Tpi { planes } => PointDef::Tpi {
            planes: planes.map(|pl| pl.map(|i| m(&i))),
        },
    }
}

impl Corpus {
    /// Keeps the first `max_calls` stored calls of each stratum, in their
    /// original order, and only the points those calls cite together with
    /// their ancestors. Indices are renumbered; counters and sampling are
    /// recomputed over what was kept, so every stratum has `total == kept`.
    ///
    /// The corpus must be valid (see [`Corpus::validate`]).
    pub fn extract(&self, max_calls: usize) -> Corpus {
        let mut taken: BTreeMap<Stratum, usize> = BTreeMap::new();
        let mut calls: Vec<Call> = Vec::new();
        for call in &self.calls {
            let n = taken.entry(self.stratum_of(call)).or_insert(0);
            if *n < max_calls {
                *n += 1;
                calls.push(*call);
            }
        }
        // Parents precede their children, so one backward pass reaches every
        // ancestor.
        let mut cited = vec![false; self.points.len()];
        for call in &calls {
            for &p in call.points() {
                cited[p as usize] = true;
            }
        }
        for i in (0..self.points.len()).rev() {
            if cited[i] {
                for p in parents(&self.points[i]) {
                    cited[p as usize] = true;
                }
            }
        }
        let mut map = vec![u32::MAX; self.points.len()];
        let mut points = Vec::new();
        for (i, point) in self.points.iter().enumerate() {
            if cited[i] {
                map[i] = points.len() as u32;
                points.push(remap(point, &map));
            }
        }
        let mut counters: BTreeMap<CounterKey, u64> = BTreeMap::new();
        let mut sampling: BTreeMap<Stratum, Sampling> = BTreeMap::new();
        for call in &mut calls {
            let (stratum, key) = (self.stratum_of(call), self.counter_key_of(call));
            *counters.entry(key).or_insert(0) += 1;
            let s = sampling
                .entry(stratum)
                .or_insert(Sampling { total: 0, kept: 0 });
            s.total += 1;
            s.kept += 1;
            for p in &mut call.points[..call.predicate.arity()] {
                *p = map[*p as usize];
            }
        }
        Corpus {
            model: self.model.clone(),
            points,
            counters,
            sampling,
            calls,
        }
    }
}
