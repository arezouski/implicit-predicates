//! The points of a recorded corpus as points of this crate, with the
//! correspondence between the two definitions of a point on a line in one
//! place.

use implicit_predicates::{AnyPoint, Explicit, Lnc, PointError};
use implicit_predicates_corpus::{Call, Corpus, PointDef};

/// The points of a corpus, in the order of its point table.
#[derive(Debug, Default)]
pub struct Mapped {
    /// Each point of the table, or `None` for a kind of point this crate
    /// does not provide yet, or one whose inputs it rejects.
    pub points: Vec<Option<AnyPoint>>,
    /// Points of a kind this crate does not provide yet.
    pub unsupported: usize,
    /// Points whose inputs are not finite, mapped to `None`.
    pub non_finite: usize,
    /// Points on a line whose parameter is outside `[0, 1]`: `Lnc::new`
    /// rejects them, so they are built with `Lnc::new_unchecked`, which
    /// accepts any finite parameter with the same exact coordinates and a
    /// valid error bound. They are counted to be reported, not dropped.
    pub unchecked: usize,
}

/// The coordinates of the explicit point at `index` of the table.
///
/// # Panics
///
/// Panics if the point is not explicit: a valid corpus has explicit parents
/// only.
fn parent(points: &[PointDef], index: u32) -> [f64; 3] {
    match points[index as usize] {
        PointDef::Explicit(c) => c,
        ref other => panic!("the parent {index} is not explicit: {other:?}"),
    }
}

/// Maps every point of `corpus` to a point of this crate.
///
/// A corpus point on the line through its parents `p` and `q` is
/// `p + t (q - p)`, while `Lnc { p, q, t }` is `q + t (p - q)`: the corpus
/// point is `Lnc { p: q, q: p, t }`, the same point exactly.
pub fn points(corpus: &Corpus) -> Mapped {
    let mut mapped = Mapped::default();
    for def in &corpus.points {
        let point = match *def {
            PointDef::Explicit(c) => Explicit::new(c).map(AnyPoint::Explicit),
            PointDef::Lnc { p, q, t } => {
                let (p, q) = (parent(&corpus.points, p), parent(&corpus.points, q));
                match Lnc::new(q, p, t) {
                    Err(PointError::ParamOutOfRange) => {
                        mapped.unchecked += 1;
                        Ok(Lnc::new_unchecked(q, p, t))
                    }
                    other => other,
                }
                .map(AnyPoint::Lnc)
            }
            PointDef::Lpi { .. } | PointDef::Tpi { .. } => {
                mapped.unsupported += 1;
                mapped.points.push(None);
                continue;
            }
        };
        mapped.non_finite += usize::from(point.is_err());
        mapped.points.push(point.ok());
    }
    mapped
}

/// The points of `call`, or `None` if one of them is `None` in `mapped`.
pub fn call_points<const N: usize>(mapped: &Mapped, call: &Call) -> Option<[AnyPoint; N]> {
    let indices = call.points();
    assert_eq!(indices.len(), N, "a call with {} points", indices.len());
    let mut out = [None; N];
    for (slot, &index) in out.iter_mut().zip(indices) {
        *slot = Some(mapped.points[index as usize]?);
    }
    Some(out.map(|p| p.expect("every slot is filled")))
}

/// How many points of `call` are not explicit in `corpus`, as the corpus
/// counts them for its strata.
pub fn implicit_count(corpus: &Corpus, call: &Call) -> usize {
    call.points()
        .iter()
        .filter(|&&i| !matches!(corpus.points[i as usize], PointDef::Explicit(_)))
        .count()
}
