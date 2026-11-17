//! Loads of the exact stages of `dihedral_sign`, `dot_sign_3d` and
//! `orient3d` on implicit points, `Lnc` and `Bpt`, built on small meshes.
//!
//! Every load holds [`EXACT_LOAD_SIZE`] calls that neither the filter nor,
//! for the predicates that have it, its second test (`filter_axes_points`)
//! decides, so that what a benchmark on them times is the exact stages.
//! Each combination of point types has two families: `degenerate`, whose
//! exact sign is zero by construction, and `near_degenerate`, the same
//! construction with one coordinate of one vertex moved by one to three
//! ulps, whose exact sign is not zero but too small for the filter.
//!
//! The vertices are multiples of 2^-20 (`short`) or 2^-40 (`full`) below
//! 2^9 in magnitude ([`PRECISIONS`]), so every construction below is exact
//! in `f64`, and the parameters of the points
//! are random multiples of 2^-53 in (0, 1), so that their exact coordinates
//! have the full length an arbitrary point has. For `dihedral_sign` and
//! `dot_sign_3d` the vertices are built in a frame where the planes are
//! axis-aligned, then mapped by the integer matrix [`ROTATION`], whose rows
//! are orthogonal and of the same length: it keeps right angles right, and
//! the planes it gives are in general position. Its entries grow the
//! coordinates by at most five times, below 2^13, still exact.
//!
//! The names of the loads are `<points>/<precision>/<family>`.
//!
//! - `dihedral_sign(a, b, c, d)`: a hinge, two triangles `(e0, e1, w1)` and
//!   `(e0, e1, w2)` on perpendicular planes, sharing the edge `e0 e1`; `c`
//!   and `d` are on the shared edge, `b` on the first triangle and `a` on
//!   the second, so the dihedral angle at `c d` is right. The nearly
//!   degenerate family moves `w2`.
//! - `dot_sign_3d(p, q, r)`: a prism, a triangle `T` and its translate `T +
//!   X` by a vector `X` normal to the plane of `T`, and a triangle `T'` of
//!   the plane of `T` sharing an edge with it; `q` is a point of `T`, `p`
//!   the point of `T + X` at the same parameters, so `p - q = X`, and `r` a
//!   point of `T'`, so the angle at `q` is right. The nearly degenerate
//!   family moves the vertex of `T'` not on `T`.
//! - `orient3d(a, b, c, d)`: a planar patch of six vertices, three random
//!   and three integer affine combinations of them, and four of its
//!   triangles; each point is on one of them, so the four are coplanar. The
//!   nearly degenerate family puts `d` on a seventh triangle whose last
//!   vertex is a moved combination.

use implicit_predicates::stages::{dihedral_sign, dot_sign_3d, orient3d};
use implicit_predicates::{
    AnyPoint, BigCoords, Bpt, ExactCoords, Explicit, Lnc, Point, Scratch, ScratchExhausted, Sign,
};

use super::{combine, int_in, nudge, small_ulps};
use crate::Rng;

/// Number of calls of every load of this module.
pub const EXACT_LOAD_SIZE: usize = 1024;

/// Seed of the loads: the load of index `i` in the list of its predicate
/// uses `EXACT_LOAD_SEED + i` for `dihedral_sign`, `+ 32 + i` for
/// `dot_sign_3d` and `+ 64 + i` for `orient3d`.
pub const EXACT_LOAD_SEED: u64 = 0x6578_6c64;

/// The integer matrix the vertices of the hinges and prisms are mapped by:
/// its rows are orthogonal and of squared length 9.
pub const ROTATION: [[f64; 3]; 3] = [[1.0, 2.0, 2.0], [2.0, 1.0, -2.0], [2.0, -2.0, 1.0]];

/// A load: calls of one predicate with one combination of point types, of
/// one family.
#[derive(Clone, Debug)]
pub struct ExactLoad<const N: usize> {
    /// The name, `<points>/<family>`, as `4_lnc/near_degenerate`.
    pub name: String,
    /// Whether the exact sign of every call is zero.
    pub degenerate: bool,
    /// The calls.
    pub cases: Vec<[AnyPoint; N]>,
}

