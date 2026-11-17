//! Loads of the public predicates `dot_sign_3d`, `dihedral_sign`, `orient3d`
//! and `insphere` on points of any type: the whole chain, filter included,
//! unlike the loads of [`super::exact_loads`], which are left to the exact
//! stages.
//!
//! The loads of the angle predicates have explicit points, `Lnc`, `Bpt`, and
//! a mix of types, in three families:
//!
//! - `random`: points in general position, which the filter decides;
//! - `near_right`: the angle is right by construction but for one vertex
//!   moved by one to three ulps, so its exact sign is not zero but too
//!   small for the filter: its first test, `filter_points`, declines the
//!   call (the second test, `filter_axes_points`, may still decide a few);
//! - `right`: the angle is exactly right, or a triangle is degenerate, so
//!   the exact sign is zero, and the first test of the filter declines the
//!   call.
//!
//! The loads of `orient3d` with `Bpt` and of `insphere` with `Bpt` have the
//! families `random`, `near_degenerate` and `degenerate`, the points
//! coplanar (`orient3d`) or cospherical (`insphere`) by construction.
//!
//! Every implicit load comes in two precisions, which stand for different
//! inputs. The calls a mesher records have mostly short parameters (dyadic
//! numbers of at most 8 bits, midpoints among them) and exact coordinates of
//! one component, so `mesh` is the representative load. `params53`, with
//! random multiples of 2^-53 as parameters, makes the exact coordinates as
//! long as an arbitrary point has: it is the adverse case. Both have
//! vertices on the 2^-20 grid. The families of `insphere` that need an exact
//! construction exist only for `mesh`, whose parameters keep the
//! construction exact in `f64`.
//!
//! The names of the loads are `<points>/<precision>/<family>`, with
//! `explicit/grid` for the explicit loads. Every call is checked against
//! the hand-written oracle of [`super::exact_signs`] when the load is built:
//! a call is kept only if its exact sign and the decision of the filter are
//! those of its family.

use implicit_predicates::stages::{dihedral_sign, dot_sign_3d, insphere, orient3d};
use implicit_predicates::{AnyPoint, Bpt, Point, Sign};

use super::exact_loads::{
    Precision, bpt, bpt_at, coord, dihedral_case, dot_case, explicit, hinge, lnc, lnc_at,
    orient3d_case, param, prism,
};
use super::exact_signs::{
    exact_dihedral_sign, exact_dot_sign_3d, exact_insphere_sign, exact_orient3d_sign,
};
use super::{SET_SIZE, int_in, nudge, small_ulps};
use crate::Rng;

/// Seed of the loads: the load of index `i` of a predicate uses
/// `PUBLIC_LOAD_SEED + i` for `dihedral_sign`, `+ 100 + i` for
/// `dot_sign_3d`, `+ 200 + i` for `orient3d` and `+ 300 + i` for `insphere`.
pub const PUBLIC_LOAD_SEED: u64 = 0x7075_626c;

/// What a call of a load is made of with respect to its predicate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// In general position: the filter decides.
    Random,
    /// Almost degenerate: the exact sign is not zero and the filter
    /// declines.
    Near,
    /// Exactly degenerate: the exact sign is zero.
    Exact,
}

/// The names of the three families of a predicate, in the order of
/// [`Family`].
type FamilyNames = [&'static str; 3];

/// The names of the families of the angle predicates.
const ANGLE_FAMILIES: FamilyNames = ["random", "near_right", "right"];

/// The names of the families of the predicates of coplanar and cospherical
/// points.
const DEGENERATE_FAMILIES: FamilyNames = ["random", "near_degenerate", "degenerate"];

/// A load: calls of one predicate with one combination of point types and
/// one precision, of one family.
#[derive(Clone, Debug)]
pub struct PublicLoad<const N: usize> {
    /// The name, `<points>/<precision>/<family>`.
    pub name: String,
    /// The family of every call.
    pub family: Family,
    /// The calls.
    pub cases: Vec<[AnyPoint; N]>,
}

/// The precisions of the implicit loads: the representative one and the
/// adverse one.
const PRECISIONS: [(&str, Precision); 2] = [
    ("mesh", Precision::Mesh),
    ("params53", Precision::Grid(-20)),
];

/// The vertices of the explicit loads: random multiples of 2^-20, mapped by
/// the rotation of the hinges and prisms.
const GRID: Precision = Precision::Grid(-20);

/// The type of each point of a call, one letter each: `E` explicit, `L`
/// `Lnc`, `B` `Bpt`.
type Slots<const N: usize> = [char; N];

/// What builds the calls of one load.
struct Builder<'a, const N: usize> {
    /// The first test of the filter of the predicate, `filter_points`.
    filter: fn(&[AnyPoint; N]) -> Option<Sign>,
    /// The exact sign of a call, from the definition of the predicate.
    oracle: fn(&[AnyPoint; N]) -> i8,
    /// A call of a family.
    make: &'a dyn Fn(&mut Rng, Family) -> [AnyPoint; N],
}

