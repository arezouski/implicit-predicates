//! Data model of a corpus and its validity rules.

use crate::CorpusError;
use std::collections::BTreeMap;

/// A geometric predicate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Predicate {
    /// Orientation of four points (code 0).
    Orient3d,
    /// Position of a point relative to the sphere through four points (code 1).
    InSphere,
}

impl Predicate {
    /// Number of points the predicate takes.
    pub fn arity(self) -> usize {
        match self {
            Predicate::Orient3d => 4,
            Predicate::InSphere => 5,
        }
    }

    pub(crate) fn code(self) -> u8 {
        self as u8
    }

    pub(crate) fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Predicate::Orient3d),
            1 => Some(Predicate::InSphere),
            _ => None,
        }
    }
}

/// The stage of the evaluation cascade that decided a call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stage {
    /// The recording program did not report a stage (code 0).
    Unknown,
    /// Decided by the floating-point filter (code 1).
    Filtered,
    /// Decided by interval arithmetic (code 2).
    Interval,
    /// Decided by multi-precision floating point (code 3).
    Bigfloat,
    /// Decided by exact arithmetic (code 4).
    Exact,
}

impl Stage {
    pub(crate) fn code(self) -> u8 {
        self as u8
    }

    pub(crate) fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Stage::Unknown),
            1 => Some(Stage::Filtered),
            2 => Some(Stage::Interval),
            3 => Some(Stage::Bigfloat),
            4 => Some(Stage::Exact),
            _ => None,
        }
    }
}

/// Definition of one point of the point table.
#[derive(Debug, Clone, PartialEq)]
pub enum PointDef {
    /// A point given by its coordinates (code 0).
    Explicit([f64; 3]),
    /// A point on the line through two explicit points (code 1).
    ///
    /// `t = 0` is the first parent `p` and `t = 1` is the second parent `q`.
    Lnc {
        /// Index of the first parent.
        p: u32,
        /// Index of the second parent.
        q: u32,
        /// Position along the line.
        t: f64,
    },
    /// Intersection of a line with a plane (code 2).
    Lpi {
        /// Indices of the two points defining the line.
        line: [u32; 2],
        /// Indices of the three points defining the plane.
        plane: [u32; 3],
    },
    /// Intersection of three planes (code 3).
    Tpi {
        /// Indices of the three points defining each plane.
        planes: [[u32; 3]; 3],
    },
}

/// One recorded predicate call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Call {
    /// The predicate that was evaluated.
    pub predicate: Predicate,
    /// Point indices. Only the first [`Predicate::arity`] entries are
    /// meaningful; the rest are zero.
    pub points: [u32; 5],
    /// The sign returned by the recording program.
    pub sign: i8,
    /// The stage that decided the call.
    pub stage: Stage,
}

impl Call {
    /// The point indices the call actually uses (4 or 5 of them).
    pub fn points(&self) -> &[u32] {
        &self.points[..self.predicate.arity()]
    }
}

/// Key of a counter: how many calls ended with a given outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CounterKey {
    /// The predicate.
    pub predicate: Predicate,
    /// Number of implicit points among the call's points.
    pub implicit: u8,
    /// The sign returned.
    pub sign: i8,
    /// The deciding stage.
    pub stage: Stage,
}

/// A sampling stratum: calls of one predicate with one implicit-point count,
/// split by whether the sign is zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Stratum {
    /// The predicate.
    pub predicate: Predicate,
    /// Number of implicit points among the call's points.
    pub implicit: u8,
    /// Whether the returned sign is zero.
    pub zero: bool,
}

/// Sampling statistics of one stratum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sampling {
    /// Calls of the stratum seen during recording.
    pub total: u64,
    /// Calls of the stratum stored in the corpus.
    pub kept: u64,
}

/// A recorded set of predicate calls over a point table.
#[derive(Debug, Clone, PartialEq)]
pub struct Corpus {
    /// Name of the model the points and calls were recorded from.
    pub model: String,
    /// The point table.
    pub points: Vec<PointDef>,
    /// Outcome counters of the whole recording run.
    pub counters: BTreeMap<CounterKey, u64>,
    /// Sampling statistics per stratum.
    pub sampling: BTreeMap<Stratum, Sampling>,
    /// The stored calls.
    pub calls: Vec<Call>,
}

fn invalid(msg: impl Into<String>) -> CorpusError {
    CorpusError::Invalid(msg.into())
}

