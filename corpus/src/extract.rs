//! Extraction of a small corpus out of a large one.

use crate::{Call, Corpus, PointDef, Sampling, Stratum};
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
    /// their ancestors. Indices are renumbered. The counters and each
    /// stratum's `total` are those of the recording run, unchanged; only
    /// `kept` shrinks to the number of calls kept.
    ///
    /// The corpus must be valid (see [`Corpus::validate`]).
    pub fn extract(&self, max_calls: usize) -> Corpus {
        let mut sampling: BTreeMap<Stratum, Sampling> = self
            .sampling
            .iter()
            .map(|(s, sampling)| {
                (
                    *s,
                    Sampling {
                        kept: 0,
                        ..*sampling
                    },
                )
            })
            .collect();
        let mut calls: Vec<Call> = Vec::new();
        for call in &self.calls {
            let s = sampling
                .get_mut(&self.stratum_of(call))
                .expect("a valid corpus has a sampling entry for every stored call");
            if s.kept < max_calls as u64 {
                s.kept += 1;
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
        for call in &mut calls {
            for p in &mut call.points[..call.predicate.arity()] {
                *p = map[*p as usize];
            }
        }
        Corpus {
            model: self.model.clone(),
            points,
            counters: self.counters.clone(),
            sampling,
            calls,
        }
    }
}