impl<const N: usize> Builder<'_, N> {
    /// The load of the family `family`, named `prefix/<family name>` after
    /// `names`, with the random stream of `seed`.
    ///
    /// # Panics
    ///
    /// Panics if the construction does not give [`SET_SIZE`] calls of the
    /// family in `20 * SET_SIZE` tries.
    fn load(&self, prefix: &str, names: FamilyNames, family: Family, seed: u64) -> PublicLoad<N> {
        let mut rng = Rng::new(seed);
        let mut cases = Vec::with_capacity(SET_SIZE);
        for _ in 0..20 * SET_SIZE {
            let case = (self.make)(&mut rng, family);
            let decided = (self.filter)(&case).is_some();
            let zero = (self.oracle)(&case) == 0;
            let wanted = match family {
                Family::Random => decided,
                Family::Near => !decided && !zero,
                Family::Exact => zero,
            };
            if wanted {
                cases.push(case);
                if cases.len() == SET_SIZE {
                    break;
                }
            }
        }
        let name = format!("{prefix}/{}", names[family as usize]);
        assert_eq!(cases.len(), SET_SIZE, "{name}: not enough calls built");
        PublicLoad {
            name,
            family,
            cases,
        }
    }
}

/// The families, in the order of [`Family`].
const FAMILIES: [Family; 3] = [Family::Random, Family::Near, Family::Exact];

/// A random vertex of the grid of `precision`.
fn random_vertex(rng: &mut Rng, precision: Precision) -> [f64; 3] {
    std::array::from_fn(|_| coord(rng, precision))
}

/// A point of the type `slot` on random vertices: an `Lnc` on a random
/// segment, a `Bpt` in a random triangle.
fn random_point(rng: &mut Rng, precision: Precision, slot: char) -> AnyPoint {
    let mut v = || random_vertex(rng, precision);
    match slot {
        'E' => explicit(v()),
        'L' => {
            let (p, q) = (v(), v());
            lnc(rng, precision, p, q)
        }
        'B' => {
            let (p, q, r) = (v(), v(), v());
            bpt(rng, precision, p, q, r)
        }
        other => unreachable!("no point type {other}"),
    }
}

/// A call whose points are random, of the types `slots`.
fn random_case<const N: usize>(
    rng: &mut Rng,
    precision: Precision,
    slots: Slots<N>,
) -> [AnyPoint; N] {
    slots.map(|slot| random_point(rng, precision, slot))
}

/// The loads of one predicate: for each combination of point types, the
/// explicit one on the grid, then one for each precision, each in the three
/// families, from `seed`.
fn loads<const N: usize>(
    filter: fn(&[AnyPoint; N]) -> Option<Sign>,
    oracle: fn(&[AnyPoint; N]) -> i8,
    names: FamilyNames,
    seed: u64,
    kinds: &[(&str, Slots<N>)],
    case: fn(&mut Rng, Precision, &str, Family) -> [AnyPoint; N],
) -> Vec<PublicLoad<N>> {
    let mut out = Vec::new();
    let mut index = 0;
    let mut add = |prefix: String, precision: Precision, kind: &str, slots: Slots<N>| {
        let make = |rng: &mut Rng, family| match family {
            Family::Random => random_case(rng, precision, slots),
            _ => case(rng, precision, kind, family),
        };
        let builder = Builder {
            filter,
            oracle,
            make: &make,
        };
        for family in FAMILIES {
            out.push(builder.load(&prefix, names, family, seed + index));
            index += 1;
        }
    };
    for &(kind, slots) in kinds {
        if kind == "explicit" {
            add("explicit/grid".to_string(), GRID, kind, slots);
        } else {
            for (precision_name, precision) in PRECISIONS {
                add(format!("{kind}/{precision_name}"), precision, kind, slots);
            }
        }
    }
    out
}

