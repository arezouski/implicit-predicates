//! Generators of points in space and of sets of four or five points with a
//! given relationship, for the predicates on points.
//!
//! The exact relationships (coplanar, cospherical, repeated) are built on a
//! grid of small integers times a power of two, where every operation used
//! to build them is exact in `f64`.
//!
//! [`lnc_params`] gives the parents and the parameter of a point on a line.

use proptest::prelude::*;

use crate::values::{any_finite, leaf, step_ulps};

/// A point in space.
pub type Point3 = [f64; 3];

/// Four points in space.
pub type Points4 = [Point3; 4];

/// A point whose coordinates are uniform in `[-1, 1)`.
pub fn point3() -> impl Strategy<Value = Point3> {
    [-1.0..1.0f64, -1.0..1.0f64, -1.0..1.0f64]
}

/// Four points from [`point3`]: in general position, almost always.
pub fn random4() -> impl Strategy<Value = Points4> {
    [point3(), point3(), point3(), point3()]
}

/// The power-of-two exponents of the grids: every nonzero coordinate of a
/// grid point lies in `2^-150 ..= 2^212`, and so does every difference of
/// two of them.
const GRID_EXPONENTS: std::ops::RangeInclusive<i32> = -150..=200;

/// `n * 2^exponent`, exactly, for an integer `|n| < 2^12`.
fn on_grid(n: i32, exponent: i32) -> f64 {
    f64::from(n) * 2f64.powi(exponent)
}

/// Four points whose coordinates are integers in `-16..=16` times a common
/// power of two between `2^-150` and `2^200`.
pub fn grid4() -> impl Strategy<Value = Points4> {
    let point = || [-16i32..=16, -16i32..=16, -16i32..=16];
    (GRID_EXPONENTS, [point(), point(), point(), point()])
        .prop_map(|(k, p)| p.map(|point| point.map(|n| on_grid(n, k))))
}

/// Four exactly coplanar points: three points of a grid of integers in
/// `-64..=64` times a power of two, and the affine combination
/// `a + s (b - a) + t (c - a)` of them with integers `s` and `t` in
/// `-3..=3`, placed at a random position among the four.
///
/// The combination is computed on the integers, which stay below `2^11`,
/// and then scaled, so it is exact.
pub fn coplanar4() -> impl Strategy<Value = Points4> {
    let point = || [-64i32..=64, -64i32..=64, -64i32..=64];
    (
        GRID_EXPONENTS,
        [point(), point(), point()],
        -3i32..=3,
        -3i32..=3,
        0usize..4,
    )
        .prop_map(|(k, [a, b, c], s, t, position)| {
            let d: [i32; 3] = std::array::from_fn(|i| a[i] + s * (b[i] - a[i]) + t * (c[i] - a[i]));
            let mut p = [a, b, c, d].map(|point| point.map(|n| on_grid(n, k)));
            p.swap(3, position);
            p
        })
}

/// Points from [`coplanar4`] with one to three distinct coordinates each
/// moved by one to three units in the last place, up or down.
///
/// The moved coordinates are distinct, so the steps never cancel each
/// other; the result is still exactly coplanar when every moved coordinate
/// happens not to change the determinant.
pub fn near_coplanar4() -> impl Strategy<Value = Points4> {
    let coordinates = prop::sample::subsequence((0..12).collect::<Vec<usize>>(), 1..=3);
    let ulps = prop_oneof![-3i32..=-1, 1i32..=3];
    (coplanar4(), coordinates, [ulps.clone(), ulps.clone(), ulps]).prop_map(
        |(mut p, coordinates, ulps)| {
            for (index, ulps) in coordinates.into_iter().zip(ulps) {
                let (point, axis) = (index / 3, index % 3);
                p[point][axis] = step_ulps(p[point][axis], ulps);
            }
            p
        },
    )
}

/// Points from [`random4`] or [`grid4`] in which one point is a copy of
/// another; in half of the cases a second point is also a copy.
pub fn repeated4() -> impl Strategy<Value = Points4> {
    let base = prop_oneof![random4(), grid4()];
    // A point and another one, `1..=3` positions after it.
    let copy = (0usize..4, 1usize..4).prop_map(|(from, offset)| (from, (from + offset) % 4));
    (base, copy.clone(), prop::option::of(copy)).prop_map(|(mut p, first, second)| {
        for (from, to) in std::iter::once(first).chain(second) {
            p[to] = p[from];
        }
        p
    })
}

