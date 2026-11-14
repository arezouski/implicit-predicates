//! Inputs at the edge of the error bound of a filter, built with exact
//! arithmetic for the generated checks.
//!
//! Each case starts from points on a grid, small integers times a power of
//! two, where the last point is an integer affine combination of the first
//! three: for an orientation these points are exactly coplanar and the
//! value of the form is zero. When it is not zero, the coordinate to move is
//! first walked, by bisection over the ordered `f64`s, to where the exact
//! value changes sign. That coordinate is then moved one representable
//! value at a time, in the order of the `f64`s, until the exact magnitude
//! of the form exceeds the bound of the filter, computed exactly with the
//! emitted constants and with the sizes as the filter computes them in
//! `f64`: `EPSILON * delta^k` for the global form with `e = 0`, and for the
//! per-axis form `EPSILON * d^G` plus `KAPPA[k] * e[k] * d^G / d_k` on each
//! axis, with `d_k = delta_k + e[k]`. The last input on each side of the
//! bound is kept. The search doubles the step and then bisects, so it takes
//! a few dozen exact evaluations.
//!
//! For the second test of the filter, `filter_axes`, [`axes_edge_cases`]
//! does the same with the bound by parts, the sum over the terms of the
//! emitted constants ([`PartsConstants`]) of `value * d^degrees` for a term
//! of epsilon and `value * e[axis] * d^rest` for a term of kappa, with
//! `d_k = delta_k + e[k]` computed in `f64` on each axis of its range. Its
//! grids also have a scale of their own on each axis, one axis `2^-20` or
//! `2^-40` times as short or as long as the other two, and an `e` that is
//! not zero on one axis of the range, for each, or on all of them: so the
//! pairs reach the terms of kappa, and the terms of epsilon of a degree
//! vector that a grid of one scale leaves small.
//!
//! [`bound_edge_cases`] takes `e = 0`, as for explicit points;
//! [`uncertainty_edge_cases`], for a per-axis filter, takes an `e` that is
//! not zero on one axis only, for each axis, or on the three: a power of
//! two between `2^-40` and `2^-8` times the scale of the grid on each axis
//! where it is not zero.
//!
//! The scales of the grids range from just above `D_MIN` to just below
//! `D_MAX`. Every choice comes from a fixed pseudo-random sequence, so the
//! cases are the same on every run.

use super::filter::FilterKind;
use super::grid::{FINITE_KEYS, GRID, SplitMix64, exponent, from_key, grid_points_axes, key, pow2};
use crate::bound::{AxisConstants, PartsConstants};
use crate::differences::{DiffGraph, DiffNode};
use crate::dyadic::Dyadic;

/// The coordinates of the points of one case.
pub(crate) type Case = Vec<[f64; 3]>;

/// The most nodes of a difference form for which edge cases are searched.
///
/// The search makes at most `8 * 64` attempts with `e = 0`, for a per-axis
/// filter `8 * 4 * 16` more with uncertainty, and for a predicate with
/// `filter_axes` `8 * 64` more for its bound by parts and at most
/// `34 * 3 * 6` with other scales and uncertainties, each with at most
/// about 330 exact evaluations of the whole form, so its work is
/// proportional to the number of nodes; at 256 nodes the slowest form
/// found, a long sum of one difference, takes a few seconds in a debug
/// build.
pub(crate) const MAX_NODES: usize = 256;

/// The cases built at each scale with `e = 0`, and the attempts allowed for
/// them.
const PER_SCALE: usize = 6;
const ATTEMPTS_PER_SCALE: usize = 64;

/// The cases built at each scale for each choice of the axes of `e`, and
/// the attempts allowed for them.
const UNCERTAIN_PER_SCALE: usize = 2;
const UNCERTAIN_ATTEMPTS_PER_SCALE: usize = 16;

/// The attempts allowed for each case of [`axes_edge_cases`] with scales
/// of their own on the axes or an uncertainty, each tried at about a third
/// of the scales, at most three.
const SHAPED_ATTEMPTS: usize = 6;

