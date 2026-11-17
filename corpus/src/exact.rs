//! Exact coordinates and exact predicate signs over [`Dyadic`] numbers.
//!
//! Every function here is exact: determinants are expanded by cofactors and
//! evaluated with no approximation at all.

use crate::PointDef;
use implicit_predicates_oracle::Dyadic;

/// The recorded orient3d sign equals this factor times the exact sign
/// computed by [`orient3d`]: the reference mesher uses the opposite
/// convention, so the factor is -1.
pub const REFERENCE_ORIENT3D_FACTOR: i8 = -1;

/// The recorded insphere sign equals this factor times the exact sign
/// computed by [`insphere`]: the reference mesher uses the opposite
/// convention, so the factor is -1.
pub const REFERENCE_INSPHERE_FACTOR: i8 = -1;

type Vec3 = [Dyadic; 3];

/// Exact coordinates of the point `index`, or `None` for an LPI or TPI point.
///
/// An LNC point is `p + t * (q - p)`, evaluated exactly.
///
/// # Panics
///
/// Panics if `index` or an LNC parent is out of range, or if a coordinate or
/// parameter is not finite. A validated [`Corpus`](crate::Corpus) never
/// triggers these.
pub fn coordinates(points: &[PointDef], index: u32) -> Option<Vec3> {
    match &points[index as usize] {
        PointDef::Explicit(c) => Some(c.map(Dyadic::from_f64)),
        PointDef::Lnc { p, q, t } => {
            let p = coordinates(points, *p)?;
            let q = coordinates(points, *q)?;
            let t = Dyadic::from_f64(*t);
            Some(std::array::from_fn(|i| &p[i] + &(&t * &(&q[i] - &p[i]))))
        }
        PointDef::Lpi { .. } | PointDef::Tpi { .. } => None,
    }
}

fn sub(a: &Vec3, b: &Vec3) -> Vec3 {
    std::array::from_fn(|i| &a[i] - &b[i])
}

fn dot(a: &Vec3) -> Dyadic {
    &(&(&a[0] * &a[0]) + &(&a[1] * &a[1])) + &(&a[2] * &a[2])
}

/// The dot product of `a` and `b`.
fn inner(a: &Vec3, b: &Vec3) -> Dyadic {
    &(&(&a[0] * &b[0]) + &(&a[1] * &b[1])) + &(&a[2] * &b[2])
}

/// The cross product of `a` and `b`.
fn cross(a: &Vec3, b: &Vec3) -> Vec3 {
    let at = |j: usize, k: usize| &(&a[j] * &b[k]) - &(&a[k] * &b[j]);
    [at(1, 2), at(2, 0), at(0, 1)]
}

/// Determinant of the 3x3 matrix with the given rows, by cofactors along the
/// first row.
fn det3(r: [&Vec3; 3]) -> Dyadic {
    let minor = |j: usize, k: usize| &(&r[1][j] * &r[2][k]) - &(&r[1][k] * &r[2][j]);
    let t0 = &r[0][0] * &minor(1, 2);
    let t1 = &r[0][1] * &minor(0, 2);
    let t2 = &r[0][2] * &minor(0, 1);
    &(&t0 - &t1) + &t2
}

/// Sign of `det[a - d; b - d; c - d]`: 1, 0 or -1.
pub fn orient3d(a: &Vec3, b: &Vec3, c: &Vec3, d: &Vec3) -> i8 {
    det3([&sub(a, d), &sub(b, d), &sub(c, d)]).sign()
}

/// Sign of the 4x4 determinant whose rows are `(p - e, |p - e|^2)` for `p` in
/// `a, b, c, d`: 1, 0 or -1.
pub fn insphere(a: &Vec3, b: &Vec3, c: &Vec3, d: &Vec3, e: &Vec3) -> i8 {
    let rows = [sub(a, e), sub(b, e), sub(c, e), sub(d, e)];
    let lifted = [dot(&rows[0]), dot(&rows[1]), dot(&rows[2]), dot(&rows[3])];
    // Cofactor expansion along the last column: the entry of row i has sign
    // (-1)^(i + 3) and its minor is the 3x3 determinant of the other rows.
    let minor = |skip: usize| {
        let mut others = (0..4).filter(|&i| i != skip).map(|i| &rows[i]);
        det3([
            others.next().unwrap(),
            others.next().unwrap(),
            others.next().unwrap(),
        ])
    };
    let mut det = Dyadic::from_f64(0.0);
    for (i, l) in lifted.iter().enumerate() {
        let term = l * &minor(i);
        det = if (i + 3) % 2 == 0 {
            &det + &term
        } else {
            &det - &term
        };
    }
    det.sign()
}

/// Sign of `(p - q) . (r - q)`: 1, 0 or -1. It is positive when the angle at
/// `q` between `p` and `r` is acute, negative when it is obtuse, and zero
/// when it is right or when `p` or `r` is `q`.
pub fn dot_sign_3d(p: &Vec3, q: &Vec3, r: &Vec3) -> i8 {
    inner(&sub(p, q), &sub(r, q)).sign()
}

/// Sign of `[(b - d) x (c - d)] . [(a - d) x (c - d)]`: 1, 0 or -1. It is
/// positive when the dihedral angle at the edge `c, d` between the triangles
/// `a, c, d` and `b, c, d` is acute, negative when it is obtuse, and zero
/// when it is right or when a triangle is degenerate.
pub fn dihedral_sign(a: &Vec3, b: &Vec3, c: &Vec3, d: &Vec3) -> i8 {
    let e = sub(c, d);
    inner(&cross(&sub(b, d), &e), &cross(&sub(a, d), &e)).sign()
}
