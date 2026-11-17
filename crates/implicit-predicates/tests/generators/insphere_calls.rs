//! The calls of the property tests of `insphere`: five arguments, each an
//! explicit point, a point on a line or a point on a triangle, from random,
//! grid, cospherical and extreme points, and mesh-like ones: short dyadic
//! parameters, points that share parents, and points on a vertex or an edge
//! of a triangle whose corners are other arguments. Shared by the test
//! binaries that declare it with `#[path]`; they also declare `common`.

use implicit_predicates_test_support::{
    bpt_params, cospherical5, extreme_value, grid5, lnc_parameter, lnc_params, near_cospherical5,
    random5,
};
use proptest::prelude::*;

use crate::common::args::Arg;
use crate::common::insphere::Points5;

/// The five arguments of a call.
pub type Args5 = [Arg; 5];

/// The kinds of the implicit points of one call: all on lines, all on
/// triangles, or each on either.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mix {
    Lines,
    Triangles,
    Mixed,
}

impl Mix {
    /// Whether an implicit point of the call is on a triangle; `coin` decides
    /// it in a mixed call.
    pub fn on_triangle(self, coin: bool) -> bool {
        match self {
            Mix::Lines => false,
            Mix::Triangles => true,
            Mix::Mixed => coin,
        }
    }
}

/// The kinds of the implicit points of a call: in half of the calls they are
/// all on lines, as in every call recorded from a mesher, in a quarter all on
/// triangles, and in the other quarter each on either.
pub fn mix() -> impl Strategy<Value = Mix> {
    prop_oneof![
        2 => Just(Mix::Lines),
        1 => Just(Mix::Triangles),
        1 => Just(Mix::Mixed),
    ]
}

/// `x` with each coordinate moved by `steps[k]` units in the last place.
pub fn stepped(x: [f64; 3], steps: [i32; 3]) -> [f64; 3] {
    std::array::from_fn(|k| {
        let step = if steps[k] >= 0 {
            f64::next_up
        } else {
            f64::next_down
        };
        (0..steps[k].unsigned_abs()).fold(x[k], |y, _| step(y))
    })
}

/// A point on a line to stand for the point `x`: one whose parents are both
/// `x`, so that its exact value is `x`; one between `x` and `x` moved by a
/// few units in the last place, so that it is near `x` with coordinates of
/// several components; or any point on a line from `lnc_params`.
pub fn line_for(x: [f64; 3]) -> BoxedStrategy<Arg> {
    let t = || 0.0..=1.0f64;
    let steps = || [-3i32..=3, -3i32..=3, -3i32..=3];
    prop_oneof![
        t().prop_map(move |t| Arg::Lnc(x, x, t)),
        (steps(), t()).prop_map(move |(s, t)| Arg::Lnc(x, stepped(x, s), t)),
        lnc_params().prop_map(|(p, q, t)| Arg::Lnc(p, q, t)),
    ]
    // Boxed, so that the value trees of the five points stay small.
    .boxed()
}

/// A point on a triangle to stand for the point `x`: one whose parents are
/// all `x`, with any parameters, so that its exact value is `x`; one with
/// parents `x` and `x` moved by a few units in the last place, so that it is
/// near `x`; or any point on a triangle from `bpt_params`.
pub fn triangle_for(x: [f64; 3]) -> BoxedStrategy<Arg> {
    let u = || 0.0..=1.0f64;
    let steps = || [-3i32..=3, -3i32..=3, -3i32..=3];
    prop_oneof![
        (u(), u()).prop_map(move |(a, b)| Arg::Bpt(x, x, x, a, b)),
        (steps(), steps(), u(), u())
            .prop_map(move |(s, t, a, b)| { Arg::Bpt(x, stepped(x, s), stepped(x, t), a, b) }),
        bpt_params().prop_map(|(p, q, r, u, v)| Arg::Bpt(p, q, r, u, v)),
    ]
    .boxed()
}