/// How many times the terms of kappa of the bound by parts must exceed its
/// terms of epsilon in a case of [`axes_edge_cases`] with an uncertainty:
/// enough that halving or doubling `e` moves the bound far more than the
/// rounding of the computed value and of the bound can.
pub(crate) const KAPPA_OVER_EPSILON: i64 = 10;

/// The binary exponents by which one axis of a grid of [`axes_edge_cases`]
/// is shorter or longer than the other two.
pub(crate) const SHAPE_SHIFTS: [i32; 2] = [20, 40];

/// The binary exponents below the scale of an axis between which the
/// uncertainty of a case of [`axes_edge_cases`] is on that axis, both
/// included: `e[k]` is a power of two from `2^-AXES_E_SHIFTS.1` to
/// `2^-AXES_E_SHIFTS.0` times the scale of axis `k`.
pub(crate) const AXES_E_SHIFTS: (i32, i32) = (8, 30);

/// The exponent of the smallest normal `f64`, `2^-1022`, the smallest that
/// [`pow2`] takes.
const MIN_NORMAL_EXPONENT: i32 = -1022;

/// Pairs of inputs on either side of the bound of the `filter` of `graph`
/// with `e = 0`, with `points` points: the first of each pair at or below
/// it, the second above it, one representable value apart in one
/// coordinate.
pub(crate) fn bound_edge_cases(
    graph: &DiffGraph,
    points: usize,
    filter: &FilterKind,
) -> Vec<(Case, Case)> {
    let form = Form {
        graph,
        bound: Bound::Filter(filter),
    };
    let mut random = SplitMix64(0x2545_f491_4f6c_dd1d);
    let mut cases = Vec::new();
    for scale in form.scales() {
        let found = form.search(
            &mut random,
            points,
            [scale; 3],
            [0.0; 3],
            PER_SCALE,
            ATTEMPTS_PER_SCALE,
        );
        cases.extend(found);
    }
    cases
}

/// Pairs of inputs on either side of the bound of the per-axis filter of
/// `graph`, with the emitted `constants`, each with its `e`: not zero on one
/// axis of positive degree only, for each such axis, and not zero on all of
/// them. The pairs are as in [`bound_edge_cases`].
pub(crate) fn uncertainty_edge_cases(
    graph: &DiffGraph,
    points: usize,
    constants: &AxisConstants,
) -> Vec<([f64; 3], Case, Case)> {
    let filter = FilterKind::PerAxis(*constants, false);
    let form = Form {
        graph,
        bound: Bound::Filter(&filter),
    };
    let axes: Vec<usize> = (0..3).filter(|&k| constants.degrees[k] >= 1).collect();
    let mut choices: Vec<Vec<usize>> = axes.iter().map(|&k| vec![k]).collect();
    if axes.len() > 1 {
        choices.push(axes.clone());
    }
    let mut random = SplitMix64(0x9fb2_1c65_1e98_df25);
    let mut cases = Vec::new();
    for scale in form.scales() {
        for choice in &choices {
            let mut e = [0.0; 3];
            for &k in choice {
                e[k] = pow2(scale - 8 - random.below(33) as i32);
            }
            let found = form.search(
                &mut random,
                points,
                [scale; 3],
                e,
                UNCERTAIN_PER_SCALE,
                UNCERTAIN_ATTEMPTS_PER_SCALE,
            );
            cases.extend(found.into_iter().map(|(below, above)| (e, below, above)));
        }
    }
    cases
}

