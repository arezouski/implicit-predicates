//! LINEAR-EXPANSION-SUM: the shared algorithm of `sum` and `diff`.

use super::output::Output;
use crate::eft::{fast_two_sum, two_sum};

/// LINEAR-EXPANSION-SUM with zero elimination: writes the expansion of
/// `e + f`, or of `e - f` if `negate_f`, to `out` and returns its length.
pub(super) fn linear_expansion_sum(e: &[f64], f: &[f64], negate_f: bool, out: Output<'_>) -> usize {
    sum_in_magnitude_order(MergedComponents { e, f, negate_f }, out)
}

/// Adds the components of two expansions, already merged by increasing
/// magnitude.
///
/// `high + low` is a running approximation of the sum of the components seen
/// so far; the rest of that sum has already been written out.
fn sum_in_magnitude_order(mut components: MergedComponents<'_>, mut out: Output<'_>) -> usize {
    let Some(first) = components.next() else {
        return out.finish();
    };
    let Some(second) = components.next() else {
        out.push(first);
        return out.finish();
    };
    // The merge order guarantees `|second| >= |first|`.
    let (mut high, mut low) = fast_two_sum(second, first);
    for component in components {
        // The proof of Theorem 24 shows `|low| <= ulp(component)`, so
        // `|low| <= |component|`.
        let (rounded, error) = fast_two_sum(component, low);
        out.push(error);
        (high, low) = two_sum(high, rounded);
    }
    out.push(low);
    out.push(high);
    out.finish()
}

/// The components of two expansions `e` and `f`, merged by increasing
/// magnitude, with those of `f` negated if requested.
struct MergedComponents<'a> {
    e: &'a [f64],
    f: &'a [f64],
    negate_f: bool,
}

impl Iterator for MergedComponents<'_> {
    type Item = f64;

    fn next(&mut self) -> Option<f64> {
        let take_from_e = match (self.e.first(), self.f.first()) {
            (Some(a), Some(b)) => a.abs() <= b.abs(),
            (Some(_), None) => true,
            (None, _) => false,
        };
        if take_from_e {
            let (&component, rest) = self.e.split_first()?;
            self.e = rest;
            Some(component)
        } else {
            let (&component, rest) = self.f.split_first()?;
            self.f = rest;
            Some(if self.negate_f { -component } else { component })
        }
    }
}