impl Corpus {
    /// Checks every validity rule of the format.
    ///
    /// Parents precede their children, LNC parents are explicit, coordinates
    /// and parameters are finite, signs are -1, 0 or 1, call indices exist,
    /// and the sampling and counters agree with the stored calls.
    pub fn validate(&self) -> Result<(), CorpusError> {
        self.validate_points()?;
        self.validate_calls()?;
        self.validate_statistics()
    }

    fn is_explicit(&self, index: u32) -> bool {
        matches!(self.points[index as usize], PointDef::Explicit(_))
    }

    fn validate_points(&self) -> Result<(), CorpusError> {
        for (i, point) in self.points.iter().enumerate() {
            let parents: Vec<u32> = match point {
                PointDef::Explicit(c) => {
                    if c.iter().any(|v| !v.is_finite()) {
                        return Err(invalid(format!("point {i}: non-finite coordinate")));
                    }
                    Vec::new()
                }
                PointDef::Lnc { p, q, t } => {
                    if !t.is_finite() {
                        return Err(invalid(format!("point {i}: non-finite parameter")));
                    }
                    vec![*p, *q]
                }
                PointDef::Lpi { line, plane } => line.iter().chain(plane).copied().collect(),
                PointDef::Tpi { planes } => planes.iter().flatten().copied().collect(),
            };
            if parents.iter().any(|&p| p as usize >= i) {
                return Err(invalid(format!("point {i}: parent does not precede it")));
            }
            if matches!(point, PointDef::Lnc { .. })
                && parents.iter().any(|&p| !self.is_explicit(p))
            {
                return Err(invalid(format!("point {i}: LNC parent is not explicit")));
            }
        }
        Ok(())
    }

    fn implicit_count(&self, call: &Call) -> usize {
        call.points()
            .iter()
            .filter(|&&p| !self.is_explicit(p))
            .count()
    }

    fn validate_calls(&self) -> Result<(), CorpusError> {
        for (i, call) in self.calls.iter().enumerate() {
            if !(-1..=1).contains(&call.sign) {
                return Err(invalid(format!(
                    "call {i}: sign {} is not -1, 0 or 1",
                    call.sign
                )));
            }
            if call
                .points()
                .iter()
                .any(|&p| p as usize >= self.points.len())
            {
                return Err(invalid(format!("call {i}: point index out of range")));
            }
            if call.points[call.predicate.arity()..]
                .iter()
                .any(|&p| p != 0)
            {
                return Err(invalid(format!("call {i}: unused point slot is not zero")));
            }
        }
        Ok(())
    }

    fn validate_statistics(&self) -> Result<(), CorpusError> {
        let mut kept: BTreeMap<Stratum, u64> = BTreeMap::new();
        for call in &self.calls {
            let stratum = Stratum {
                predicate: call.predicate,
                implicit: self.implicit_count(call) as u8,
                zero: call.sign == 0,
            };
            *kept.entry(stratum).or_insert(0) += 1;
        }
        for (stratum, n) in &kept {
            if self.sampling.get(stratum).map(|s| s.kept) != Some(*n) {
                return Err(invalid(format!(
                    "sampling of {stratum:?} does not match the calls"
                )));
            }
        }
        let mut totals: BTreeMap<Stratum, u64> = BTreeMap::new();
        for (key, n) in &self.counters {
            if !(-1..=1).contains(&key.sign) || usize::from(key.implicit) > key.predicate.arity() {
                return Err(invalid(format!("counter {key:?} is out of range")));
            }
            let stratum = Stratum {
                predicate: key.predicate,
                implicit: key.implicit,
                zero: key.sign == 0,
            };
            let total = totals.entry(stratum).or_insert(0u64);
            *total = total
                .checked_add(*n)
                .ok_or_else(|| invalid("counter sum overflows"))?;
        }
        for (stratum, s) in &self.sampling {
            if s.kept > s.total {
                return Err(invalid(format!("{stratum:?}: kept exceeds total")));
            }
            if usize::from(stratum.implicit) > stratum.predicate.arity() {
                return Err(invalid(format!("{stratum:?}: implicit count out of range")));
            }
            if kept.get(stratum).copied().unwrap_or(0) != s.kept {
                return Err(invalid(format!(
                    "sampling of {stratum:?} does not match the calls"
                )));
            }
            if totals.get(stratum).copied().unwrap_or(0) != s.total {
                return Err(invalid(format!(
                    "counters of {stratum:?} do not add up to its total"
                )));
            }
        }
        if totals.keys().any(|k| !self.sampling.contains_key(k)) {
            return Err(invalid(
                "a counter belongs to a stratum with no sampling entry",
            ));
        }
        Ok(())
    }
}