/// The points `p` with none to five of them, at any positions, replaced by
/// points on a line from [`line_for`] or on a triangle from
/// [`triangle_for`], as [`mix`] chooses.
pub fn with_lines(p: Points5) -> impl Strategy<Value = Args5> {
    let implicit = move |m: Mix, i: usize| match m {
        Mix::Lines => line_for(p[i]),
        Mix::Triangles => triangle_for(p[i]),
        Mix::Mixed => prop_oneof![line_for(p[i]), triangle_for(p[i])].boxed(),
    };
    (
        prop::sample::subsequence(vec![0usize, 1, 2, 3, 4], 0..=5),
        mix().prop_flat_map(move |m| [0usize, 1, 2, 3, 4].map(|i| implicit(m, i))),
    )
        .prop_map(move |(at, lines)| {
            std::array::from_fn(|i| {
                if at.contains(&i) {
                    lines[i]
                } else {
                    Arg::Explicit(p[i])
                }
            })
        })
}

/// A parameter of a short dyadic kind: ends, midpoints, quarters and
/// 1/256, the parameters of real meshes.
fn short_parameter() -> impl Strategy<Value = f64> {
    prop::sample::select(vec![
        0.0,
        1.0,
        0.5,
        0.25,
        0.75,
        1.0 / 256.0,
        255.0 / 256.0,
        3.0 / 256.0,
    ])
}

/// Arguments from the points `p`, each explicit or an implicit point built
/// on other points of the call, as a mesh does it: on a segment or a
/// triangle with corners that are other arguments, with short dyadic
/// parameters, at a vertex or on an edge of the triangle; [`mix`] chooses
/// the kinds of the implicit points.
pub fn mesh_args(p: Points5) -> impl Strategy<Value = Args5> {
    let corner = || 0usize..5;
    let line = move || {
        (corner(), corner(), short_parameter())
            .prop_map(move |(a, b, t)| Some(Arg::Lnc(p[a], p[b], t)))
            .boxed()
    };
    let triangle = move || {
        prop_oneof![
            3 => (corner(), corner(), corner(), short_parameter(), short_parameter())
                .prop_map(move |(a, b, c, u, v)| Some(Arg::Bpt(p[a], p[b], p[c], u, v))),
            // At a vertex, or on an edge, of a triangle with another
            // argument as a corner.
            2 => (corner(), corner(), corner(), 0usize..6).prop_map(move |(a, b, c, kind)| {
                let (u, v) =
                    [(1.0, 0.0), (0.0, 1.0), (0.0, 0.0), (0.5, 0.5), (0.5, 0.0), (0.0, 0.5)][kind];
                Some(Arg::Bpt(p[a], p[b], p[c], u, v))
            }),
        ]
        .boxed()
    };
    // Boxed, so that the value trees of the five points stay small.
    let one = move |m: Mix| match m {
        Mix::Lines => prop_oneof![1 => Just(None), 3 => line()].boxed(),
        Mix::Triangles => prop_oneof![1 => Just(None), 3 => triangle()].boxed(),
        Mix::Mixed => prop_oneof![2 => Just(None), 1 => line(), 5 => triangle()].boxed(),
    };
    mix()
        .prop_flat_map(move |m| [one(m), one(m), one(m), one(m), one(m)])
        .prop_map(move |implicit| {
            std::array::from_fn(|i| implicit[i].unwrap_or(Arg::Explicit(p[i])))
        })
}

/// Calls on random points, points of a grid of small integers and points
/// of which four lie on a sphere and one is a few units in the last place
/// off it, with points on a line or on a triangle among them; and mesh-like
/// calls.
pub fn calls() -> impl Strategy<Value = Args5> {
    prop_oneof![
        3 => prop_oneof![random5(), grid5(), near_cospherical5()].prop_flat_map(with_lines),
        1 => prop_oneof![random5(), grid5(), cospherical5()].prop_flat_map(mesh_args),
    ]
}

/// A point on a line whose exact value is `p[i]`, or `p[j]` for another
/// point `j` of the call, by `kind`: parents both `p[i]`; `p[i]` at `t =
/// 1`; `p[i]` at `t = 0`; the midpoint of `2 p[i] - p[j]` and `p[j]`; and
/// `p[j]` at `t = 1`, which repeats it. On the grid of [`cospherical5`]
/// every input is exact.
pub fn line_at(p: &Points5, i: usize, j: usize, kind: usize, t: f64) -> Arg {
    let (x, y) = (p[i], p[j]);
    match kind {
        0 => Arg::Lnc(x, x, t),
        1 => Arg::Lnc(x, y, 1.0),
        2 => Arg::Lnc(y, x, 0.0),
        3 => Arg::Lnc(std::array::from_fn(|k| 2.0 * x[k] - y[k]), y, 0.5),
        _ => Arg::Lnc(y, x, 1.0),
    }
}