/// Points from [`grid4`] in which some coordinates are replaced by `0.0` or
/// `-0.0`.
pub fn signed_zeros4() -> impl Strategy<Value = Points4> {
    // Per coordinate: keep it, or replace it with 0.0 or -0.0.
    let choice = || prop_oneof![2 => Just(None), 1 => Just(Some(0.0)), 1 => Just(Some(-0.0))];
    let point = || [choice(), choice(), choice()];
    (grid4(), [point(), point(), point(), point()]).prop_map(|(mut p, zeros)| {
        for (point, zeros) in p.iter_mut().zip(zeros) {
            for (x, zero) in point.iter_mut().zip(zeros) {
                if let Some(zero) = zero {
                    *x = zero;
                }
            }
        }
        p
    })
}

/// Four points whose coordinates mix every magnitude: any finite value,
/// subnormals, values near `2^-1000` and `2^1000`, zeros of both signs, the
/// largest finite value, and each of `edges` and its two neighbours, with
/// either sign.
///
/// `edges` are typically the ends of the ranges of the stages under test,
/// so that values just inside and just outside each range are common.
///
/// # Panics
///
/// Panics if `edges` is empty.
pub fn extreme4(edges: &[f64]) -> impl Strategy<Value = Points4> + use<> {
    let mut near_edges = Vec::new();
    for &x in edges {
        near_edges.extend([x.next_down(), x, x.next_up()]);
    }
    let around = prop::sample::select(near_edges);
    let value = prop_oneof![
        1 => any_finite(),
        1 => leaf(-1074, -1023, 52),
        1 => leaf(-1010, -990, 53),
        1 => leaf(990, 1010, 53),
        1 => prop_oneof![Just(0.0), Just(-0.0), Just(f64::MAX), Just(f64::MIN_POSITIVE)],
        2 => (around, any::<bool>()).prop_map(|(x, negative)| if negative { -x } else { x }),
    ]
    // Boxed, so that the value trees of the twelve coordinates stay small.
    .boxed();
    let point = [value.clone(), value.clone(), value];
    [point.clone(), point.clone(), point.clone(), point]
}

/// The parents `p`, `q` and the parameter `t` of a point on the line
/// through them, `q + t (p - q)`.
pub type LncParams = (Point3, Point3, f64);

/// Parents and parameters of points on a line, mixing with equal weights:
///
/// - generic parents in `[-1, 1)^3` and `t` in `[0, 1]`;
/// - parents that share one to three coordinates;
/// - `t` at the ends of `[0, 1]` or just inside them, and tiny;
/// - `t` outside `[0, 1]`;
/// - coordinates of every magnitude, as in [`extreme4`], with `t` of every
///   magnitude too;
/// - tiny parents and parameters whose products underflow, with one parent
///   often zero, so that the only error is that of the product;
/// - parents a few units in the last place apart.
///
/// Every value is finite.
pub fn lnc_params() -> impl Strategy<Value = LncParams> {
    let unit = || 0.0..=1.0f64;
    let generic = (point3(), point3(), unit());
    let shared = (
        point3(),
        point3(),
        unit(),
        [any::<bool>(), any::<bool>(), any::<bool>()],
    )
        .prop_map(|(p, mut q, t, shared)| {
            for k in 0..3 {
                if shared[k] {
                    q[k] = p[k];
                }
            }
            (p, q, t)
        });
    let ends = prop::sample::select(vec![
        0.0,
        -0.0,
        1.0,
        1.0f64.next_down(),
        f64::MIN_POSITIVE,
        f64::from_bits(1),
        2f64.powi(-600),
        2f64.powi(-53),
    ]);
    let outside = prop::sample::select(vec![
        -1.5,
        2.0,
        1e10,
        -1e-5,
        1.0f64.next_up(),
        -f64::from_bits(1),
        -1.0,
        3.5e300,
    ]);
    let value = || {
        prop_oneof![
            any_finite(),
            leaf(-1074, -1023, 52),
            leaf(-1010, -990, 53),
            leaf(990, 1010, 53),
            prop_oneof![
                Just(0.0),
                Just(-0.0),
                Just(f64::MAX),
                Just(f64::MIN_POSITIVE)
            ],
        ]
        .boxed()
    };
    let extreme_point = || [value(), value(), value()];
    let extreme = (
        extreme_point(),
        extreme_point(),
        prop_oneof![value(), unit(), leaf(-1074, 0, 53)],
    );
    let tiny = || prop_oneof![leaf(-1074, -500, 53), Just(0.0), Just(-0.0)];
    let underflow = (
        [tiny(), tiny(), tiny()],
        [tiny(), tiny(), tiny()],
        leaf(-700, -1, 53),
    );
    let ulps = || -3i32..=3;
    let close = (point3(), [ulps(), ulps(), ulps()], unit()).prop_map(|(p, steps, t)| {
        let q = std::array::from_fn(|k| step_ulps(p[k], steps[k]));
        (p, q, t)
    });
    prop_oneof![
        generic,
        shared,
        (point3(), point3(), ends),
        (point3(), point3(), outside),
        extreme,
        underflow,
        close,
    ]
}

