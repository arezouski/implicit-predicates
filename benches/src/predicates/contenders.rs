//! The contenders, each returning the sign in the convention of the module
//! documentation of [`super`].

use std::cell::RefCell;

use implicit_predicates::stages::insphere;
use implicit_predicates::{AnyPoint, Point, Scratch};

use super::{ImplicitInput, InSphereImplicitInput, InSphereInput, Orient3dInput};

/// The sign, 1, 0 or -1, of a value.
fn sign(x: f64) -> i8 {
    if x > 0.0 {
        1
    } else if x < 0.0 {
        -1
    } else {
        0
    }
}

/// Multiplier that maps the sign `robust` returns for `orient3d` to the
/// convention above.
const ROBUST_ORIENT3D_FACTOR: i8 = 1;

/// Multiplier for `robust::insphere`.
const ROBUST_INSPHERE_FACTOR: i8 = 1;

/// Multiplier for `geometry_predicates::orient3d`.
const GEOMETRY_ORIENT3D_FACTOR: i8 = 1;

/// Multiplier for `geometry_predicates::insphere`.
const GEOMETRY_INSPHERE_FACTOR: i8 = 1;

fn coord(p: &[f64; 3]) -> robust::Coord3D<f64> {
    robust::Coord3D {
        x: p[0],
        y: p[1],
        z: p[2],
    }
}

/// `orient3d` of the `robust` crate.
#[must_use]
pub fn robust_orient3d_sign(p: &Orient3dInput) -> i8 {
    let [a, b, c, d] = p;
    ROBUST_ORIENT3D_FACTOR * sign(robust::orient3d(coord(a), coord(b), coord(c), coord(d)))
}

/// `insphere` of the `robust` crate.
#[must_use]
pub fn robust_insphere_sign(p: &InSphereInput) -> i8 {
    let [a, b, c, d, e] = p;
    ROBUST_INSPHERE_FACTOR
        * sign(robust::insphere(
            coord(a),
            coord(b),
            coord(c),
            coord(d),
            coord(e),
        ))
}

/// `orient3d` of the `geometry-predicates` crate.
#[must_use]
pub fn geometry_orient3d_sign(p: &Orient3dInput) -> i8 {
    let [a, b, c, d] = *p;
    GEOMETRY_ORIENT3D_FACTOR * sign(geometry_predicates::orient3d(a, b, c, d))
}

/// `orient3d` of `implicit-predicates`.
#[must_use]
pub fn implicit_predicates_orient3d_sign(p: &Orient3dInput) -> i8 {
    let [a, b, c, d] = p;
    i8::from(implicit_predicates::orient3d(a, b, c, d))
}

/// `orient3d` of `implicit-predicates` on points of any type, with the
/// arena of the thread for the exact stages of implicit points.
#[must_use]
pub fn implicit_predicates_orient3d_any_sign(p: &ImplicitInput) -> i8 {
    let [a, b, c, d] = p;
    i8::from(implicit_predicates::orient3d(a, b, c, d))
}

thread_local! {
    /// The arena of the thread for [`implicit_predicates_insphere_any_sign`],
    /// kept between calls like the arena inside `orient3d`.
    static INSPHERE_ARENA: RefCell<Scratch<'static>> = RefCell::new(Scratch::growable(1 << 14));
}

/// The whole chain of `insphere` of `implicit-predicates` on points of any
/// type, with the arena of the thread for the exact stages.
#[must_use]
pub fn implicit_predicates_insphere_any_sign(p: &InSphereImplicitInput) -> i8 {
    let [a, b, c, d, e] = p;
    INSPHERE_ARENA.with(|arena| {
        let mut scratch = arena.borrow_mut();
        let empty = scratch.mark();
        let sign = insphere::staged_in(&mut scratch, a, b, c, d, e).expect("a growable arena");
        scratch.reset(empty);
        i8::from(sign)
    })
}

/// The public `insphere` of `implicit-predicates` on points of any type,
/// with the arena of the thread that the function keeps for itself.
#[must_use]
pub fn implicit_predicates_insphere_public_sign(p: &InSphereImplicitInput) -> i8 {
    let [a, b, c, d, e] = p;
    i8::from(implicit_predicates::insphere(a, b, c, d, e))
}

