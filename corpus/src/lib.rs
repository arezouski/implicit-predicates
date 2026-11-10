//! Binary file format for recorded geometric predicate calls.
//!
//! A [`Corpus`] holds a point table (explicit and implicit points), the
//! predicate calls recorded over those points, and the counters and sampling
//! statistics of the recording run. See `FORMAT.md` for the on-disk layout.

pub mod exact;
mod extract;
mod format;
mod model;
pub mod stats;
pub mod verify;
pub mod weighted;

pub use format::CorpusError;
pub use model::{Call, Corpus, CounterKey, PointDef, Predicate, Sampling, Stage, Stratum};