/// Pairs of inputs on either side of the bound by parts of `filter_axes`,
/// for the difference form `graph` with `points` points and the emitted
/// `constants`, each with its `e`. The pairs are as in [`bound_edge_cases`].
///
/// First, pairs with `e = 0` on grids of one scale. Then pairs on grids of
/// a scale of their own on each axis, one axis `2^-20` or `2^-40` times as
/// short or as long as the other two, with `e = 0` and with an `e` that is
/// not zero on one axis of the range, for each, or on all of them: a power
/// of two between `2^-AXES_E_SHIFTS.1` and `2^-AXES_E_SHIFTS.0` times the
/// scale of the grid on that axis. A grid with the scale of some axis, in
/// the range or not, below that of the smallest normal `f64` is skipped. A
/// pair with an uncertainty is kept only when the terms of kappa exceed
/// `2^KAPPA_OVER_EPSILON` times the terms of epsilon at both inputs,
/// evaluated exactly as for the edge, and when the sizes with `e / 2` are
/// in the range too: then `filter_axes` must decline the first input with
/// `2 e`, whose bound is near twice the exact value, and decide the second
/// with `e / 2`, whose bound is near half of it.
pub(crate) fn axes_edge_cases(
    graph: &DiffGraph,
    points: usize,
    constants: &PartsConstants,
) -> Vec<([f64; 3], Case, Case)> {
    let form = Form {
        graph,
        bound: Bound::Parts(constants),
    };
    let mut random = SplitMix64(0x6a09_e667_f3bc_c909);
    let mut cases = Vec::new();
    let scales = form.scales();
    for &scale in &scales {
        let found = form.search(
            &mut random,
            points,
            [scale; 3],
            [0.0; 3],
            PER_SCALE,
            ATTEMPTS_PER_SCALE,
        );
        cases.extend(
            found
                .into_iter()
                .map(|(below, above)| ([0.0; 3], below, above)),
        );
    }
    let axes: Vec<usize> = (0..3).filter(|&k| constants.range_axes[k]).collect();
    let mut choices: Vec<Vec<usize>> = vec![Vec::new()];
    choices.extend(axes.iter().map(|&k| vec![k]));
    if axes.len() > 1 {
        choices.push(axes.clone());
    }
    // One scale per axis: all the same, or one axis shorter, or longer,
    // than the other two, as a shift of the exponent of each axis.
    let mut shapes: Vec<Option<(usize, bool)>> = vec![None];
    for &k in &axes {
        shapes.push(Some((k, false)));
        shapes.push(Some((k, true)));
    }
    let (low, high) = form.range();
    let in_range = |s: i32| low < s && s + 8 <= high;
    let mut random = SplitMix64(0xbb67_ae85_84ca_a73b);
    let mut combination = 0usize;
    for shape in &shapes {
        for choice in &choices {
            if shape.is_none() && choice.is_empty() {
                continue;
            }
            combination += 1;
            for (index, &scale) in scales.iter().enumerate() {
                if (index + combination) % 3 != 0 {
                    continue;
                }
                let shift = SHAPE_SHIFTS[random.below(2) as usize];
                let Some(axis_scales) = shaped_scales(scale, *shape, shift) else {
                    continue;
                };
                if !axes.iter().all(|&k| in_range(axis_scales[k])) {
                    continue;
                }
                let mut e = [0.0; 3];
                let mut normal = true;
                for &k in choice {
                    let (nearest, farthest) = AXES_E_SHIFTS;
                    let spread = (farthest - nearest + 1) as u64;
                    let exponent = axis_scales[k] - nearest - random.below(spread) as i32;
                    // `e / 2` must be a normal power of two too.
                    normal &= exponent > MIN_NORMAL_EXPONENT;
                    if normal {
                        e[k] = pow2(exponent);
                    }
                }
                if !normal {
                    continue;
                }
                let found = form.search(&mut random, points, axis_scales, e, 1, SHAPED_ATTEMPTS);
                cases.extend(
                    found
                        .into_iter()
                        .filter(|(below, above)| {
                            choice.is_empty() || form.margins_hold(constants, [below, above], e)
                        })
                        .map(|(below, above)| (e, below, above)),
                );
            }
        }
    }
    cases
}