/// A call of `dihedral_sign` of the family `family`; the right angle of the
/// hinge, `a` and `b` on its two triangles, the edge `c d`.
fn dihedral_call(rng: &mut Rng, precision: Precision, kind: &str, family: Family) -> [AnyPoint; 4] {
    let near = family == Family::Near;
    if kind == "explicit" {
        let [e0, e1, w1, w2] = hinge(rng, precision, near);
        return [w2, w1, e0, e1].map(explicit);
    }
    dihedral_case(rng, precision, kind, near)
}

/// A call of `dot_sign_3d` of the family `family`; the angle at `q`, in a
/// prism.
fn dot_call(rng: &mut Rng, precision: Precision, kind: &str, family: Family) -> [AnyPoint; 3] {
    let near = family == Family::Near;
    let ([t0, t1, t2], [x0, x1, _], s) = prism(rng, precision, near);
    match kind {
        "explicit" => [x0, t0, s].map(explicit),
        // Two `Lnc` of the same parameter on the translated edges, and a
        // `Bpt` of the plane of the base.
        "mixed" => {
            let t = param(rng, precision);
            [
                lnc_at(x0, x1, t),
                lnc_at(t0, t1, t),
                bpt(rng, precision, t1, t2, s),
            ]
        }
        _ => dot_case(rng, precision, kind, near),
    }
}

/// The loads of `dihedral_sign`: `explicit`, `4_lnc`, `4_bpt` and `mixed`
/// (`a` an `Lnc`, `b` a `Bpt`, `c` an `Lnc` and `d` explicit), each in the
/// families of [`ANGLE_FAMILIES`].
#[must_use]
pub fn dihedral_sign_public_loads() -> Vec<PublicLoad<4>> {
    loads(
        |[a, b, c, d]| dihedral_sign::filter_points(a, b, c, d),
        exact_dihedral_sign,
        ANGLE_FAMILIES,
        PUBLIC_LOAD_SEED,
        &[
            ("explicit", ['E'; 4]),
            ("4_lnc", ['L'; 4]),
            ("4_bpt", ['B'; 4]),
            ("mixed", ['L', 'B', 'L', 'E']),
        ],
        dihedral_call,
    )
}

/// The loads of `dot_sign_3d`: `explicit`, `3_lnc`, `3_bpt` and `mixed` (two
/// `Lnc` and a `Bpt`), each in the families of [`ANGLE_FAMILIES`].
#[must_use]
pub fn dot_sign_3d_public_loads() -> Vec<PublicLoad<3>> {
    loads(
        |[p, q, r]| dot_sign_3d::filter_points(p, q, r),
        exact_dot_sign_3d,
        ANGLE_FAMILIES,
        PUBLIC_LOAD_SEED + 100,
        &[
            ("explicit", ['E'; 3]),
            ("3_lnc", ['L'; 3]),
            ("3_bpt", ['B'; 3]),
            ("mixed", ['L', 'L', 'B']),
        ],
        dot_call,
    )
}

/// A call of `orient3d` on four `Bpt` of a planar patch, in the families of
/// [`DEGENERATE_FAMILIES`].
fn orient3d_call(rng: &mut Rng, precision: Precision, kind: &str, family: Family) -> [AnyPoint; 4] {
    orient3d_case(rng, precision, kind, family == Family::Near)
}

/// The loads of `orient3d` with four `Bpt`, as [`DEGENERATE_FAMILIES`] names
/// the families.
#[must_use]
pub fn orient3d_bpt_loads() -> Vec<PublicLoad<4>> {
    loads(
        |[a, b, c, d]| orient3d::filter_points(a, b, c, d),
        exact_orient3d_sign,
        DEGENERATE_FAMILIES,
        PUBLIC_LOAD_SEED + 200,
        &[("4_bpt", ['B'; 4])],
        orient3d_call,
    )
}

