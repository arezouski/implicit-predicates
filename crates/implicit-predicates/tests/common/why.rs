//! Why a call reaches the stage on big integers, in the same words for
//! every predicate: no stage on expansions decides it, and the reason is
//! one of the documented limits of those stages.

use implicit_predicates::{AnyPoint, Point};
use implicit_predicates_core::{eft, expansion};

/// Why no stage on expansions decides a call, which the stage on big
/// integers then decides.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Why {
    /// Explicit points of `orient3d` with a coordinate outside its
    /// `UNTRANSLATED_RANGE`, the range of its stages on the stack.
    CoordinateOutOfRange,
    /// `Point::exact` failed with `ExactError::OutOfRange`.
    ExactOutOfRange,
    /// An exact coordinate has more than `MAX_COORD_COMPONENTS` components.
    TooManyComponents,
    /// A component of an exact coordinate is outside
    /// `SPANS_UNTRANSLATED_RANGE`. For a predicate without stages on the
    /// stack, such as `insphere`, explicit points go through the stages
    /// over spans too, and each of their coordinates is one component.
    ComponentOutOfRange,
    /// None of the above: every input is inside the documented ranges, so
    /// some stage on expansions should have decided. The call is decided,
    /// by the stage on big integers, but a sweep counts it as a mismatch.
    Unexplained,
}

/// Why `Point::exact` failed on `point` with `ExactError::OutOfRange`:
/// [`Why::ExactOutOfRange`] if its inputs are outside the range of its
/// exact arithmetic, and [`Why::Unexplained`] if they are inside, where it
/// should not have failed.
pub fn why_exact_failed(point: &AnyPoint) -> Why {
    let in_range = match point {
        AnyPoint::Lnc(l) => lnc_in_exact_range(l.p(), l.q(), l.t()),
        // An explicit point fails only with a coordinate that is not finite.
        AnyPoint::Explicit(_) => point.approx().iter().all(|x| x.is_finite()),
    };
    if in_range {
        Why::Unexplained
    } else {
        Why::ExactOutOfRange
    }
}

/// Whether the exact arithmetic of a point on the line through `p` and `q`
/// with parameter `t` can take them: every parent coordinate is below
/// `2^1020`, and `t` and every component of the exact differences of the
/// parents lie in the product domain.
fn lnc_in_exact_range(p: &[f64; 3], q: &[f64; 3], t: f64) -> bool {
    let limit = 2f64.powi(1020);
    let below = |x: f64| x.abs() < limit;
    eft::in_product_domain(t)
        && (0..3).all(|k| {
            let mut d = [0.0; 2];
            below(p[k]) && below(q[k]) && {
                let n = expansion::from_two_diff(p[k], q[k], &mut d);
                d[..n].iter().all(|&c| eft::in_product_domain(c))
            }
        })
}