/// The scale of each axis of a grid of [`axes_edge_cases`] of scale `scale`
/// and shape `shape`: every axis at `scale` for `None`, and for
/// `Some((j, long))` axis `j` `shift` below the others, or above them when
/// `long`, the others at `scale`. `None` when the scale of an axis, in the
/// range of the bound or not, is below that of the smallest normal `f64`,
/// so that the grid can be built with [`pow2`].
pub(crate) fn shaped_scales(
    scale: i32,
    shape: Option<(usize, bool)>,
    shift: i32,
) -> Option<[i32; 3]> {
    let scales: [i32; 3] = std::array::from_fn(|k| match shape {
        None => scale,
        // Axis `j` shorter than the others, or the others shorter than it.
        Some((j, long)) => {
            if (k == j) != long {
                scale - shift
            } else {
                scale
            }
        }
    });
    scales
        .iter()
        .all(|&s| s >= MIN_NORMAL_EXPONENT)
        .then_some(scales)
}

/// The bound whose edge is searched: that of the filter, or that of
/// `filter_axes`.
enum Bound<'a> {
    Filter(&'a FilterKind),
    Parts(&'a PartsConstants),
}

/// A difference form with the constants of the bound whose edge is
/// searched.
struct Form<'a> {
    graph: &'a DiffGraph,
    bound: Bound<'a>,
}

impl Form<'_> {
    /// `D_MIN` and `D_MAX`, as exponents.
    fn range(&self) -> (i32, i32) {
        let (d_min, d_max) = match self.bound {
            Bound::Filter(FilterKind::Global(c)) => (c.d_min, c.d_max),
            Bound::Filter(FilterKind::PerAxis(c, _)) => (c.d_min, c.d_max),
            Bound::Parts(c) => (c.d_min, c.d_max),
        };
        (exponent(d_min), exponent(d_max))
    }

    /// The scales of the grids, from just above `D_MIN` to just below
    /// `D_MAX`: the differences are between `2^scale` and `2^(scale + 8)`.
    fn scales(&self) -> Vec<i32> {
        let (low, high) = self.range();
        [low + 1, low + 40, -100, -30, 0, 30, 100, high - 12]
            .into_iter()
            .filter(|&s| low < s && s + 8 <= high)
            .collect()
    }

    /// Up to `wanted` pairs of inputs on either side of the bound with
    /// uncertainty `e`, from grids at `scales`, one per axis, in at most
    /// `attempts` attempts.
    fn search(
        &self,
        random: &mut SplitMix64,
        points: usize,
        scales: [i32; 3],
        e: [f64; 3],
        wanted: usize,
        attempts: usize,
    ) -> Vec<(Case, Case)> {
        let mut cases = Vec::new();
        for _ in 0..attempts {
            if cases.len() == wanted {
                break;
            }
            let start = grid_points_axes(random, points, scales);
            let moved: Vec<(usize, usize)> = (0..points)
                .flat_map(|i| (0..3).map(move |axis| (i, axis)))
                .filter(|&(i, axis)| start[i][axis] != 0.0)
                .collect();
            if moved.is_empty() {
                continue;
            }
            let (point, axis) = moved[random.below(moved.len() as u64) as usize];
            let up = random.below(2) == 0;
            let line = Line {
                form: self,
                start,
                point,
                axis,
                e,
            };
            if let Some(pair) = line.edge_pair(scales[axis], up) {
                cases.push(pair);
            }
        }
        cases
    }

    /// The exact value of the form at `p`, node by node in the order of the
    /// graph.
    fn value(&self, p: &[[f64; 3]]) -> Dyadic {
        let mut values: Vec<Dyadic> = Vec::with_capacity(self.graph.nodes.len());
        for node in &self.graph.nodes {
            let value = match *node {
                DiffNode::Leaf {
                    axis,
                    minuend,
                    subtrahend,
                } => {
                    let axis = usize::from(axis);
                    &Dyadic::from_f64(p[minuend][axis]) - &Dyadic::from_f64(p[subtrahend][axis])
                }
                DiffNode::Neg(x) => &Dyadic::integer(0) - &values[x],
                DiffNode::Add(x, y) => &values[x] + &values[y],
                DiffNode::Sub(x, y) => &values[x] - &values[y],
                DiffNode::Mul(x, y) => &values[x] * &values[y],
            };
            values.push(value);
        }
        values.swap_remove(self.graph.root)
    }

    /// Whether the exact magnitude of the form at `p` exceeds the bound of
    /// the filter with uncertainty `e`, or `None` when a size computed in
    /// `f64` as the filter computes it is outside `D_MIN..=D_MAX`.
    fn above_bound(&self, p: &[[f64; 3]], e: [f64; 3]) -> Option<bool> {
        let delta = self.delta(p);
        let bound = match self.bound {
            Bound::Filter(FilterKind::Global(c)) => {
                let delta = delta[0].max(delta[1]).max(delta[2]);
                if !(c.d_min..=c.d_max).contains(&delta) {
                    return None;
                }
                let delta = Dyadic::from_f64(delta);
                let mut bound = Dyadic::from_f64(c.epsilon);
                for _ in 0..c.degree {
                    bound = &bound * &delta;
                }
                bound
            }
            Bound::Filter(FilterKind::PerAxis(c, _)) => axis_bound(c, delta, e)?,
            Bound::Parts(c) => {
                let (epsilon, kappa) = parts_bound_at(c, delta, e)?;
                &epsilon + &kappa
            }
        };
        let value = self.value(p);
        Some(value > bound || value < &Dyadic::integer(0) - &bound)
    }

    /// The largest magnitude of the leaves of each axis at `p`, computed in
    /// `f64` as the filter computes it.
    fn delta(&self, p: &[[f64; 3]]) -> [f64; 3] {
        let mut delta = [0.0f64; 3];
        for node in &self.graph.nodes {
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
        delta
    }

    /// Whether, at each of the inputs `p`, the terms of kappa of the bound
    /// by parts with the constants `c` and the uncertainty `e` exceed
    /// `2^KAPPA_OVER_EPSILON` times its terms of epsilon, and the sizes
    /// with `e` and with `e / 2` are in the range.
    fn margins_hold(&self, c: &PartsConstants, p: [&Case; 2], e: [f64; 3]) -> bool {
        let half = e.map(|x| x / 2.0);
        p.iter().all(|q| {
            let delta = self.delta(q);
            let Some((epsilon, kappa)) = parts_bound_at(c, delta, e) else {
                return false;
            };
            parts_bound_at(c, delta, half).is_some()
                && kappa > &epsilon * &Dyadic::pow2(KAPPA_OVER_EPSILON)
        })
    }

    /// The sign of the form at `p`: -1, 0 or 1.
    fn sign(&self, p: &[[f64; 3]]) -> i8 {
        match self.value(p).cmp(&Dyadic::integer(0)) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }
    }
}

