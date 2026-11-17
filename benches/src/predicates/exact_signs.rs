//! The exact sign of a predicate on points of any type, from the definitions
//! of the points and of the predicates alone: what the loads of this crate
//! are selected with and checked against, so that no code under test
//! chooses an input.

use implicit_predicates::{AnyPoint, Point};
use implicit_predicates_corpus::exact;
use implicit_predicates_oracle::Dyadic;

/// The exact coordinates of a point, from its definition: an `Lnc` with
/// parameters `p`, `q` and `t` is `q + t (p - q)`, and a `Bpt` with
/// parameters `p`, `q`, `r`, `u` and `v` is `r + u (p - r) + v (q - r)`.
///
/// # Panics
///
/// Panics on a point of a type this crate does not define.
#[must_use]
pub fn exact_point(point: &AnyPoint) -> [Dyadic; 3] {
    match point {
        AnyPoint::Explicit(e) => e.approx().map(Dyadic::from_f64),
        AnyPoint::Lnc(l) => {
            let t = Dyadic::from_f64(l.t());
            std::array::from_fn(|i| {
                let (p, q) = (Dyadic::from_f64(l.p()[i]), Dyadic::from_f64(l.q()[i]));
                &q + &(&t * &(&p - &q))
            })
        }
        AnyPoint::Bpt(b) => {
            let (u, v) = (Dyadic::from_f64(b.u()), Dyadic::from_f64(b.v()));
            std::array::from_fn(|i| {
                let (p, q, r) = (
                    Dyadic::from_f64(b.p()[i]),
                    Dyadic::from_f64(b.q()[i]),
                    Dyadic::from_f64(b.r()[i]),
                );
                &(&r + &(&u * &(&p - &r))) + &(&v * &(&q - &r))
            })
        }
        other => panic!("no definition of the exact coordinates of {other:?}"),
    }
}

/// The exact sign of `dihedral_sign` on the points, 1, 0 or -1.
#[must_use]
pub fn exact_dihedral_sign(p: &[AnyPoint; 4]) -> i8 {
    let [a, b, c, d] = p.each_ref().map(exact_point);
    exact::dihedral_sign(&a, &b, &c, &d)
}

/// The exact sign of `dot_sign_3d` on the points, 1, 0 or -1.
#[must_use]
pub fn exact_dot_sign_3d(p: &[AnyPoint; 3]) -> i8 {
    let [a, b, c] = p.each_ref().map(exact_point);
    exact::dot_sign_3d(&a, &b, &c)
}

/// The exact sign of `orient3d` on the points, 1, 0 or -1.
#[must_use]
pub fn exact_orient3d_sign(p: &[AnyPoint; 4]) -> i8 {
    let [a, b, c, d] = p.each_ref().map(exact_point);
    exact::orient3d(&a, &b, &c, &d)
}

/// The exact sign of `insphere` on the points, 1, 0 or -1.
#[must_use]
pub fn exact_insphere_sign(p: &[AnyPoint; 5]) -> i8 {
    let [a, b, c, d, e] = p.each_ref().map(exact_point);
    exact::insphere(&a, &b, &c, &d, &e)
}
