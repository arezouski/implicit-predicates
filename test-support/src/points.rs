//! Generators of points in space and of sets of four points with a given
//! relationship, for the predicates on points.
//!
//! The exact relationships (coplanar, repeated) are built on a grid of small
//! integers times a power of two, where every operation used to build them
//! is exact in `f64`.

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
