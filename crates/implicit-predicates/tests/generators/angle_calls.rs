//! The calls of the property tests of `dot_sign_3d` and `dihedral_sign`:
//! three or four arguments, each an explicit point, a point on a segment or a
//! point on a triangle, from random, grid, right-angled, repeated and extreme
//! points, and mesh-like ones: short dyadic parameters, points that share
//! parents, and points on a vertex or an edge of their triangle; and points
//! with full mantissas and widely spread exponents. Shared by the test
//! binaries that declare it with `#[path]`; they also declare `common`.

use implicit_predicates::{AnyPoint, Bpt, Explicit, Lnc, Point};
use implicit_predicates_oracle::Dyadic;
use implicit_predicates_test_support::{bpt_params, extreme_value, lnc_params, point3};
use proptest::prelude::*;

use crate::common::bpt::exact_bpt_by_hand;

/// One argument: an explicit point, `Lnc { p, q, t }`, the point
/// `q + t (p - q)`, or `Bpt { p, q, r, u, v }`, the point
/// `r + u (p - r) + v (q - r)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pt {
    Explicit([f64; 3]),
    Lnc([f64; 3], [f64; 3], f64),
    Bpt([f64; 3], [f64; 3], [f64; 3], f64, f64),
}

/// The exact coordinates of the point `q + t (p - q)`, from that formula
/// alone, on `Dyadic`.
fn exact_lnc_by_hand(p: &[f64; 3], q: &[f64; 3], t: f64) -> [Dyadic; 3] {
    let t = Dyadic::from_f64(t);
    std::array::from_fn(|k| {
        let (p, q) = (Dyadic::from_f64(p[k]), Dyadic::from_f64(q[k]));
        &q + &(&t * &(&p - &q))
    })
}

impl Pt {
    /// The point, as an [`AnyPoint`], built without validation.
    pub fn any(&self) -> AnyPoint {
        match *self {
            Pt::Explicit(p) => AnyPoint::Explicit(Explicit::new_unchecked(p)),
            Pt::Lnc(p, q, t) => AnyPoint::Lnc(Lnc::new_unchecked(p, q, t)),
            Pt::Bpt(p, q, r, u, v) => AnyPoint::Bpt(Bpt::new_unchecked(p, q, r, u, v)),
        }
    }

    /// The exact coordinates of the point, by hand from its definition.
    ///
    /// # Panics
    ///
    /// Panics if an input is not finite.
    pub fn exact(&self) -> [Dyadic; 3] {
        match *self {
            Pt::Explicit(p) => p.map(Dyadic::from_f64),
            Pt::Lnc(p, q, t) => exact_lnc_by_hand(&p, &q, t),
            Pt::Bpt(p, q, r, u, v) => exact_bpt_by_hand(&p, &q, &r, u, v),
        }
    }
}

/// The exact coordinates of the points of `args`.
pub fn exact_points<const N: usize>(args: &[Pt; N]) -> [[Dyadic; 3]; N] {
    args.each_ref().map(Pt::exact)
}

/// The points of `args` as [`AnyPoint`]s.
pub fn any_points<const N: usize>(args: &[Pt; N]) -> [AnyPoint; N] {
    args.each_ref().map(Pt::any)
}

/// A call on three points of any type, given to [`with_concrete3`].
pub trait Call3 {
    type Output;
    fn call<A: Point, B: Point, C: Point>(&mut self, a: &A, b: &B, c: &C) -> Self::Output;
}

/// A call on four points of any type, given to [`with_concrete4`].
pub trait Call4 {
    type Output;
    fn call<A: Point, B: Point, C: Point, D: Point>(
        &mut self,
        a: &A,
        b: &B,
        c: &C,
        d: &D,
    ) -> Self::Output;
}