/// Five points in space, the arguments of `insphere`.
pub type Points5 = [Point3; 5];

/// Five points from [`point3`]: in general position, almost always.
pub fn random5() -> impl Strategy<Value = Points5> {
    [point3(), point3(), point3(), point3(), point3()]
}

/// Five points whose coordinates are integers in `-16..=16` times a common
/// power of two between `2^-150` and `2^200`.
pub fn grid5() -> impl Strategy<Value = Points5> {
    let point = || [-16i32..=16, -16i32..=16, -16i32..=16];
    (
        GRID_EXPONENTS,
        [point(), point(), point(), point(), point()],
    )
        .prop_map(|(k, p)| p.map(|point| point.map(|n| on_grid(n, k))))
}

/// The squares of the radii of the spheres of [`cospherical5`]: each sphere
/// about a point of the integer lattice holds from 30 to a few hundred
/// points of the lattice.
const RADII_SQUARED: [i32; 6] = [9, 25, 49, 81, 169, 625];

/// The points of the integer lattice at distance `sqrt(r2)` from the
/// origin.
fn lattice_sphere(r2: i32) -> Vec<[i32; 3]> {
    let r = (1..).find(|r| r * r >= r2).expect("a radius");
    let range = || -r..=r;
    range()
        .flat_map(|x| range().flat_map(move |y| range().map(move |z| [x, y, z])))
        .filter(|[x, y, z]| x * x + y * y + z * z == r2)
        .collect()
}

/// Five exactly cospherical points: points of the integer lattice on a
/// sphere of radius 3 to 25 about a centre with integer coordinates in
/// `-64..=64`, times a common power of two between `2^-150` and `2^200`.
/// Points may repeat, and four of them may be coplanar.
///
/// The coordinates are integers below `2^7` before the scaling, so every
/// point is exact.
pub fn cospherical5() -> impl Strategy<Value = Points5> {
    let spheres: Vec<Vec<[i32; 3]>> = RADII_SQUARED.into_iter().map(lattice_sphere).collect();
    let index = || any::<prop::sample::Index>();
    (
        GRID_EXPONENTS,
        prop::sample::select(spheres),
        [-64i32..=64, -64i32..=64, -64i32..=64],
        [index(), index(), index(), index(), index()],
    )
        .prop_map(|(k, sphere, centre, at)| {
            at.map(|i| {
                let offset = i.get(&sphere);
                std::array::from_fn(|axis| on_grid(centre[axis] + offset[axis], k))
            })
        })
}

/// Points from [`cospherical5`] with one to three coordinates of one point,
/// at any position, moved by one to three units in the last place, up or
/// down: four points on a sphere and one just off it, or still on it when
/// the moves happen not to change the value.
pub fn near_cospherical5() -> impl Strategy<Value = Points5> {
    let ulps = || prop_oneof![-3i32..=-1, 1i32..=3];
    (
        cospherical5(),
        0usize..5,
        prop::sample::subsequence(vec![0usize, 1, 2], 1..=3),
        [ulps(), ulps(), ulps()],
    )
        .prop_map(|(mut p, point, axes, ulps)| {
            for (axis, ulps) in axes.into_iter().zip(ulps) {
                p[point][axis] = step_ulps(p[point][axis], ulps);
            }
            p
        })
}
