//! Serialization of a corpus.

use super::{CorpusError, MAGIC, VERSION};
use crate::{Corpus, PointDef};
use std::io::Write;

fn len_u64(n: usize) -> u64 {
    u64::try_from(n).expect("length fits in u64")
}

impl Corpus {
    /// Writes the corpus in the on-disk format.
    ///
    /// The corpus is written as is; call [`Corpus::validate`] first to check
    /// it. [`Corpus::read`] always validates.
    pub fn write(&self, mut w: impl Write) -> Result<(), CorpusError> {
        let name = u32::try_from(self.model.len())
            .map_err(|_| CorpusError::Invalid("model name is too long".into()))?;
        w.write_all(MAGIC)?;
        w.write_all(&VERSION.to_le_bytes())?;
        w.write_all(&name.to_le_bytes())?;
        w.write_all(self.model.as_bytes())?;
        for n in [
            len_u64(self.points.len()),
            len_u64(self.counters.len()),
            len_u64(self.sampling.len()),
            len_u64(self.calls.len()),
        ] {
            w.write_all(&n.to_le_bytes())?;
        }
        for point in &self.points {
            write_point(&mut w, point)?;
        }
        for (key, n) in &self.counters {
            w.write_all(&[
                key.predicate.code(),
                key.implicit,
                key.sign as u8,
                key.stage.code(),
            ])?;
            w.write_all(&n.to_le_bytes())?;
        }
        for (stratum, s) in &self.sampling {
            w.write_all(&[
                stratum.predicate.code(),
                stratum.implicit,
                u8::from(stratum.zero),
            ])?;
            w.write_all(&s.total.to_le_bytes())?;
            w.write_all(&s.kept.to_le_bytes())?;
        }
        for call in &self.calls {
            w.write_all(&[call.predicate.code(), call.sign as u8, call.stage.code()])?;
            for p in call.points() {
                w.write_all(&p.to_le_bytes())?;
            }
        }
        Ok(())
    }
}

fn write_indices(w: &mut impl Write, indices: &[u32]) -> std::io::Result<()> {
    indices
        .iter()
        .try_for_each(|i| w.write_all(&i.to_le_bytes()))
}

fn write_point(w: &mut impl Write, point: &PointDef) -> std::io::Result<()> {
    match point {
        PointDef::Explicit(c) => {
            w.write_all(&[0])?;
            c.iter().try_for_each(|v| w.write_all(&v.to_le_bytes()))
        }
        PointDef::Lnc { p, q, t } => {
            w.write_all(&[1])?;
            write_indices(w, &[*p, *q])?;
            w.write_all(&t.to_le_bytes())
        }
        PointDef::Lpi { line, plane } => {
            w.write_all(&[2])?;
            write_indices(w, line)?;
            write_indices(w, plane)
        }
        PointDef::Tpi { planes } => {
            w.write_all(&[3])?;
            planes.iter().try_for_each(|p| write_indices(w, p))
        }
    }
}