/// `insphere` of the `geometry-predicates` crate.
#[must_use]
pub fn geometry_insphere_sign(p: &InSphereInput) -> i8 {
    let [a, b, c, d, e] = *p;
    GEOMETRY_INSPHERE_FACTOR * sign(geometry_predicates::insphere(a, b, c, d, e))
}

fn sub(a: &[f64; 3], b: &[f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}

/// 3x3 determinant by cofactors along the first row, in plain `f64`.
fn det3(r: [&[f64; 3]; 3]) -> f64 {
    r[0][0] * (r[1][1] * r[2][2] - r[1][2] * r[2][1])
        - r[0][1] * (r[1][0] * r[2][2] - r[1][2] * r[2][0])
        + r[0][2] * (r[1][0] * r[2][1] - r[1][1] * r[2][0])
}

/// `det[a - d; b - d; c - d]` in plain `f64`, with no error bound: its sign is
/// wrong on nearly degenerate inputs.
#[must_use]
pub fn naive_orient3d(p: &Orient3dInput) -> f64 {
    let [a, b, c, d] = p;
    det3([&sub(a, d), &sub(b, d), &sub(c, d)])
}

/// The 4x4 determinant with rows `(p - e, |p - e|^2)` in plain `f64`, with no
/// error bound: its sign is wrong on nearly degenerate inputs.
#[must_use]
pub fn naive_insphere(p: &InSphereInput) -> f64 {
    let [a, b, c, d, e] = p;
    let rows = [sub(a, e), sub(b, e), sub(c, e), sub(d, e)];
    let lifted = rows.map(|r| r[0] * r[0] + r[1] * r[1] + r[2] * r[2]);
    // Expansion along the last column; the entry of row i has sign
    // (-1)^(i + 3).
    -lifted[0] * det3([&rows[1], &rows[2], &rows[3]])
        + lifted[1] * det3([&rows[0], &rows[2], &rows[3]])
        - lifted[2] * det3([&rows[0], &rows[1], &rows[3]])
        + lifted[3] * det3([&rows[0], &rows[1], &rows[2]])
}

/// Sign of [`naive_orient3d`].
#[must_use]
pub fn naive_orient3d_sign(p: &Orient3dInput) -> i8 {
    sign(naive_orient3d(p))
}

/// Sign of [`naive_insphere`].
#[must_use]
pub fn naive_insphere_sign(p: &InSphereInput) -> i8 {
    sign(naive_insphere(p))
}

/// `dot_sign_3d` of `implicit-predicates`: the whole public chain.
#[must_use]
pub fn implicit_predicates_dot_sign_3d_sign(p: &[AnyPoint; 3]) -> i8 {
    let [a, b, c] = p;
    i8::from(implicit_predicates::dot_sign_3d(a, b, c))
}

/// `dihedral_sign` of `implicit-predicates`: the whole public chain.
#[must_use]
pub fn implicit_predicates_dihedral_sign_sign(p: &[AnyPoint; 4]) -> i8 {
    let [a, b, c, d] = p;
    i8::from(implicit_predicates::dihedral_sign(a, b, c, d))
}

fn cross(a: &[f64; 3], b: &[f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The sign of `(p - q) . (r - q)` in plain `f64` on the approximations of
/// the points, with no error bound: it is wrong on nearly degenerate inputs,
/// and meant for explicit points.
#[must_use]
pub fn naive_dot_sign_3d_sign(p: &[AnyPoint; 3]) -> i8 {
    let [p, q, r] = p.each_ref().map(|x| x.approx());
    sign(dot(&sub(p, q), &sub(r, q)))
}

/// The sign of `[(b - d) x (c - d)] . [(a - d) x (c - d)]` in plain `f64` on
/// the approximations of the points, with no error bound: it is wrong on
/// nearly degenerate inputs, and meant for explicit points.
#[must_use]
pub fn naive_dihedral_sign_sign(p: &[AnyPoint; 4]) -> i8 {
    let [a, b, c, d] = p.each_ref().map(|x| x.approx());
    let e = sub(c, d);
    sign(dot(&cross(&sub(b, d), &e), &cross(&sub(a, d), &e)))
}
