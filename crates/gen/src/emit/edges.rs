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
//! of the form exceeds the bound `EPSILON * delta^k` of the filter for
//! explicit points, computed with the emitted constant and with `delta` the
//! largest difference of coordinates as the filter computes it in `f64`.
//! The last input on each side of the bound is kept. The search doubles the
//! step and then bisects, so it takes a few dozen exact evaluations.
//!
//! The scales of the grids range from just above `D_MIN` to just below
//! `D_MAX`. Every choice comes from a fixed pseudo-random sequence, so the
//! cases are the same on every run.

use crate::bound::FilterConstants;
use crate::differences::{DiffGraph, DiffNode};
use crate::dyadic::Dyadic;

/// The coordinates of the points of one case.
pub(crate) type Case = Vec<[f64; 3]>;

/// The cases built at each scale, and the attempts allowed for them.
const PER_SCALE: usize = 6;
const ATTEMPTS_PER_SCALE: usize = 64;

/// The grid integers are in `-GRID..=GRID`; with the affine combination,
/// every coordinate is below `2^8` in magnitude, and so is every
/// difference of two of them, before scaling.
const GRID: i64 = 16;

/// Pairs of inputs on either side of the bound of the filter of `graph`,
/// with `points` points and the emitted `constants`: the first of each
/// pair at or below it, the second above it, one representable value apart
/// in one coordinate.
pub(crate) fn bound_edge_cases(
    graph: &DiffGraph,
    points: usize,
    constants: &FilterConstants,
) -> Vec<(Case, Case)> {
    let form = Form { graph, constants };
    let (low, high) = (exponent(constants.d_min), exponent(constants.d_max));
    // From just above D_MIN to just below D_MAX: the differences are
    // between 2^scale and 2^(scale + 8).
    let scales = [low + 1, low + 40, -100, -30, 0, 30, 100, high - 12];
    let mut random = SplitMix64(0x2545_f491_4f6c_dd1d);
    let mut cases = Vec::new();
    for scale in scales.into_iter().filter(|&s| low < s && s + 8 <= high) {
        let mut found = 0;
        for _ in 0..ATTEMPTS_PER_SCALE {
            if found == PER_SCALE {
                break;
            }
            let start = grid_points(&mut random, points, scale);
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
                form: &form,
                start,
                point,
                axis,
            };
            if let Some(pair) = line.edge_pair(scale, up) {
                cases.push(pair);
                found += 1;
            }
        }
    }
    cases
}

/// A difference form with the constants of its filter.
struct Form<'a> {
    graph: &'a DiffGraph,
    constants: &'a FilterConstants,
}

impl Form<'_> {
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

    /// Whether the exact magnitude of the form at `p` exceeds
    /// `EPSILON * delta^k`, or `None` when `delta`, the largest difference
    /// computed in `f64`, is outside `D_MIN..=D_MAX`.
    fn above_bound(&self, p: &[[f64; 3]]) -> Option<bool> {
        let mut delta = 0.0f64;
        for node in &self.graph.nodes {
            if let DiffNode::Leaf {
                axis,
                minuend,
                subtrahend,
            } = *node
            {
                let axis = usize::from(axis);
                delta = delta.max((p[minuend][axis] - p[subtrahend][axis]).abs());
            }
        }
        if !(self.constants.d_min..=self.constants.d_max).contains(&delta) {
            return None;
        }
        let delta = Dyadic::from_f64(delta);
        let mut bound = Dyadic::from_f64(self.constants.epsilon);
        for _ in 0..self.constants.degree {
            bound = &bound * &delta;
        }
        let value = self.value(p);
        Some(value > bound || value < &Dyadic::integer(0) - &bound)
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
            self.form.above_bound(&self.at(key))
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

/// `count` points on the grid of integers in `-GRID..=GRID` times
/// `2^scale`, the last an integer affine combination of the first three
/// when there are at least four.
fn grid_points(random: &mut SplitMix64, count: usize, scale: i32) -> Case {
    let mut integer = || random.below(2 * GRID as u64 + 1) as i64 - GRID;
    let mut p: Vec<[i64; 3]> = (0..count)
        .map(|_| [integer(), integer(), integer()])
        .collect();
    if count >= 4 {
        let (s, t) = (random.below(5) as i64 - 2, random.below(5) as i64 - 2);
        let (a, b, c) = (p[0], p[1], p[2]);
        p[count - 1] = std::array::from_fn(|i| a[i] + s * (b[i] - a[i]) + t * (c[i] - a[i]));
    }
    // The integers are below 2^8, so the products are exact.
    let unit = pow2(scale);
    p.iter()
        .map(|point| point.map(|n| n as f64 * unit))
        .collect()
}

/// The largest key of a finite `f64`: that of `f64::MAX`.
const FINITE_KEYS: i128 = 0x7fef_ffff_ffff_ffff;

/// The position of `x` in the order of the finite `f64`s, with both zeros
/// at 0: consecutive keys are consecutive values.
fn key(x: f64) -> i128 {
    let magnitude = i128::from(x.to_bits() & !(1 << 63));
    if x.is_sign_negative() {
        -magnitude
    } else {
        magnitude
    }
}

/// The `f64` of key `key`, which must be at most [`FINITE_KEYS`] in
/// magnitude.
fn from_key(key: i128) -> f64 {
    let magnitude = u64::try_from(key.unsigned_abs()).expect("a finite key");
    let sign = if key < 0 { 1 << 63 } else { 0 };
    f64::from_bits(sign | magnitude)
}

/// The exponent of a power of two.
fn exponent(x: f64) -> i32 {
    i32::try_from(x.to_bits() >> 52).expect("11 bits") - 1023
}

/// `2^exponent`, for a normal power of two.
fn pow2(exponent: i32) -> f64 {
    f64::from_bits(u64::try_from(exponent + 1023).expect("a normal power of two") << 52)
}

/// The SplitMix64 sequence of pseudo-random numbers.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A number in `0..n`, nearly uniform for small `n`.
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}
