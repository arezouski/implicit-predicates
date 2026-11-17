//! A point on a triangle, `Bpt { p, q, r, u, v }`, by hand: its exact
//! coordinates, written from the definition.

use implicit_predicates_oracle::Dyadic;

/// The exact coordinates of the point `r + u (p - r) + v (q - r)`, which is
/// `u p + v q + (1 - u - v) r`, computed on `Dyadic` from that formula alone.
pub fn exact_bpt_by_hand(p: &[f64; 3], q: &[f64; 3], r: &[f64; 3], u: f64, v: f64) -> [Dyadic; 3] {
    let (u, v) = (Dyadic::from_f64(u), Dyadic::from_f64(v));
    std::array::from_fn(|k| {
        let (p, q, r) = (
            Dyadic::from_f64(p[k]),
            Dyadic::from_f64(q[k]),
            Dyadic::from_f64(r[k]),
        );
        let up = &u * &(&p - &r);
        let vq = &v * &(&q - &r);
        &(&r + &up) + &vq
    })
}