/// Builds the point of `$arg` as its concrete type, `[f64; 3]` or `Explicit`
/// for an explicit point as `$raw` says, `Lnc` or `Bpt`, and evaluates `$body`
/// with it bound to `$x`.
macro_rules! concrete {
    ($arg:expr, $raw:expr, |$x:ident| $body:expr) => {
        match $arg {
            Pt::Explicit(p) if $raw => {
                let $x = &p;
                $body
            }
            Pt::Explicit(p) => {
                let $x = &Explicit::new_unchecked(p);
                $body
            }
            Pt::Lnc(p, q, t) => {
                let $x = &Lnc::new_unchecked(p, q, t);
                $body
            }
            Pt::Bpt(p, q, r, u, v) => {
                let $x = &Bpt::new_unchecked(p, q, r, u, v);
                $body
            }
        }
    };
}

/// `f` on the three points of `args`, each of its concrete type: an explicit
/// point is a `[f64; 3]` when `raw` is true and an [`Explicit`] otherwise.
pub fn with_concrete3<F: Call3>(args: &[Pt; 3], raw: bool, f: &mut F) -> F::Output {
    concrete!(args[0], raw, |a| concrete!(args[1], raw, |b| concrete!(
        args[2],
        raw,
        |c| f.call(a, b, c)
    )))
}

