//! Approximations of exact points within error boxes, for the tests of the
//! uncertainty term of the filter: the exact partial derivatives of the
//! determinant, the corner of the boxes that moves it furthest, `e` as a
//! caller computes it, and the extremal coplanar configuration.

use implicit_predicates_oracle::Dyadic;

use super::Points;

/// Four points with exact coordinates.
pub type ExactPoints = [[Dyadic; 3]; 4];

/// The coordinates of `p`, exactly.
pub fn exact_points(p: &Points) -> ExactPoints {
    p.map(|point| point.map(Dyadic::from_f64))
}

/// `x` rounded up to an `f64` that is at least the exact value `exact`, of
/// which `x` is the rounding to nearest.
pub fn at_least(x: f64, exact: &Dyadic) -> f64 {
    if Dyadic::from_f64(x) < *exact {
        x.next_up()
    } else {
        x
    }
}

/// `|x|`, exactly.
pub fn abs(x: &Dyadic) -> Dyadic {
    if x.sign() < 0 { -x } else { x.clone() }
}

/// `e` as a caller computes it: the sum of the two largest errors, rounded
/// up.
pub fn uncertainty(errors: [f64; 4]) -> f64 {
    let mut errors = errors;
    errors.sort_by(|a, b| b.total_cmp(a));
    at_least(
        errors[0] + errors[1],
        &(&Dyadic::from_f64(errors[0]) + &Dyadic::from_f64(errors[1])),
    )
}

/// The exact partial derivatives of det[a - d; b - d; c - d] with respect
/// to every coordinate of every point, at `x`: the cofactors of the rows
/// `a - d`, `b - d`, `c - d`, and minus their sum for `d`.
pub fn partials_at(x: &ExactPoints) -> ExactPoints {
    let m: [[Dyadic; 3]; 3] = std::array::from_fn(|i| std::array::from_fn(|j| &x[i][j] - &x[3][j]));
    let cofactor = |i: usize, j: usize| {
        let (i1, i2, j1, j2) = ((i + 1) % 3, (i + 2) % 3, (j + 1) % 3, (j + 2) % 3);
        &(&m[i1][j1] * &m[i2][j2]) - &(&m[i1][j2] * &m[i2][j1])
    };
    let rows: [[Dyadic; 3]; 3] = std::array::from_fn(|i| std::array::from_fn(|j| cofactor(i, j)));
    let last = std::array::from_fn(|j| -&(&(&rows[0][j] + &rows[1][j]) + &rows[2][j]));
    [rows[0].clone(), rows[1].clone(), rows[2].clone(), last]
}

/// [`partials_at`] the coordinates of `p`.
pub fn partials(p: &Points) -> ExactPoints {
    partials_at(&exact_points(p))
}

/// The exact determinant det[a - d; b - d; c - d] at `x`.
pub fn determinant_at(x: &ExactPoints) -> Dyadic {
    // Expanded along the first row with the cofactors of `partials_at`.
    let d = partials_at(x);
    (0..3)
        .map(|j| &(&x[0][j] - &x[3][j]) * &d[0][j])
        .fold(Dyadic::from_f64(0.0), |sum, term| &sum + &term)
}

/// [`determinant_at`] the coordinates of `p`.
pub fn determinant(p: &Points) -> Dyadic {
    determinant_at(&exact_points(p))
}

/// The sign, `-1` or `1`, by which [`pushed`] moves a coordinate whose
/// partial derivative is `derivative`, in `direction`.
pub fn push_sign(derivative: &Dyadic, direction: i8) -> i8 {
    match derivative.sign() * direction {
        0 => direction,
        s => s,
    }
}

/// `truth` with every coordinate of point `i` moved by `errors[i]`, up or
/// down by the sign of the partial derivative there times `direction`: the
/// corner of the error boxes that moves the determinant furthest in that
/// direction, to first order. Returns `None` unless every move is exact.
pub fn pushed(truth: &Points, errors: [f64; 4], direction: i8) -> Option<Points> {
    pushed_with(truth, &partials(truth), errors, direction)
}

/// [`pushed`] with the partial derivatives at `truth` already computed.
pub fn pushed_with(
    truth: &Points,
    partials: &ExactPoints,
    errors: [f64; 4],
    direction: i8,
) -> Option<Points> {
    let mut p = *truth;
    for i in 0..4 {
        for axis in 0..3 {
            let offset = f64::from(push_sign(&partials[i][axis], direction)) * errors[i];
            p[i][axis] = truth[i][axis] + offset;
            let moved = &Dyadic::from_f64(truth[i][axis]) + &Dyadic::from_f64(offset);
            if Dyadic::from_f64(p[i][axis]) != moved {
                return None;
            }
        }
    }
    Some(p)
}

/// The extremal coplanar configuration at scale `s`: the differences of `a`,
/// `b` and `c` against `d`, in units of `s`. Its determinant is zero, every
/// partial derivative with respect to `a`, `b` and `c` is `s^2` in
/// magnitude and every one with respect to `d` is `3 s^2`, so pushing every
/// coordinate by `err` moves the determinant by `18 s^2 err = 9 E s^2`.
pub const EXTREMAL: [[f64; 3]; 3] = [[-1.0, -1.0, 0.0], [0.0, 1.0, -1.0], [1.0, 0.0, 1.0]];

/// The permutations of three items.
pub const PERMUTATIONS: [[usize; 3]; 6] = [
    [0, 1, 2],
    [0, 2, 1],
    [1, 0, 2],
    [1, 2, 0],
    [2, 0, 1],
    [2, 1, 0],
];

/// The extremal configuration at scale `s`, translated by `t`, with row `i`
/// given to point `roles[i]`, column `k` to axis `axes[k]`, and the axes in
/// the bits of `negated` negated.
pub fn extremal(s: f64, t: [f64; 3], roles: [usize; 3], axes: [usize; 3], negated: u8) -> Points {
    let mut p = [t; 4];
    for (i, row) in EXTREMAL.iter().enumerate() {
        for (k, &v) in row.iter().enumerate() {
            let axis = axes[k];
            let sign = if negated >> axis & 1 == 1 { -1.0 } else { 1.0 };
            p[roles[i]][axis] = t[axis] + sign * v * s;
        }
    }
    p
}

/// `D` as the filter computes it for `p` and `e`, exactly: the largest
/// magnitude of a difference against `d`, plus `e`.
pub fn scale_of(p: &ExactPoints, e: f64) -> Dyadic {
    let delta = (0..3)
        .flat_map(|i| (0..3).map(move |axis| (i, axis)))
        .map(|(i, axis)| abs(&(&p[i][axis] - &p[3][axis])))
        .max()
        .unwrap();
    &delta + &Dyadic::from_f64(e)
}

/// The true points to try for an approximation `p` within `errors`: `p`
/// itself and the two corners of the boxes that move the determinant
/// furthest each way, with exact coordinates, since a subnormal move of a
/// normal coordinate is not an `f64`.
pub fn exact_corners(p: &Points, errors: [f64; 4]) -> Vec<ExactPoints> {
    let center = exact_points(p);
    let d = partials_at(&center);
    let mut truths = vec![center.clone()];
    for direction in [1, -1] {
        let mut corner = center.clone();
        for i in 0..4 {
            for axis in 0..3 {
                let offset = &Dyadic::from_f64(errors[i])
                    * &Dyadic::from_f64(f64::from(push_sign(&d[i][axis], direction)));
                corner[i][axis] = &center[i][axis] + &offset;
            }
        }
        truths.push(corner);
    }
    truths
}