/// The stages of a predicate of `N` points that the loads of this module
/// are measured with.
pub struct ExactStages<const N: usize> {
    /// The name of the predicate.
    pub name: &'static str,
    /// The tests of the filter that run before the exact stages:
    /// `filter_points`, then `filter_axes_points` when the predicate has it.
    pub filters: fn(&[AnyPoint; N]) -> Option<Sign>,
    /// The stage on expansions over arena spans,
    /// `expansions_translated_in`.
    pub expansions_in: ExpansionsStage<N>,
    /// The stage on big integers, `big_integers_in`.
    pub big_integers_in: BigIntegersStage<N>,
    /// The exact chain on points of any type, `exact_stages_in`.
    pub exact_stages_in: fn(&mut Scratch<'_>, &[AnyPoint; N]) -> Result<Sign, ScratchExhausted>,
    /// The largest number of components of a coordinate the stage on
    /// expansions accepts, `MAX_COORD_COMPONENTS`.
    pub max_coord_components: usize,
}

/// A stage on expansions over arena spans of a predicate of `N` points.
pub type ExpansionsStage<const N: usize> =
    fn(&mut Scratch<'_>, [&ExactCoords; N]) -> Result<Option<Sign>, ScratchExhausted>;

/// A stage on big integers of a predicate of `N` points.
pub type BigIntegersStage<const N: usize> =
    fn(&mut Scratch<'_>, [&BigCoords; N]) -> Result<Sign, ScratchExhausted>;

/// The stages of `dihedral_sign`.
pub const DIHEDRAL_SIGN: ExactStages<4> = ExactStages {
    name: "dihedral_sign",
    filters: |[a, b, c, d]| {
        dihedral_sign::filter_points(a, b, c, d)
            .or_else(|| dihedral_sign::filter_axes_points(a, b, c, d))
    },
    expansions_in: dihedral_sign::expansions_translated_in,
    big_integers_in: dihedral_sign::big_integers_in,
    exact_stages_in: |s, [a, b, c, d]| dihedral_sign::exact_stages_in(s, a, b, c, d),
    max_coord_components: dihedral_sign::MAX_COORD_COMPONENTS,
};

/// The stages of `dot_sign_3d`.
pub const DOT_SIGN_3D: ExactStages<3> = ExactStages {
    name: "dot_sign_3d",
    filters: |[p, q, r]| {
        dot_sign_3d::filter_points(p, q, r).or_else(|| dot_sign_3d::filter_axes_points(p, q, r))
    },
    expansions_in: dot_sign_3d::expansions_translated_in,
    big_integers_in: dot_sign_3d::big_integers_in,
    exact_stages_in: |s, [p, q, r]| dot_sign_3d::exact_stages_in(s, p, q, r),
    max_coord_components: dot_sign_3d::MAX_COORD_COMPONENTS,
};

/// The stages of `orient3d`, which has no second test of the filter.
pub const ORIENT3D: ExactStages<4> = ExactStages {
    name: "orient3d",
    filters: |[a, b, c, d]| orient3d::filter_points(a, b, c, d),
    expansions_in: orient3d::expansions_translated_in,
    big_integers_in: orient3d::big_integers_in,
    exact_stages_in: |s, [a, b, c, d]| orient3d::exact_stages_in(s, a, b, c, d),
    max_coord_components: orient3d::MAX_COORD_COMPONENTS,
};

impl<const N: usize> ExactStages<N> {
    /// The exact coordinates of the points of a call, written to `scratch`,
    /// or `None` if a point cannot give them.
    ///
    /// # Panics
    ///
    /// Panics if the arena is exhausted (use a growable one).
    pub fn exact_coords(
        &self,
        scratch: &mut Scratch<'_>,
        case: &[AnyPoint; N],
    ) -> Option<[ExactCoords; N]> {
        let mut out = [None; N];
        for (slot, point) in out.iter_mut().zip(case) {
            match point.exact(scratch) {
                Ok(q) => *slot = Some(q),
                Err(implicit_predicates::ExactError::Exhausted(_)) => {
                    panic!("a growable arena")
                }
                Err(_) => return None,
            }
        }
        Some(out.map(|q| q.expect("every coordinate")))
    }

    /// The stage on expansions on the exact coordinates `coords`, written
    /// to `scratch` beforehand.
    ///
    /// # Panics
    ///
    /// Panics if the arena is exhausted (use a growable one).
    pub fn expansions_on(
        &self,
        scratch: &mut Scratch<'_>,
        coords: &[ExactCoords; N],
    ) -> Option<Sign> {
        (self.expansions_in)(scratch, coords.each_ref()).expect("a growable arena")
    }

    /// The stage on expansions on the points of one call, as the chain runs
    /// it: the exact coordinates of the points are written to `scratch`,
    /// the stage evaluates the predicate, and the arena is left as it was.
    /// `None` if a point cannot give its coordinates or the stage declines.
    ///
    /// # Panics
    ///
    /// Panics if the arena is exhausted (use a growable one).
    pub fn expansions(&self, scratch: &mut Scratch<'_>, case: &[AnyPoint; N]) -> Option<Sign> {
        let mark = scratch.mark();
        let sign = self
            .exact_coords(scratch, case)
            .and_then(|coords| self.expansions_on(scratch, &coords));
        scratch.reset(mark);
        sign
    }

    /// The big-integer coordinates of the points of a call, written to
    /// `scratch`.
    ///
    /// # Panics
    ///
    /// Panics if the arena is exhausted (use a growable one) or a point has
    /// a non-finite input.
    pub fn big_coords(&self, scratch: &mut Scratch<'_>, case: &[AnyPoint; N]) -> [BigCoords; N] {
        let mut out = [None; N];
        for (slot, point) in out.iter_mut().zip(case) {
            *slot = Some(point.exact_big(scratch).expect("big-integer coordinates"));
        }
        out.map(|q| q.expect("every coordinate"))
    }

    /// The stage on big integers on the big-integer coordinates `coords`,
    /// written to `scratch` beforehand.
    ///
    /// # Panics
    ///
    /// Panics if the arena is exhausted (use a growable one).
    pub fn big_integers_on(&self, scratch: &mut Scratch<'_>, coords: &[BigCoords; N]) -> Sign {
        (self.big_integers_in)(scratch, coords.each_ref()).expect("a growable arena")
    }

    /// The stage on big integers on the points of one call, as the chain
    /// runs it: the big-integer coordinates of the points are written to
    /// `scratch`, the stage evaluates the predicate, and the arena is left
    /// as it was.
    ///
    /// # Panics
    ///
    /// Panics if the arena is exhausted (use a growable one) or a point has
    /// a non-finite input.
    pub fn big_integers(&self, scratch: &mut Scratch<'_>, case: &[AnyPoint; N]) -> Sign {
        let mark = scratch.mark();
        let coords = self.big_coords(scratch, case);
        let sign = self.big_integers_on(scratch, &coords);
        scratch.reset(mark);
        sign
    }

    /// The exact chain on the points of one call.
    ///
    /// # Panics
    ///
    /// Panics if the arena is exhausted (use a growable one) or a point has
    /// a non-finite input.
    pub fn chain(&self, scratch: &mut Scratch<'_>, case: &[AnyPoint; N]) -> Sign {
        (self.exact_stages_in)(scratch, case).expect("a growable arena")
    }
}

/// How the coordinates of the vertices of a load are drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Precision {
    /// Random multiples of `2^step` in `[-2^9, 2^9]`, mapped by
    /// [`ROTATION`] for the hinges and the prisms.
    Grid(i32),
    /// Random `f64` with a full 53-bit significand and a magnitude from
    /// 2^-20 to 2^9, as the coordinates of a mesh that crosses the
    /// coordinate planes. [`ROTATION`] would not be exact on them, so the
    /// hinges and prisms are on planes parallel to the coordinate planes,
    /// and the patch on the plane `z = x`.
    Wide,
}

/// The precisions of the vertices of the loads, with their names. The
/// `short` vertices have at most 30 significant bits, as a mesh of `f32`
/// coordinates scaled up; the `full` ones up to 53 after [`ROTATION`] or
/// the affine combinations of the patch, the most that keeps the
/// constructions exact; the `wide` ones are [`Precision::Wide`].
pub const PRECISIONS: [(&str, Precision); 3] = [
    ("short", Precision::Grid(-20)),
    ("full", Precision::Grid(-40)),
    ("wide", Precision::Wide),
];

/// A random coordinate of a vertex.
fn coord(rng: &mut Rng, precision: Precision) -> f64 {
    match precision {
        Precision::Grid(step) => {
            let half = 1i64 << (9 - step);
            int_in(rng, -half, half) as f64 * 2f64.powi(step)
        }
        Precision::Wide => {
            let significand = ((rng.next_u64() >> 11) | (1 << 52)) as f64;
            let exponent = int_in(rng, -20, 8) as i32;
            let sign = if rng.next_u64() & 1 == 0 { 1.0 } else { -1.0 };
            sign * significand * 2f64.powi(exponent - 52)
        }
    }
}

/// A random nonzero coordinate of a vertex.
fn nonzero_coord(rng: &mut Rng, precision: Precision) -> f64 {
    loop {
        let x = coord(rng, precision);
        if x != 0.0 {
            return x;
        }
    }
}

/// `p` mapped by [`ROTATION`] for a grid precision, `p` itself otherwise.
fn frame(precision: Precision, p: [f64; 3]) -> [f64; 3] {
    match precision {
        Precision::Grid(_) => rotate(p),
        Precision::Wide => p,
    }
}

/// `p`, a vertex on a plane of normal `axis` before [`frame`], moved off it
/// by a few ulps: any coordinate once rotated, `axis` otherwise.
fn off_plane(rng: &mut Rng, precision: Precision, axis: usize, mut p: [f64; 3]) -> [f64; 3] {
    let axis = match precision {
        Precision::Grid(_) => int_in(rng, 0, 2) as usize,
        Precision::Wide => axis,
    };
    p[axis] = nudge(p[axis], small_ulps(rng));
    p
}

/// A random nonzero multiple of 2^-53 in `(0, 1)`.
fn param(rng: &mut Rng) -> f64 {
    loop {
        let t = (rng.next_u64() >> 11) as f64 * 2f64.powi(-53);
        if t > 0.0 {
            return t;
        }
    }
}

/// `p` mapped by [`ROTATION`]. Exact for the vertices of this module.
fn rotate(p: [f64; 3]) -> [f64; 3] {
    ROTATION.map(|row| row[0] * p[0] + row[1] * p[1] + row[2] * p[2])
}

/// An explicit point.
fn explicit(p: [f64; 3]) -> AnyPoint {
    AnyPoint::Explicit(Explicit::new(p).expect("finite coordinates"))
}

/// A point of the segment `p q` at a random parameter: `q + t (p - q)`.
fn lnc(rng: &mut Rng, p: [f64; 3], q: [f64; 3]) -> AnyPoint {
    AnyPoint::Lnc(Lnc::new(p, q, param(rng)).expect("a valid parameter"))
}

/// A point of the segment `p q` at the parameter `t`.
fn lnc_at(p: [f64; 3], q: [f64; 3], t: f64) -> AnyPoint {
    AnyPoint::Lnc(Lnc::new(p, q, t).expect("a valid parameter"))
}

/// Random parameters `(u, v)` of a point inside a triangle: `u + v < 1`.
fn bpt_params(rng: &mut Rng) -> (f64, f64) {
    loop {
        let (u, v) = (param(rng), param(rng));
        // `1 - u` and `1 - v` are exact for multiples of 2^-53 in (0, 1).
        // A computed sum below one, or above it, is so exactly too.
        let (u, v) = if u + v > 1.0 {
            (1.0 - u, 1.0 - v)
        } else {
            (u, v)
        };
        if u + v < 1.0 {
            return (u, v);
        }
    }
}

/// A point of the triangle `p q r` at the parameters `(u, v)`: `r + u (p -
/// r) + v (q - r)`.
fn bpt_at(p: [f64; 3], q: [f64; 3], r: [f64; 3], (u, v): (f64, f64)) -> AnyPoint {
    AnyPoint::Bpt(Bpt::new(p, q, r, u, v).expect("valid parameters"))
}

/// A random point inside the triangle `p q r`.
fn bpt(rng: &mut Rng, p: [f64; 3], q: [f64; 3], r: [f64; 3]) -> AnyPoint {
    let uv = bpt_params(rng);
    bpt_at(p, q, r, uv)
}

/// A random point of the edge `p q` of the triangle `p q r`, as a `Bpt`
/// with `u + v = 1` exactly: `u p + (1 - u) q`.
fn bpt_on_edge(rng: &mut Rng, p: [f64; 3], q: [f64; 3], r: [f64; 3]) -> AnyPoint {
    // `1 - u` is exact for `u` in [1/2, 1).
    let u = 0.5 + 0.5 * param(rng);
    let (u, v) = if rng.next_u64() & 1 == 0 {
        (u, 1.0 - u)
    } else {
        (1.0 - u, u)
    };
    bpt_at(p, q, r, (u, v))
}

/// The vertices of a hinge: `e0` and `e1` on the shared edge, `w1` and `w2`
/// the other vertices of its two triangles, on perpendicular planes; with
/// `near`, `w2` moved by a few ulps.
fn hinge(rng: &mut Rng, precision: Precision, near: bool) -> [[f64; 3]; 4] {
    // In the axis frame: the first plane is z = z0, the second y = y0.
    let mut c = || coord(rng, precision);
    let (y0, z0) = (c(), c());
    let e0 = [c(), y0, z0];
    let e1 = [c(), y0, z0];
    let w1 = [c(), c(), z0];
    let w2 = [c(), y0, c()];
    let [e0, e1, w1, mut w2] = [e0, e1, w1, w2].map(|p| frame(precision, p));
    if near {
        w2 = off_plane(rng, precision, 1, w2);
    }
    [e0, e1, w1, w2]
}

/// The vertices of a prism: a triangle `t`, its translate `x` by a vector
/// normal to its plane, and `s`, the third vertex of a triangle `t1 t2 s`
/// of the plane of `t`; with `near`, `s` moved by a few ulps.
fn prism(
    rng: &mut Rng,
    precision: Precision,
    near: bool,
) -> ([[f64; 3]; 3], [[f64; 3]; 3], [f64; 3]) {
    // In the axis frame: the plane of `t` is x = x1, the translation is
    // along x, by `x1` itself for wide coordinates, so that it is exact.
    let x1 = nonzero_coord(rng, precision);
    let shift = match precision {
        Precision::Grid(_) => nonzero_coord(rng, precision),
        Precision::Wide => x1,
    };
    let base: [[f64; 3]; 3] =
        std::array::from_fn(|_| [x1, coord(rng, precision), coord(rng, precision)]);
    let t = base.map(|p| frame(precision, p));
    let x = base.map(|p| frame(precision, [p[0] + shift, p[1], p[2]]));
    let mut s = frame(
        precision,
        [x1, coord(rng, precision), coord(rng, precision)],
    );
    if near {
        s = off_plane(rng, precision, 0, s);
    }
    (t, x, s)
}

/// The triangles of the planar patch of [`patch`], as indices of its
/// vertices.
const PATCH_TRIANGLES: [[usize; 3]; 4] = [[0, 1, 2], [1, 2, 3], [2, 3, 4], [3, 4, 5]];

/// The weights `(s, t)` of the vertices of [`patch`] after the first three,
/// as `v0 + s (v1 - v0) + t (v2 - v0)`: no triangle of [`PATCH_TRIANGLES`],
/// nor the seventh, `v0 v1 v6`, is degenerate.
const PATCH_WEIGHTS: [(f64, f64); 4] = [(1.0, 1.0), (2.0, 0.0), (1.0, 2.0), (2.0, 2.0)];

/// The vertices of a planar patch, all exactly coplanar: for a grid
/// precision, three random, then integer affine combinations of them with
/// the weights of [`PATCH_WEIGHTS`]; for wide coordinates, seven random
/// vertices of the plane `z = x` (on a plane `z = z0` the filter of
/// `orient3d` decides the degenerate calls). With `near`, the seventh is moved off the
/// plane by a few ulps.
fn patch(rng: &mut Rng, precision: Precision, near: bool) -> [[f64; 3]; 7] {
    let mut v = [[0.0; 3]; 7];
    match precision {
        Precision::Grid(_) => {
            for p in &mut v[..3] {
                *p = std::array::from_fn(|_| coord(rng, precision));
            }
            for (i, (s, t)) in PATCH_WEIGHTS.into_iter().enumerate() {
                v[3 + i] = combine(v[0], v[1], v[2], s, t);
            }
        }
        Precision::Wide => {
            for p in &mut v {
                let x = coord(rng, precision);
                *p = [x, coord(rng, precision), x];
            }
        }
    }
    if near {
        v[6] = off_plane(rng, precision, 2, v[6]);
    }
    v
}

/// A random triangle of a patch, as its three vertices.
fn patch_triangle(rng: &mut Rng, v: &[[f64; 3]; 7]) -> [[f64; 3]; 3] {
    let [i, j, k] = PATCH_TRIANGLES[int_in(rng, 0, 3) as usize];
    [v[i], v[j], v[k]]
}

/// The point types of a call of `dihedral_sign`, in the order of the loads.
const DIHEDRAL_KINDS: [&str; 3] = ["4_lnc", "4_bpt", "mixed"];

/// One call of `dihedral_sign` of the kind `kind`.
fn dihedral_case(rng: &mut Rng, precision: Precision, kind: &str, near: bool) -> [AnyPoint; 4] {
    let [e0, e1, w1, w2] = hinge(rng, precision, near);
    match kind {
        "4_lnc" => [
            lnc(rng, w2, e0),
            lnc(rng, w1, e1),
            lnc(rng, e0, e1),
            lnc(rng, e0, e1),
        ],
        "4_bpt" => [
            bpt(rng, e0, e1, w2),
            bpt(rng, e0, e1, w1),
            bpt_on_edge(rng, e0, e1, w1),
            bpt_on_edge(rng, e1, e0, w2),
        ],
        "mixed" => [
            lnc(rng, w2, e1),
            bpt(rng, e0, e1, w1),
            lnc(rng, e0, e1),
            explicit(e0),
        ],
        _ => unreachable!("no kind {kind}"),
    }
}

/// The point types of a call of `dot_sign_3d`, in the order of the loads.
const DOT_KINDS: [&str; 2] = ["3_bpt", "3_lnc"];

/// One call of `dot_sign_3d` of the kind `kind`.
fn dot_case(rng: &mut Rng, precision: Precision, kind: &str, near: bool) -> [AnyPoint; 3] {
    let ([t0, t1, t2], [x0, x1, x2], s) = prism(rng, precision, near);
    match kind {
        "3_bpt" => {
            let uv = bpt_params(rng);
            [
                bpt_at(x0, x1, x2, uv),
                bpt_at(t0, t1, t2, uv),
                bpt(rng, t1, t2, s),
            ]
        }
        "3_lnc" => {
            let t = param(rng);
            [lnc_at(x0, x1, t), lnc_at(t0, t1, t), lnc(rng, s, t2)]
        }
        _ => unreachable!("no kind {kind}"),
    }
}

/// The point types of a call of `orient3d`, in the order of the loads.
const ORIENT3D_KINDS: [&str; 2] = ["4_bpt", "4_lnc"];

/// One call of `orient3d` of the kind `kind`.
fn orient3d_case(rng: &mut Rng, precision: Precision, kind: &str, near: bool) -> [AnyPoint; 4] {
    let v = patch(rng, precision, near);
    // The last point is on the triangle of the moved vertex in the nearly
    // degenerate family, and on a random triangle of the patch otherwise.
    let last = if near {
        [v[0], v[1], v[6]]
    } else {
        patch_triangle(rng, &v)
    };
    match kind {
        "4_bpt" => {
            let [a, b, c] = std::array::from_fn(|_| {
                let [p, q, r] = patch_triangle(rng, &v);
                bpt(rng, p, q, r)
            });
            let [p, q, r] = last;
            [a, b, c, bpt(rng, p, q, r)]
        }
        // The first three on the three edges of one triangle, so that they
        // are not collinear; the last on an edge with the last vertex, the
        // moved one in the nearly degenerate family.
        "4_lnc" => {
            let [p, q, r] = patch_triangle(rng, &v);
            let [s, _, u] = last;
            [
                lnc(rng, p, q),
                lnc(rng, q, r),
                lnc(rng, r, p),
                lnc(rng, u, s),
            ]
        }
        _ => unreachable!("no kind {kind}"),
    }
}

/// The loads of `stages` built by `case`, six for each kind: for each
/// precision of [`PRECISIONS`], the nearly degenerate family, then the
/// degenerate one, each with the next seed from `seed`. Only calls that the filters of `stages` do not decide are kept.
fn loads<const N: usize>(
    stages: &ExactStages<N>,
    kinds: &[&str],
    seed: u64,
    case: fn(&mut Rng, Precision, &str, bool) -> [AnyPoint; N],
) -> Vec<ExactLoad<N>> {
    let mut out = Vec::new();
    for (k, kind) in kinds.iter().enumerate() {
        let families = PRECISIONS
            .into_iter()
            .flat_map(|precision| [(precision, true), (precision, false)]);
        for (f, ((name, precision), near)) in families.enumerate() {
            let mut rng = Rng::new(seed + (6 * k + f) as u64);
            let mut cases = Vec::with_capacity(EXACT_LOAD_SIZE);
            let mut tries = 0;
            while cases.len() < EXACT_LOAD_SIZE {
                tries += 1;
                assert!(
                    tries <= 100 * EXACT_LOAD_SIZE,
                    "{} {kind}: the filters decide almost every call",
                    stages.name
                );
                let c = case(&mut rng, precision, kind, near);
                if (stages.filters)(&c).is_none() {
                    cases.push(c);
                }
            }
            let family = if near {
                "near_degenerate"
            } else {
                "degenerate"
            };
            out.push(ExactLoad {
                name: format!("{kind}/{name}/{family}"),
                degenerate: !near,
                cases,
            });
        }
    }
    out
}

/// The loads of `dihedral_sign`: `4_lnc`, `4_bpt` and `mixed` (`a` an
/// `Lnc`, `b` a `Bpt`, `c` an `Lnc` and `d` explicit), each nearly
/// degenerate, then degenerate.
#[must_use]
pub fn dihedral_sign_loads() -> Vec<ExactLoad<4>> {
    loads(
        &DIHEDRAL_SIGN,
        &DIHEDRAL_KINDS,
        EXACT_LOAD_SEED,
        dihedral_case,
    )
}

/// The loads of `dot_sign_3d`: `3_bpt` and `3_lnc`, each nearly degenerate,
/// then degenerate.
#[must_use]
pub fn dot_sign_3d_loads() -> Vec<ExactLoad<3>> {
    loads(&DOT_SIGN_3D, &DOT_KINDS, EXACT_LOAD_SEED + 32, dot_case)
}

/// The loads of `orient3d`: `4_bpt` and `4_lnc`, each nearly degenerate,
/// then degenerate.
#[must_use]
pub fn orient3d_exact_loads() -> Vec<ExactLoad<4>> {
    loads(
        &ORIENT3D,
        &ORIENT3D_KINDS,
        EXACT_LOAD_SEED + 64,
        orient3d_case,
    )
}