/// A point on a triangle whose exact value is `p[i]`, or `p[l]` for another
/// point `l` of the call, by `kind`: parents all `p[i]`; `p[i]` as the
/// vertex `p`, `q` or `r` of a triangle with `p[j]` and `p[l]` as the
/// others; the midpoint of the edge from `2 p[i] - p[j]` to `p[j]`, whose
/// third corner is `p[l]`; and `p[l]` as a vertex, which repeats it. On the
/// grid of [`cospherical5`] every input is exact.
pub fn triangle_at(p: &Points5, i: usize, j: usize, l: usize, kind: usize, t: f64) -> Arg {
    let (x, y, z) = (p[i], p[j], p[l]);
    match kind {
        0 => Arg::Bpt(x, x, x, t, 0.5 * t),
        1 => Arg::Bpt(x, y, z, 1.0, 0.0),
        2 => Arg::Bpt(y, x, z, 0.0, 1.0),
        3 => Arg::Bpt(y, z, x, 0.0, 0.0),
        4 => Arg::Bpt(std::array::from_fn(|k| 2.0 * x[k] - y[k]), z, y, 0.5, 0.0),
        _ => Arg::Bpt(z, x, y, 1.0, 0.0),
    }
}

/// Calls whose exact points lie on one sphere: from [`cospherical5`], with
/// a point repeated in half of them, and none to five points replaced by
/// points on a line from [`line_at`] or on a triangle from [`triangle_at`],
/// as [`mix`] chooses.
pub fn cospherical_args() -> impl Strategy<Value = Args5> {
    let line = || (0usize..5, 1usize..5, 0.0..=1.0f64).boxed();
    let triangle = || (0usize..6, 1usize..5, any::<bool>()).boxed();
    (
        (cospherical5(), prop::option::of((0usize..5, 1usize..5))),
        (
            mix(),
            prop::sample::subsequence(vec![0usize, 1, 2, 3, 4], 0..=5),
        ),
        [line(), line(), line(), line(), line()],
        [triangle(), triangle(), triangle(), triangle(), triangle()],
    )
        .prop_map(|((mut p, repeated), (m, at), lines, triangles)| {
            if let Some((i, k)) = repeated {
                p[(i + k) % 5] = p[i];
            }
            std::array::from_fn(|i| {
                let (kind, k, t) = lines[i];
                let (triangle_kind, other, coin) = triangles[i];
                if at.contains(&i) && m.on_triangle(coin) {
                    triangle_at(&p, i, (i + k) % 5, (i + other) % 5, triangle_kind, t)
                } else if at.contains(&i) {
                    line_at(&p, i, (i + k) % 5, kind, t)
                } else {
                    Arg::Explicit(p[i])
                }
            })
        })
}

/// Five arguments whose inputs are values of one family of
/// [`extreme_value`], with points on a line or on a triangle, as [`mix`]
/// chooses, in any number from none to five, at any positions; the others
/// are explicit.
pub fn extreme_args() -> impl Strategy<Value = Args5> {
    (0usize..5).prop_flat_map(|family| {
        let v = move || extreme_value(family);
        let point = move || [v(), v(), v()];
        // Boxed, so that the value trees of the points stay small.
        let line = move || (point(), point(), lnc_parameter()).boxed();
        let triangle =
            move || (point(), point(), point(), lnc_parameter(), lnc_parameter()).boxed();
        (
            [point(), point(), point(), point(), point()],
            [line(), line(), line(), line(), line()],
            [triangle(), triangle(), triangle(), triangle(), triangle()],
            prop::sample::subsequence(vec![0usize, 1, 2, 3, 4], 0..=5),
            (
                mix(),
                [
                    any::<bool>(),
                    any::<bool>(),
                    any::<bool>(),
                    any::<bool>(),
                    any::<bool>(),
                ],
            ),
        )
            .prop_map(|(explicit, lines, triangles, at, (m, coins))| {
                std::array::from_fn(|i| {
                    if at.contains(&i) && m.on_triangle(coins[i]) {
                        let (p, q, r, u, v) = triangles[i];
                        Arg::Bpt(p, q, r, u, v)
                    } else if at.contains(&i) {
                        let (p, q, t) = lines[i];
                        Arg::Lnc(p, q, t)
                    } else {
                        Arg::Explicit(explicit[i])
                    }
                })
            })
    })
}
