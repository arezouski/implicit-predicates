//! Parsing of a corpus.
//!
//! Vectors grow as data is read; nothing is reserved from header counts, so a
//! hostile header cannot force a large allocation.

use super::{CorpusError, MAGIC, VERSION};
use crate::{Call, Corpus, CounterKey, PointDef, Predicate, Sampling, Stage, Stratum};
use std::collections::BTreeMap;
use std::io::Read;

fn bad(msg: impl Into<String>) -> CorpusError {
    CorpusError::Format(msg.into())
}

struct Reader<R>(R);

impl<R: Read> Reader<R> {
    fn array<const N: usize>(&mut self) -> Result<[u8; N], CorpusError> {
        let mut buf = [0u8; N];
        self.0.read_exact(&mut buf)?;
        Ok(buf)
    }

    fn u8(&mut self) -> Result<u8, CorpusError> {
        Ok(self.array::<1>()?[0])
    }

    fn i8(&mut self) -> Result<i8, CorpusError> {
        Ok(self.u8()? as i8)
    }

    fn u32(&mut self) -> Result<u32, CorpusError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, CorpusError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn f64(&mut self) -> Result<f64, CorpusError> {
        Ok(f64::from_le_bytes(self.array()?))
    }

    fn indices<const N: usize>(&mut self) -> Result<[u32; N], CorpusError> {
        let mut out = [0u32; N];
        for slot in &mut out {
            *slot = self.u32()?;
        }
        Ok(out)
    }

    fn predicate(&mut self) -> Result<Predicate, CorpusError> {
        let code = self.u8()?;
        Predicate::from_code(code).ok_or_else(|| bad(format!("unknown predicate code {code}")))
    }

    fn stage(&mut self) -> Result<Stage, CorpusError> {
        let code = self.u8()?;
        Stage::from_code(code).ok_or_else(|| bad(format!("unknown stage code {code}")))
    }

    fn point(&mut self) -> Result<PointDef, CorpusError> {
        match self.u8()? {
            0 => Ok(PointDef::Explicit([self.f64()?, self.f64()?, self.f64()?])),
            1 => {
                let [p, q] = self.indices()?;
                Ok(PointDef::Lnc {
                    p,
                    q,
                    t: self.f64()?,
                })
            }
            2 => Ok(PointDef::Lpi {
                line: self.indices()?,
                plane: self.indices()?,
            }),
            3 => Ok(PointDef::Tpi {
                planes: [self.indices()?, self.indices()?, self.indices()?],
            }),
            code => Err(bad(format!("unknown point kind {code}"))),
        }
    }

    fn call(&mut self) -> Result<Call, CorpusError> {
        let predicate = self.predicate()?;
        let sign = self.i8()?;
        let stage = self.stage()?;
        let mut points = [0u32; 5];
        for slot in &mut points[..predicate.arity()] {
            *slot = self.u32()?;
        }
        Ok(Call {
            predicate,
            points,
            sign,
            stage,
        })
    }

    fn model_name(&mut self) -> Result<String, CorpusError> {
        let len = u64::from(self.u32()?);
        let mut bytes = Vec::new();
        (&mut self.0).take(len).read_to_end(&mut bytes)?;
        if bytes.len() as u64 != len {
            return Err(bad("file ends inside the model name"));
        }
        String::from_utf8(bytes).map_err(|_| bad("model name is not UTF-8"))
    }
}

impl Corpus {
    /// Reads and validates a corpus.
    ///
    /// Fails if the bytes are truncated, have trailing data, use unknown
    /// codes, or violate a rule checked by [`Corpus::validate`].
    pub fn read(r: impl Read) -> Result<Corpus, CorpusError> {
        let mut r = Reader(r);
        if &r.array::<8>()? != MAGIC {
            return Err(bad("bad magic"));
        }
        let version = r.u32()?;
        if version != VERSION {
            return Err(bad(format!("unsupported version {version}")));
        }
        let model = r.model_name()?;
        let (n_points, n_counters, n_strata, n_calls) = (r.u64()?, r.u64()?, r.u64()?, r.u64()?);

        let mut points = Vec::new();
        for _ in 0..n_points {
            points.push(r.point()?);
        }
        let mut counters = BTreeMap::new();
        for _ in 0..n_counters {
            let key = CounterKey {
                predicate: r.predicate()?,
                implicit: r.u8()?,
                sign: r.i8()?,
                stage: r.stage()?,
            };
            if counters.insert(key, r.u64()?).is_some() {
                return Err(bad("duplicate counter"));
            }
        }
        let mut sampling = BTreeMap::new();
        for _ in 0..n_strata {
            let predicate = r.predicate()?;
            let implicit = r.u8()?;
            let zero = match r.u8()? {
                0 => false,
                1 => true,
                v => return Err(bad(format!("zero flag {v} is not 0 or 1"))),
            };
            let s = Sampling {
                total: r.u64()?,
                kept: r.u64()?,
            };
            if sampling
                .insert(
                    Stratum {
                        predicate,
                        implicit,
                        zero,
                    },
                    s,
                )
                .is_some()
            {
                return Err(bad("duplicate stratum"));
            }
        }
        let mut calls = Vec::new();
        for _ in 0..n_calls {
            calls.push(r.call()?);
        }
        if r.0.read(&mut [0u8])? != 0 {
            return Err(bad("trailing bytes after the last call"));
        }
        let corpus = Corpus {
            model,
            points,
            counters,
            sampling,
            calls,
        };
        corpus.validate()?;
        Ok(corpus)
    }
}