/// The inputs obtained by moving one coordinate of `start`.
struct Line<'a> {
    form: &'a Form<'a>,
    start: Case,
    point: usize,
    axis: usize,
    e: [f64; 3],
}

impl Line<'_> {
    /// The points with the moved coordinate set to the `f64` of key `key`.
    fn at(&self, key: i128) -> Case {
        let mut p = self.start.clone();
        p[self.point][self.axis] = from_key(key);
        p
    }

    /// The last input at or below the bound and the first above it, moving
    /// up or down from the zero of the form on this line.
    fn edge_pair(&self, scale: i32, up: bool) -> Option<(Case, Case)> {
        let (below, above) = self.zero(scale)?;
        let (origin, step) = if up { (above, 1) } else { (below, -1) };
        let side = |n: i128| -> Option<bool> {
            let key = origin + step * n;
            if !(-FINITE_KEYS..=FINITE_KEYS).contains(&key) {
                return None;
            }
            self.form.above_bound(&self.at(key), self.e)
        };
        if side(0)? {
            return None;
        }
        // Double the step until it is above the bound, then bisect.
        let mut high: i128 = 1;
        while !side(high)? {
            high = high.checked_mul(2).filter(|&h| h <= FINITE_KEYS)?;
        }
        let mut low = high / 2;
        while high - low > 1 {
            let middle = low + (high - low) / 2;
            if side(middle)? {
                high = middle;
            } else {
                low = middle;
            }
        }
        Some((self.at(origin + step * low), self.at(origin + step * high)))
    }

    /// Adjacent keys of the moved coordinate between which the form
    /// changes sign, or the same key twice where it is zero, searching
    /// among the grid values of the moved coordinate if needed.
    fn zero(&self, scale: i32) -> Option<(i128, i128)> {
        let sign = |key: i128| self.form.sign(&self.at(key));
        let start = key(self.start[self.point][self.axis]);
        let first = sign(start);
        if first == 0 {
            return Some((start, start));
        }
        // The nearest grid value with the other sign, or zero.
        let other = (-4 * GRID..=4 * GRID)
            .map(|n| key(n as f64 * pow2(scale)))
            .filter(|&k| sign(k) != first)
            .min_by_key(|&k| (k - start).abs())?;
        let (mut same, mut changed) = (start, other);
        while (changed - same).abs() > 1 {
            let middle = same + (changed - same) / 2;
            match sign(middle) {
                0 => return Some((middle, middle)),
                s if s == first => same = middle,
                _ => changed = middle,
            }
        }
        if sign(changed) == 0 {
            Some((changed, changed))
        } else {
            Some((same.min(changed), same.max(changed)))
        }
    }
}