/// `f` on the four points of `args`, as [`with_concrete3`] builds them.
pub fn with_concrete4<F: Call4>(args: &[Pt; 4], raw: bool, f: &mut F) -> F::Output {
    concrete!(args[0], raw, |a| concrete!(args[1], raw, |b| concrete!(
        args[2],
        raw,
        |c| concrete!(args[3], raw, |d| f.call(a, b, c, d))
    )))
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

/// `2^k` for a small `k`, to scale a grid.
fn scale(k: i32) -> f64 {
    2f64.powi(k)
}

/// The exponents of the grids: every nonzero coordinate of a grid point, and
/// every combination of a few of them, lies in `2^-150 ..= 2^200`.
const GRID_EXPONENTS: std::ops::RangeInclusive<i32> = -150..=190;

/// A cross product of integer vectors.
fn cross(u: [i32; 3], v: [i32; 3]) -> [i32; 3] {
    [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ]
}

/// A vector of small integers.
fn ints(bound: i32) -> impl Strategy<Value = [i32; 3]> {
    [-bound..=bound, -bound..=bound, -bound..=bound]
}

/// The explicit points under the implicit ones of a call, and whether they
/// are on a grid, where `2 x - y` is exact.
pub type Base<const N: usize> = ([[f64; 3]; N], bool);

/// `N` points of a grid of small integers times a power of two.
fn grid<const N: usize>() -> impl Strategy<Value = Base<N>> {
    let point = || ints(6);
    (GRID_EXPONENTS, prop::collection::vec(point(), N)).prop_map(|(k, p)| {
        let p = std::array::from_fn(|i| p[i].map(|n| f64::from(n) * scale(k)));
        (p, true)
    })
}

/// `N` random points of the cube `[-1, 1)^3`.
fn random<const N: usize>() -> impl Strategy<Value = Base<N>> {
    prop::collection::vec(point3(), N).prop_map(|p| (std::array::from_fn(|i| p[i]), false))
}

/// `N` points of one family of [`extreme_value`] (values from `1e-300` to
/// `1e300`, subnormals, zeros).
fn extreme<const N: usize>() -> impl Strategy<Value = Base<N>> {
    (0usize..5).prop_flat_map(|family| {
        let v = move || extreme_value(family);
        prop::collection::vec([v(), v(), v()], N)
            .prop_map(|p| (std::array::from_fn(|i| p[i]), false))
    })
}

/// The base with one point, at any position, moved a few units in the last
/// place.
fn nudged<const N: usize>(base: Base<N>) -> impl Strategy<Value = Base<N>> {
    (0..N, [-3i32..=3, -3i32..=3, -3i32..=3]).prop_map(move |(i, steps)| {
        let (mut p, _) = base;
        p[i] = stepped(p[i], steps);
        (p, false)
    })
}

/// The bases of `dot_sign_3d`: `p`, `q`, `r`, with right angles at `q` among
/// them, exact on a grid or nudged by a few units in the last place, and
/// repeated points.
fn bases3() -> impl Strategy<Value = Base<3>> {
    let right = (GRID_EXPONENTS, ints(5), ints(5), ints(3)).prop_map(|(k, q, x, w)| {
        // `r - q = x * w` is perpendicular to `x = p - q`.
        let n = cross(x, w);
        let point = |v: [i32; 3]| std::array::from_fn(|i| f64::from(v[i]) * scale(k));
        let p = std::array::from_fn(|i| q[i] + x[i]);
        let r = std::array::from_fn(|i| q[i] + n[i]);
        ([point(p), point(q), point(r)], true)
    });
    let right = right.boxed();
    let nearly_right = right.clone().prop_flat_map(nudged);
    let repeated = (grid::<3>(), 0usize..3, 1usize..3).prop_map(|((mut p, g), i, k)| {
        p[(i + k) % 3] = p[i];
        (p, g)
    });
    prop_oneof![random(), grid(), extreme(), right, nearly_right, repeated]
}

/// The bases of `dihedral_sign`: `a`, `b`, `c`, `d`, with right dihedral
/// angles among them, exact on a grid or nudged by a few units in the last
/// place, degenerate triangles, and repeated points.
fn bases4() -> impl Strategy<Value = Base<4>> {
    let right = (
        GRID_EXPONENTS,
        ints(4),
        ints(4),
        ints(3),
        -2i32..=2,
        -2i32..=2,
    )
        .prop_map(|(k, d, e, x, s, m)| {
            // The edge is `d`, `d + e`. The triangle of `a = d + x` has the
            // normal `n = x X e`; for `b = d + s n + m e` the other cross
            // product is `s (n X e)`, which is perpendicular to `n`.
            let n = cross(x, e);
            let point = |v: [i32; 3]| std::array::from_fn(|i| f64::from(v[i]) * scale(k));
            let a = std::array::from_fn(|i| d[i] + x[i]);
            let b = std::array::from_fn(|i| d[i] + s * n[i] + m * e[i]);
            let c = std::array::from_fn(|i| d[i] + e[i]);
            ([point(a), point(b), point(c), point(d)], true)
        });
    let right = right.boxed();
    let nearly_right = right.clone().prop_flat_map(nudged);
    let degenerate = (
        GRID_EXPONENTS,
        ints(4),
        ints(4),
        ints(4),
        -3i32..=3,
        0usize..2,
    )
        .prop_map(|(k, d, e, y, s, which)| {
            // `a` or `b` on the line through `c` and `d`, or `c = d`.
            let point = |v: [i32; 3]| std::array::from_fn(|i| f64::from(v[i]) * scale(k));
            let on_line: [i32; 3] = std::array::from_fn(|i| d[i] + s * e[i]);
            let c: [i32; 3] = std::array::from_fn(|i| d[i] + e[i]);
            let other: [i32; 3] = std::array::from_fn(|i| d[i] + y[i]);
            let mut p = [point(on_line), point(other), point(c), point(d)];
            if which == 1 {
                p.swap(0, 1);
            }
            (p, true)
        });
    let repeated = (grid::<4>(), 0usize..4, 1usize..4).prop_map(|((mut p, g), i, k)| {
        p[(i + k) % 4] = p[i];
        (p, g)
    });
    prop_oneof![
        random(),
        grid(),
        extreme(),
        right,
        nearly_right,
        degenerate,
        repeated
    ]
}

/// The parameters of a short dyadic kind: ends, midpoints, quarters, eighths.
fn short_parameter() -> impl Strategy<Value = f64> {
    prop::sample::select(vec![0.0, 1.0, 0.5, 0.25, 0.75, 0.125, 0.375, 0.875])
}

/// A point whose exact value is `x`, of an implicit type, built from `x` and
/// other points `y`, `z` of the call: on a segment or a triangle with `x` as
/// an end, a vertex or the midpoint of an edge, or with parents that are all
/// `x`. `on_grid` says that `2 x - y` is exact, which two kinds need.
pub fn same_value(x: [f64; 3], y: [f64; 3], z: [f64; 3], on_grid: bool) -> BoxedStrategy<Pt> {
    let u = || 0.0..=1.0f64;
    let double: [f64; 3] = std::array::from_fn(|k| 2.0 * x[k] - y[k]);
    let mut kinds: Vec<BoxedStrategy<Pt>> = vec![
        // On a segment: parents both `x`; `x` at either end of a segment to
        // `y`.
        u().prop_map(move |t| Pt::Lnc(x, x, t)).boxed(),
        Just(Pt::Lnc(x, y, 1.0)).boxed(),
        Just(Pt::Lnc(y, x, 0.0)).boxed(),
        // On a triangle: parents all `x`; a vertex `p`, `q` or `r`, with
        // the others any points.
        (u(), u())
            .prop_map(move |(a, b)| Pt::Bpt(x, x, x, a, b))
            .boxed(),
        Just(Pt::Bpt(x, y, z, 1.0, 0.0)).boxed(),
        Just(Pt::Bpt(y, x, z, 0.0, 1.0)).boxed(),
        Just(Pt::Bpt(y, z, x, 0.0, 0.0)).boxed(),
        Just(Pt::Bpt(x, y, y, 1.0, 0.0)).boxed(),
    ];
    if on_grid {
        kinds.push(Just(Pt::Lnc(double, y, 0.5)).boxed());
        // The midpoint of the edge `p`, `r`, and of the edge `p`, `q`.
        kinds.push(Just(Pt::Bpt(double, y, y, 0.5, 0.0)).boxed());
        kinds.push(Just(Pt::Bpt(double, y, z, 0.5, 0.5)).boxed());
    }
    prop::strategy::Union::new(kinds).boxed()
}

/// An implicit point with parameters that are short dyadic numbers, built on
/// the points `x`, `y`, `z` of the call, so that it shares its parents with
/// the other arguments.
fn shared_parents(x: [f64; 3], y: [f64; 3], z: [f64; 3]) -> BoxedStrategy<Pt> {
    prop_oneof![
        short_parameter().prop_map(move |t| Pt::Lnc(x, y, t)),
        (short_parameter(), short_parameter()).prop_map(move |(u, v)| Pt::Bpt(x, y, z, u, v)),
    ]
    .boxed()
}

/// Arguments from `base` with none to all of the points, at any positions,
/// replaced by implicit ones: with the value of the point, with parents that
/// are other points of the call, or with random parameters.
fn with_implicit<const N: usize>(base: Base<N>) -> impl Strategy<Value = [Pt; N]> {
    let (p, on_grid) = base;
    let at = |i: usize| p[i];
    let choice = move |i: usize| -> BoxedStrategy<Pt> {
        let (y, z) = (at((i + 1) % N), at((i + 2) % N));
        prop_oneof![
            4 => Just(Pt::Explicit(at(i))),
            3 => same_value(at(i), y, z, on_grid),
            2 => shared_parents(at(i), y, z),
            1 => lnc_params().prop_map(|(p, q, t)| Pt::Lnc(p, q, t)),
            1 => bpt_params().prop_map(|(p, q, r, u, v)| Pt::Bpt(p, q, r, u, v)),
        ]
        .boxed()
    };
    let strategies: Vec<BoxedStrategy<Pt>> = (0..N).map(choice).collect();
    strategies.prop_map(|v| std::array::from_fn(|i| v[i]))
}

/// Calls of `dot_sign_3d`.
pub fn calls3() -> impl Strategy<Value = [Pt; 3]> {
    bases3().prop_flat_map(with_implicit)
}

/// Calls of `dihedral_sign`.
pub fn calls4() -> impl Strategy<Value = [Pt; 4]> {
    bases4().prop_flat_map(with_implicit)
}

/// Values of every magnitude that `f64` has at its ends and in the middle.
pub const EXTREMES: [f64; 10] = [
    1e-300,
    -1e-300,
    1e300,
    -1e300,
    5e-324,
    -5e-324,
    f64::MIN_POSITIVE,
    f64::MAX,
    0.0,
    1.0,
];

fn extreme_point() -> impl Strategy<Value = [f64; 3]> {
    prop::array::uniform3(prop::sample::select(EXTREMES.to_vec()))
}

/// `N` arguments whose inputs are among [`EXTREMES`]: explicit, on a segment
/// or on a triangle.
pub fn extreme_calls<const N: usize>() -> impl Strategy<Value = [Pt; N]> {
    let parameter = || prop::sample::select(vec![0.0, 1.0, 0.5, 1e-300, 5e-324, -1e300, 1e300]);
    let one = move || {
        prop_oneof![
            extreme_point().prop_map(Pt::Explicit),
            (extreme_point(), extreme_point(), parameter()).prop_map(|(p, q, t)| Pt::Lnc(p, q, t)),
            (
                extreme_point(),
                extreme_point(),
                extreme_point(),
                parameter(),
                parameter()
            )
                .prop_map(|(p, q, r, u, v)| Pt::Bpt(p, q, r, u, v)),
        ]
    };
    let strategies: Vec<_> = (0..N).map(|_| one().boxed()).collect();
    strategies.prop_map(|v| std::array::from_fn(|i| v[i]))
}

/// `N` explicit arguments whose coordinates are among [`EXTREMES`].
pub fn explicit_extreme_calls<const N: usize>() -> impl Strategy<Value = [Pt; N]> {
    prop::collection::vec(extreme_point(), N)
        .prop_map(|v| std::array::from_fn(|i| Pt::Explicit(v[i])))
}

/// Values of either sign with a full 53-bit significand whose exponent is
/// uniform in `exponents`, which must be exponents of normal numbers.
fn full_mantissa(exponents: std::ops::RangeInclusive<i32>) -> impl Strategy<Value = f64> {
    (exponents, any::<u64>(), any::<bool>()).prop_map(|(exp, bits, negative)| {
        let significand = f64::from_bits(1f64.to_bits() | (bits >> 12));
        let magnitude = significand * 2f64.powi(exp);
        if negative { -magnitude } else { magnitude }
    })
}

/// A point whose coordinates are values of [`full_mantissa`].
fn full_point(exponents: std::ops::RangeInclusive<i32>) -> impl Strategy<Value = [f64; 3]> {
    let v = move || full_mantissa(exponents.clone());
    [v(), v(), v()]
}

/// A parameter in `[0, 1)` with up to 53 random bits: a random multiple of
/// `2^-53`.
fn parameter53() -> impl Strategy<Value = f64> {
    any::<u64>().prop_map(|bits| (bits >> 11) as f64 * 2f64.powi(-53))
}

/// `N` arguments, all on segments, all on triangles, or each on either, whose
/// parents are points of [`full_point`] with exponents in `exponents`, and
/// whose parameters are those of [`parameter53`]: `t` for a segment, and `u`
/// and `v = w (1 - u)` for a triangle, inside it up to the rounding of `v`.
/// Their exact coordinates have many components of widely spread exponents.
pub fn wide_calls<const N: usize>(
    exponents: std::ops::RangeInclusive<i32>,
) -> impl Strategy<Value = [Pt; N]> {
    let point = move || full_point(exponents.clone());
    let line = (point(), point(), parameter53()).prop_map(|(p, q, t)| Pt::Lnc(p, q, t));
    let triangle = (point(), point(), point(), parameter53(), parameter53())
        .prop_map(|(p, q, r, u, w)| Pt::Bpt(p, q, r, u, w * (1.0 - u)));
    let (line, triangle) = (line.boxed(), triangle.boxed());
    let either = prop_oneof![line.clone(), triangle.clone()].boxed();
    prop_oneof![
        prop::collection::vec(line, N),
        prop::collection::vec(triangle, N),
        prop::collection::vec(either, N),
    ]
    .prop_map(|v| std::array::from_fn(|i| v[i]))
}

/// `N` explicit arguments whose coordinates are values of [`full_mantissa`]
/// with exponents in `exponents`.
pub fn wide_explicit_calls<const N: usize>(
    exponents: std::ops::RangeInclusive<i32>,
) -> impl Strategy<Value = [Pt; N]> {
    prop::collection::vec(full_point(exponents).prop_map(Pt::Explicit), N)
        .prop_map(|v| std::array::from_fn(|i| v[i]))
}
