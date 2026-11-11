//! The edge cases of the checks of a per-axis filter straddle its bound,
//! with and without uncertainty, computed here independently of the search.

use super::axis::{Emitted, per_axis};
use super::value_at;
use crate::bound::AxisConstants;
use crate::differences::{DiffGraph, DiffNode};
use crate::dyadic::Dyadic;
use crate::emit::edges::{Case, bound_edge_cases, uncertainty_edge_cases};
use crate::emit::filter::FilterKind;

/// The exact per-axis bound with the emitted constants, for the `d_k`
/// computed in `f64` as the filter computes them, or `None` when some `d_k`
/// is outside `D_MIN..=D_MAX`.
fn axis_bound_at(
    graph: &DiffGraph,
    c: &AxisConstants,
    p: &[[f64; 3]],
    e: [f64; 3],
) -> Option<Dyadic> {
    let mut delta = [0.0f64; 3];
    for node in &graph.nodes {
        if let DiffNode::Leaf {
            axis,
            minuend,
            subtrahend,
        } = *node
        {
            let k = usize::from(axis);
            delta[k] = delta[k].max((p[minuend][k] - p[subtrahend][k]).abs());
        }
    }
    let mut d = [Dyadic::integer(0), Dyadic::integer(0), Dyadic::integer(0)];
    for k in (0..3).filter(|&k| c.degrees[k] >= 1) {
        let dk = delta[k] + e[k];
        if !(c.d_min..=c.d_max).contains(&dk) {
            return None;
        }
        d[k] = Dyadic::from_f64(dk);
    }
    let power = |skip: Option<usize>| {
        let mut x = Dyadic::integer(1);
        for (k, dk) in d.iter().enumerate() {
            for _ in 0..c.degrees[k] - u32::from(skip == Some(k)) {
                x = &x * dk;
            }
        }
        x
    };
    let mut bound = &Dyadic::from_f64(c.epsilon) * &power(None);
    for k in (0..3).filter(|&k| c.degrees[k] >= 1) {
        let term = &(&Dyadic::from_f64(c.kappa[k]) * &Dyadic::from_f64(e[k])) * &power(Some(k));
        bound = &bound + &term;
    }
    Some(bound)
}

/// Checks that `below` and `above` straddle the per-axis bound with `e`,
/// one representable value apart in one coordinate.
fn assert_straddles(e: &Emitted, c: &AxisConstants, below: &Case, above: &Case, u: [f64; 3]) {
    let side = |q: &Case| {
        let bound = axis_bound_at(&e.graph, c, q, u).unwrap_or_else(|| panic!("{q:?}"));
        let value = value_at(&e.predicate.main, q);
        value > bound || value < &Dyadic::integer(0) - &bound
    };
    assert!(!side(below), "{below:?} e = {u:?}");
    assert!(side(above), "{above:?} e = {u:?}");
    let moved: Vec<(f64, f64)> = below
        .iter()
        .flatten()
        .zip(above.iter().flatten())
        .filter(|(a, b)| a.to_bits() != b.to_bits())
        .map(|(&a, &b)| (a, b))
        .collect();
    assert_eq!(moved.len(), 1, "{below:?} {above:?}");
    let (a, b) = moved[0];
    assert!(a.next_up() == b || a.next_down() == b, "{a:e} {b:e}");
}

#[test]
fn the_edge_cases_straddle_the_per_axis_bound() {
    for (e, constants, _) in per_axis()
        .into_iter()
        .filter(|(e, ..)| e.predicate.name == "orient3d")
    {
        let points = e.predicate.params.len();
        let explicit = bound_edge_cases(&e.graph, points, &FilterKind::PerAxis(constants, true));
        assert!(explicit.len() >= 32, "{} pairs", explicit.len());
        for (below, above) in &explicit {
            assert_straddles(&e, &constants, below, above, [0.0; 3]);
        }
        let uncertain = uncertainty_edge_cases(&e.graph, points, &constants);
        // On each axis alone, and on the three together.
        let mut kinds = [0usize; 4];
        for (u, below, above) in &uncertain {
            assert!(u.iter().all(|&x| x >= 0.0 && x.is_finite()), "{u:?}");
            let nonzero: Vec<usize> = (0..3).filter(|&k| u[k] > 0.0).collect();
            match nonzero.as_slice() {
                [k] => kinds[*k] += 1,
                [0, 1, 2] => kinds[3] += 1,
                _ => panic!("e = {u:?}"),
            }
            assert_straddles(&e, &constants, below, above, *u);
        }
        assert!(kinds.iter().all(|&n| n >= 8), "{kinds:?}");
    }
}