/// The exact per-axis bound with the constants `c`, for the largest
/// magnitudes `delta` of the leaves of each axis and the uncertainty `e`,
/// with `d_k = delta_k + e[k]` computed in `f64`; `None` when some `d_k` of
/// an axis of positive degree is outside `D_MIN..=D_MAX`.
fn axis_bound(c: &AxisConstants, delta: [f64; 3], e: [f64; 3]) -> Option<Dyadic> {
    let mut d = [Dyadic::integer(0), Dyadic::integer(0), Dyadic::integer(0)];
    for k in (0..3).filter(|&k| c.degrees[k] >= 1) {
        let dk = delta[k] + e[k];
        if !(c.d_min..=c.d_max).contains(&dk) {
            return None;
        }
        d[k] = Dyadic::from_f64(dk);
    }
    // `d^G`, without one factor `d_k` when `skip` is `Some(k)`.
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

/// The exact bound by parts with the emitted constants `c`, for the largest
/// magnitudes `delta` of the leaves of each axis and the uncertainty `e`,
/// with `d_k = delta_k + e[k]` computed in `f64`, in its two parts: the sum
/// of its terms of epsilon, `value * d^degrees`, and the sum of its terms
/// of kappa, `value * e[axis] * d^rest`; the bound is the sum of the two.
/// `None` when some `d_k` of an axis of the range is outside
/// `d_min..=d_max`.
fn parts_bound_at(c: &PartsConstants, delta: [f64; 3], e: [f64; 3]) -> Option<(Dyadic, Dyadic)> {
    let mut d = [Dyadic::integer(0), Dyadic::integer(0), Dyadic::integer(0)];
    for k in (0..3).filter(|&k| c.range_axes[k]) {
        let dk = delta[k] + e[k];
        if !(c.d_min..=c.d_max).contains(&dk) {
            return None;
        }
        d[k] = Dyadic::from_f64(dk);
    }
    let power = |start: Dyadic, g: [u32; 3]| {
        let mut x = start;
        for (k, dk) in d.iter().enumerate() {
            for _ in 0..g[k] {
                x = &x * dk;
            }
        }
        x
    };
    let mut epsilon = Dyadic::integer(0);
    for t in &c.epsilon {
        epsilon = &epsilon + &power(Dyadic::from_f64(t.value), t.degrees);
    }
    let mut kappa = Dyadic::integer(0);
    for t in &c.kappa {
        let start = &Dyadic::from_f64(t.value) * &Dyadic::from_f64(e[t.axis]);
        kappa = &kappa + &power(start, t.rest);
    }
    Some((epsilon, kappa))
}