/// A `Bpt` at the midpoint of the segment `p q` of its triangle `p q r`,
/// that is exactly `c`: `p = c + (1 - u) w`, `q = c - u w`, with `u` a short
/// dyadic parameter and `u + v = 1`.
fn bpt_equal_to(rng: &mut Rng, precision: Precision, c: [f64; 3]) -> AnyPoint {
    let u = param(rng, precision);
    let w = random_vertex(rng, precision);
    let p = std::array::from_fn(|i| c[i] + (1.0 - u) * w[i]);
    let q = std::array::from_fn(|i| c[i] - u * w[i]);
    let r = random_vertex(rng, precision);
    bpt_at(p, q, r, (u, 1.0 - u))
}

/// A call of `insphere`: five distinct corners of a box with integer
/// sides, exactly cospherical, those whose slot is `B` written as a `Bpt`
/// equal to the corner; with `near`, one coordinate of one parent moved by
/// a few ulps.
fn insphere_call(
    rng: &mut Rng,
    precision: Precision,
    slots: Slots<5>,
    family: Family,
) -> [AnyPoint; 5] {
    let origin = random_vertex(rng, precision);
    let sides: [f64; 3] = std::array::from_fn(|_| int_in(rng, 1, 8) as f64);
    // A partial Fisher-Yates shuffle of the eight corner masks.
    let mut masks: [usize; 8] = std::array::from_fn(|i| i);
    for i in 0..5 {
        let j = i + int_in(rng, 0, 7 - i as i64) as usize;
        masks.swap(i, j);
    }
    let mut call: [AnyPoint; 5] = std::array::from_fn(|i| {
        let corner = std::array::from_fn(|axis| {
            origin[axis] + ((masks[i] >> axis) & 1) as f64 * sides[axis]
        });
        match slots[i] {
            'B' => bpt_equal_to(rng, precision, corner),
            _ => explicit(corner),
        }
    });
    if family == Family::Near {
        let i = int_in(rng, 0, 4) as usize;
        let axis = int_in(rng, 0, 2) as usize;
        let ulps = small_ulps(rng);
        call[i] = match &call[i] {
            AnyPoint::Bpt(b) => {
                let mut p = *b.p();
                p[axis] = nudge(p[axis], ulps);
                AnyPoint::Bpt(Bpt::new(p, *b.q(), *b.r(), b.u(), b.v()).expect("valid parameters"))
            }
            AnyPoint::Explicit(e) => {
                let mut p = *e.approx();
                p[axis] = nudge(p[axis], ulps);
                explicit(p)
            }
            other => unreachable!("no point {other:?}"),
        };
    }
    call
}

/// The loads of `insphere` with `Bpt`: `5_bpt` and `mixed_bpt`, in which the
/// first two points are explicit. Both have the three families of
/// [`DEGENERATE_FAMILIES`] with `mesh` parameters, and `random` with
/// `params53` ones.
#[must_use]
pub fn insphere_bpt_loads() -> Vec<PublicLoad<5>> {
    let kinds: [(&str, Slots<5>); 2] = [
        ("5_bpt", ['B'; 5]),
        ("mixed_bpt", ['E', 'E', 'B', 'B', 'B']),
    ];
    let mut out = Vec::new();
    let mut index = 0;
    for (kind, slots) in kinds {
        for (precision_name, precision) in PRECISIONS {
            let make = |rng: &mut Rng, family| match family {
                Family::Random => random_case(rng, precision, slots),
                _ => insphere_call(rng, precision, slots, family),
            };
            let builder = Builder {
                filter: |[a, b, c, d, e]| insphere::filter_points(a, b, c, d, e),
                oracle: exact_insphere_sign,
                make: &make,
            };
            // The construction is exact only with short parameters.
            let families = if precision_name == "mesh" {
                &FAMILIES[..]
            } else {
                &FAMILIES[..1]
            };
            for &family in families {
                let seed = PUBLIC_LOAD_SEED + 300 + index;
                index += 1;
                out.push(builder.load(
                    &format!("{kind}/{precision_name}"),
                    DEGENERATE_FAMILIES,
                    family,
                    seed,
                ));
            }
        }
    }
    out
}
