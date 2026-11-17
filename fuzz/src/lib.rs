//! Decoder of fuzzer input into five points and the checks that the fuzz
//! targets run on them.
//!
//! The first byte of an input chooses a [`Mode`]; the rest gives the points,
//! their indices and a permutation. Every mode produces finite points, and
//! the modes concentrate on the cases where the predicates are hardest:
//! small integer grids, where exact zeros are common, points that share
//! their parents, repeated points, parameters at the ends of `[0, 1]`,
//! coordinates at the ends of the range of `f64`, grid points moved by a
//! few units in the last place, points on a triangle at its vertices and on
//! its edges, and right angles and right dihedral angles moved by a few
//! units in the last place.

use implicit_predicates::{AnyPoint, Bpt, Explicit, Lnc, Point, Scratch, Sign};
use implicit_predicates_corpus::exact;
use implicit_predicates_oracle::Dyadic;

/// How the points of a [`Case`] are built, chosen by the first byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Coordinates are integers from -4 to 4.
    Grid,
    /// Implicit points take their parents from a pool of three grid points.
    SharedParents,
    /// A point repeats an earlier one.
    Repeated,
    /// Implicit points have a parameter of exactly 0 or exactly 1.
    EndParameters,
    /// Coordinates of any finite magnitude, up to the ends of `f64`.
    ExtremeExponents,
    /// Grid coordinates moved by up to four units in the last place.
    Nudged,
    /// Coordinates with all their bits, on a plane up to rounding, so that
    /// the determinants are near zero and the rounding errors decide.
    NearCoplanar,
    /// A large common offset of 2^20 to 2^40 plus a small spread, so that
    /// the differences are exact but the coordinates themselves are not
    /// small.
    Translated,
    /// Points on a common sphere exactly, in three out of four inputs, and
    /// moved by up to two units in the last place otherwise, of any scale
    /// from 2^-32 to 2^32; implicit points have their parents on the sphere
    /// and a parameter at or near an end, so that they land on or near it.
    NearSpherical,
    /// Points on a triangle at one of its vertices, or just beside it; the
    /// parents are other points of the same call.
    BptVertex,
    /// Points on a triangle on one of its edges; the parents are other
    /// points of the same call.
    BptEdge,
    /// Points on a triangle, and `Lnc` points, with parents that are other
    /// points of the same call and parameters of any kind.
    SharedPoints,
    /// The first three points make a right angle at the second, or the
    /// first four a right dihedral angle at the edge `c`, `d`, on integer
    /// coordinates; then each point is moved by a few units in the last
    /// place or rewritten as an implicit point that lands on it.
    RightAngle,
}

impl Mode {
    /// Every mode.
    pub const ALL: [Mode; 13] = [
        Mode::Grid,
        Mode::SharedParents,
        Mode::Repeated,
        Mode::EndParameters,
        Mode::ExtremeExponents,
        Mode::Nudged,
        Mode::NearCoplanar,
        Mode::Translated,
        Mode::NearSpherical,
        Mode::BptVertex,
        Mode::BptEdge,
        Mode::SharedPoints,
        Mode::RightAngle,
    ];

    /// The mode that the first byte `b` chooses.
    pub fn of_byte(b: u8) -> Mode {
        Mode::ALL[usize::from(b) % Mode::ALL.len()]
    }
}

/// Five points with the indices and the permutation that go with them.
#[derive(Clone, Debug)]
pub struct Case {
    /// The mode that built the points.
    pub mode: Mode,
    /// The five points.
    pub points: [AnyPoint; 5],
    /// Five distinct indices, one per point.
    pub idx: [usize; 5],
    /// A permutation of `0..5`.
    pub order: [usize; 5],
}

/// Reads the bytes of an input in order.
struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn byte(&mut self) -> Option<u8> {
        let (&b, rest) = self.0.split_first()?;
        self.0 = rest;
        Some(b)
    }

    fn u64(&mut self) -> Option<u64> {
        let (head, rest) = self.0.split_first_chunk::<8>()?;
        self.0 = rest;
        Some(u64::from_le_bytes(*head))
    }
}

/// A finite `f64` of any magnitude, from the bits of `r`.
fn any_finite(r: &mut Reader<'_>) -> Option<f64> {
    let bits = r.u64()?;
    let x = f64::from_bits(bits);
    if x.is_finite() {
        Some(x)
    } else {
        // The exponent field is all ones: use the largest finite exponent.
        Some(f64::from_bits(bits & !(1 << 52)))
    }
}

/// A coordinate of the given mode.
fn coordinate(mode: Mode, r: &mut Reader<'_>) -> Option<f64> {
    match mode {
        Mode::ExtremeExponents => {
            if r.byte()? % 2 == 0 {
                any_finite(r)
            } else {
                // A small integer times a power of two from 2^-1000 to 2^1000.
                let m = f64::from(i32::from(r.byte()? % 9) - 4);
                let e = i32::from(u16::from_le_bytes([r.byte()?, r.byte()?]) % 2001) - 1000;
                Some(m * 2f64.powi(e))
            }
        }
        Mode::Nudged => {
            let g = f64::from(i32::from(r.byte()? % 9) - 4);
            let k = i64::from(r.byte()? % 9) - 4;
            Some(nudge(g, k))
        }
        _ => Some(f64::from(i32::from(r.byte()? % 9) - 4)),
    }
}

/// `x` moved by `k` units in the last place; zero moves to a subnormal.
fn nudge(x: f64, k: i64) -> f64 {
    let bits = x.to_bits();
    let magnitude = (bits & !(1 << 63)) as i64 + k * if x.is_sign_negative() { -1 } else { 1 };
    let magnitude = magnitude.clamp(0, f64::MAX.to_bits() as i64) as u64;
    f64::from_bits((bits & (1 << 63)) | magnitude)
}

/// A number in [-1, 1) with 52 random bits.
fn unit(r: &mut Reader<'_>) -> Option<f64> {
    Some((r.u64()? >> 11) as f64 / 2f64.powi(52) - 1.0)
}

/// A point of the mode. `pool` holds three grid points that give the
/// parameters of the geometric modes: in [`Mode::NearCoplanar`], the plane
/// `z = pool[0][0] + pool[0][1] * x + pool[0][2] * y`, rounded and then
/// moved by up to two units in the last place; in [`Mode::Translated`], the
/// offset; in [`Mode::NearSpherical`], the center and the radius.
fn point3(mode: Mode, pool: &[[f64; 3]; 3], r: &mut Reader<'_>) -> Option<[f64; 3]> {
    let plane = &pool[0];
    match mode {
        Mode::NearCoplanar => {
            let (x, y) = (unit(r)?, unit(r)?);
            let z = plane[0] + plane[1] * x + plane[2] * y;
            let k = i64::from(r.byte()? % 5) - 2;
            return Some([x, y, nudge(z, k)]);
        }
        Mode::Translated => {
            let shift = 20 + 5 * pool[1][0].abs() as i32;
            let spread = 2f64.powi(i32::from(r.byte()? % 6));
            let mut p = [0.0; 3];
            for k in 0..3 {
                let offset = (pool[0][k] + 5.0) * 2f64.powi(shift);
                p[k] = offset + spread * unit(r)?;
            }
            return Some(p);
        }
        Mode::NearSpherical => {
            // The signed permutations of the integer vector `|pool[1]|` all
            // have the same squared norm, so with the integer center
            // `pool[0]` and a power of two as a factor, which keep every
            // value exact, the points are exactly cospherical. Then, a
            // quarter of the time, each coordinate is moved by 0 to 2 ulps.
            let (perm, signs, nudges) = (r.byte()?, r.byte()?, r.byte()?);
            let v = pool[1].map(f64::abs);
            let order = [
                [0, 1, 2],
                [0, 2, 1],
                [1, 0, 2],
                [1, 2, 0],
                [2, 0, 1],
                [2, 1, 0],
            ][usize::from(perm) % 6];
            let scale = 2f64.powi(8 * pool[2][0] as i32);
            let mut p = [0.0; 3];
            for k in 0..3 {
                let sign = if signs >> k & 1 == 1 { -1.0 } else { 1.0 };
                p[k] = (pool[0][k] + sign * v[order[k]]) * scale;
                if nudges % 4 == 0 {
                    p[k] = nudge(p[k], i64::from(nudges >> (2 + 2 * k) & 3).min(2));
                }
            }
            return Some(p);
        }
        _ => {}
    }
    Some([
        coordinate(mode, r)?,
        coordinate(mode, r)?,
        coordinate(mode, r)?,
    ])
}

/// A parameter of an implicit point: dyadic fractions, a few that are not,
/// and the ends.
fn parameter(mode: Mode, r: &mut Reader<'_>) -> Option<f64> {
    let b = r.byte()?;
    if mode == Mode::EndParameters && b % 4 != 0 {
        return Some(f64::from(b % 2));
    }
    if mode == Mode::NearCoplanar && b % 2 == 0 {
        return Some((r.u64()? >> 11) as f64 / 2f64.powi(53));
    }
    if mode == Mode::NearSpherical && b % 2 == 0 {
        return Some(match b / 2 % 5 {
            0 => 0.0,
            1 => 1.0,
            2 => nudge(1.0, -1 - i64::from(b / 10 % 2)),
            3 => nudge(0.0, 1 + i64::from(b / 10 % 2)),
            _ => (r.u64()? >> 11) as f64 / 2f64.powi(53),
        });
    }
    Some(match b % 16 {
        0 => 0.0,
        1 => 1.0,
        2 => 0.1,
        3 => 1.0 / 3.0,
        4 => nudge(1.0, -1),
        5 => nudge(0.0, 1),
        6 => 0.5,
        // A short parameter, as in a mesh: k / 2^bits with up to 8 bits.
        7..=9 => {
            let bits = u32::from(r.byte()? % 8) + 1;
            let k = u32::from(r.byte()?) % ((1 << bits) + 1);
            f64::from(k) / f64::from(1u32 << bits)
        }
        // A parameter with all its 53 bits.
        10 | 11 => (r.u64()? >> 11) as f64 / 2f64.powi(53),
        // A fraction with a denominator that is a power of two.
        _ => f64::from(b / 16) / 16.0,
    })
}

/// The parameters `(u, v)` of a point on a triangle, in the range that
/// [`Bpt::new`] accepts or just beside it.
fn bpt_parameters(mode: Mode, r: &mut Reader<'_>) -> Option<(f64, f64)> {
    let u = parameter(mode, r)?;
    Some(match mode {
        Mode::BptVertex => match r.byte()? % 6 {
            0 => (1.0, 0.0),
            1 => (0.0, 1.0),
            2 => (0.0, 0.0),
            3 => (nudge(1.0, -1), 0.0),
            4 => (0.0, nudge(0.0, 1)),
            _ => (nudge(0.0, 1), 0.0),
        },
        Mode::BptEdge => match r.byte()? % 4 {
            0 => (0.0, u),
            1 => (u, 0.0),
            _ => (u, 1.0 - u),
        },
        _ => (u, (1.0 - u) * parameter(mode, r)?),
    })
}

/// The `Bpt` with these parents and parameters; when the parameters are
/// not accepted because of rounding, `v` is lowered until they are.
fn bpt(p: [f64; 3], q: [f64; 3], r: [f64; 3], u: f64, v: f64) -> Option<Bpt> {
    [v, nudge(v, -1), nudge(v, -2), 0.0]
        .into_iter()
        .find_map(|v| Bpt::new(p, q, r, u, v).ok())
}

/// A parent for a point of the modes whose parents are other points of the
/// call: the approximation of one of `earlier`, or a point of `pool`.
fn shared_parent(
    earlier: &[AnyPoint],
    pool: &[[f64; 3]; 3],
    r: &mut Reader<'_>,
) -> Option<[f64; 3]> {
    let b = usize::from(r.byte()?);
    Some(if earlier.is_empty() || b % 4 == 0 {
        pool[b / 4 % 3]
    } else {
        *earlier[b / 4 % earlier.len()].approx()
    })
}

/// The point of the input: explicit, `Lnc` or `Bpt`. `earlier` holds the
/// points built before this one.
fn point(
    mode: Mode,
    pool: &[[f64; 3]; 3],
    earlier: &[AnyPoint],
    r: &mut Reader<'_>,
) -> Option<AnyPoint> {
    let kind = r.byte()?;
    let shared = matches!(mode, Mode::BptVertex | Mode::BptEdge | Mode::SharedPoints);
    match kind % 8 {
        0..=3 => Some(Explicit::new(point3(mode, pool, r)?).ok()?.into()),
        4 | 5 => {
            let (p, q) = if mode == Mode::SharedParents {
                (
                    pool[usize::from(kind >> 3) % 3],
                    pool[usize::from(kind >> 5) % 3],
                )
            } else if mode == Mode::SharedPoints {
                (
                    shared_parent(earlier, pool, r)?,
                    shared_parent(earlier, pool, r)?,
                )
            } else {
                (point3(mode, pool, r)?, point3(mode, pool, r)?)
            };
            Some(Lnc::new(p, q, parameter(mode, r)?).ok()?.into())
        }
        _ => {
            let [p, q, s] = if mode == Mode::SharedParents {
                let b = usize::from(r.byte()?);
                [pool[b % 3], pool[b / 3 % 3], pool[b / 9 % 3]]
            } else if shared {
                [
                    shared_parent(earlier, pool, r)?,
                    shared_parent(earlier, pool, r)?,
                    shared_parent(earlier, pool, r)?,
                ]
            } else {
                [
                    point3(mode, pool, r)?,
                    point3(mode, pool, r)?,
                    point3(mode, pool, r)?,
                ]
            };
            let (u, v) = bpt_parameters(mode, r)?;
            Some(bpt(p, q, s, u, v)?.into())
        }
    }
}

/// A vector of small integers.
fn small_vector(r: &mut Reader<'_>) -> Option<[f64; 3]> {
    let mut v = [0.0; 3];
    for c in &mut v {
        *c = f64::from(i32::from(r.byte()? % 7) - 3);
    }
    Some(v)
}

fn add(a: [f64; 3], b: [f64; 3], k: f64) -> [f64; 3] {
    std::array::from_fn(|i| a[i] + k * b[i])
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// A point that is `x`, or is meant to be: explicit, or an `Lnc` or a
/// `Bpt` that lands on `x` when `x` plus or minus a small vector is exact.
fn form_around(x: [f64; 3], r: &mut Reader<'_>) -> Option<AnyPoint> {
    let form = r.byte()?;
    let w = small_vector(r)?;
    let (j1, j2) = (small_vector(r)?, small_vector(r)?);
    Some(match form % 8 {
        0..=3 => Explicit::new(x).ok()?.into(),
        4 => Lnc::new(x, j1, 1.0).ok()?.into(),
        5 => Lnc::new(add(x, w, 1.0), add(x, w, -1.0), 0.5).ok()?.into(),
        6 => Bpt::new(x, j1, j2, 1.0, 0.0).ok()?.into(),
        _ => Bpt::new(add(x, w, 1.0), add(x, w, -1.0), j1, 0.5, 0.5)
            .ok()?
            .into(),
    })
}

/// Five points of [`Mode::RightAngle`].
fn right_angle_points(r: &mut Reader<'_>) -> Option<Vec<AnyPoint>> {
    let layout = r.byte()? % 2;
    let scale = [1.0, 2f64.powi(-40), 2f64.powi(40), 2f64.powi(100)][usize::from(r.byte()? % 4)];
    let apex = small_vector(r)?;
    let (x, w) = (small_vector(r)?, small_vector(r)?);
    let spare = (
        add(apex, small_vector(r)?, 1.0),
        add(apex, small_vector(r)?, 1.0),
    );
    let points = if layout == 0 {
        // p, q, r with (p - q) . (r - q) = 0 at q = apex.
        [
            add(apex, x, 1.0),
            apex,
            add(apex, cross(x, w), 1.0),
            spare.0,
            spare.1,
        ]
    } else {
        // a, b, c, d: the edge c - d is `x`, and the directions of a and b
        // from it, `w` and `x` times `w`, are perpendicular once projected
        // along the edge.
        let (s1, s2) = (
            f64::from(r.byte()? % 5) - 2.0,
            f64::from(r.byte()? % 5) - 2.0,
        );
        let (fa, fb) = (add(w, x, s1), add(cross(x, w), x, s2));
        [
            add(apex, fa, 1.0),
            add(apex, fb, 1.0),
            add(apex, x, 1.0),
            apex,
            spare.0,
        ]
    };
    let mut out = Vec::with_capacity(5);
    for p in points {
        let pb = r.byte()?;
        let mut p = p.map(|c| c * scale);
        if pb & 3 == 0 {
            for (i, c) in p.iter_mut().enumerate() {
                *c = nudge(*c, i64::from(pb >> (2 + 2 * i) & 3) - 1);
            }
        }
        out.push(form_around(p, r)?);
    }
    Some(out)
}

/// A permutation of `0..5` from the index `n` among the 120.
fn permutation(mut n: usize) -> [usize; 5] {
    let mut rest = vec![0, 1, 2, 3, 4];
    let mut out = [0; 5];
    for (k, o) in out.iter_mut().enumerate() {
        let left = 5 - k;
        *o = rest.remove(n % left);
        n /= left;
    }
    out
}

/// The case that the bytes of `data` describe, or `None` when they are too
/// short or describe a point that the constructors reject.
pub fn decode(data: &[u8]) -> Option<Case> {
    let mut r = Reader(data);
    let mode = Mode::of_byte(r.byte()?);
    // The pool: three grid points, whose first one is also the plane of
    // `NearCoplanar`.
    let grid = if matches!(
        mode,
        Mode::NearCoplanar | Mode::Translated | Mode::NearSpherical
    ) {
        Mode::Grid
    } else {
        mode
    };
    if mode == Mode::RightAngle {
        return finish(mode, right_angle_points(&mut r)?, &mut r);
    }
    let none = [[0.0; 3]; 3];
    let pool = [
        point3(grid, &none, &mut r)?,
        point3(grid, &none, &mut r)?,
        point3(grid, &none, &mut r)?,
    ];
    let mut points = Vec::with_capacity(5);
    for _ in 0..5 {
        let next = point(mode, &pool, &points, &mut r)?;
        points.push(next);
    }
    finish(mode, points, &mut r)
}

/// The rest of the input: the repetition of [`Mode::Repeated`], the indices
/// and the permutation.
fn finish(mode: Mode, mut points: Vec<AnyPoint>, r: &mut Reader<'_>) -> Option<Case> {
    if mode == Mode::Repeated {
        let from = usize::from(r.byte()?) % 5;
        let to = usize::from(r.byte()?) % 5;
        points[to] = points[from];
    }
    let shift = u32::from(r.byte()? % 49);
    let mut idx = [0; 5];
    for (k, i) in idx.iter_mut().enumerate() {
        // Distinct because the last factor 5 separates the five points.
        *i = (usize::from(r.byte()?) * 5 + k) << shift;
    }
    let order = permutation(usize::from(r.byte()?) % 120);
    Some(Case {
        mode,
        points: points.try_into().ok()?,
        idx,
        order,
    })
}

/// The exact coordinates of a point: for an `Lnc`, `q + t * (p - q)`; for a
/// `Bpt`, `r + u * (p - r) + v * (q - r)`.
pub fn exact_coordinates(p: &AnyPoint) -> [Dyadic; 3] {
    match p {
        AnyPoint::Explicit(e) => e.approx().map(Dyadic::from_f64),
        AnyPoint::Lnc(l) => {
            let (p, q) = (l.p().map(Dyadic::from_f64), l.q().map(Dyadic::from_f64));
            let t = Dyadic::from_f64(l.t());
            std::array::from_fn(|i| &q[i] + &(&t * &(&p[i] - &q[i])))
        }
        AnyPoint::Bpt(b) => {
            let (p, q, r) = (
                b.p().map(Dyadic::from_f64),
                b.q().map(Dyadic::from_f64),
                b.r().map(Dyadic::from_f64),
            );
            let (u, v) = (Dyadic::from_f64(b.u()), Dyadic::from_f64(b.v()));
            std::array::from_fn(|i| &(&r[i] + &(&u * &(&p[i] - &r[i]))) + &(&v * &(&q[i] - &r[i])))
        }
        other => panic!("no definition of the exact coordinates of {other:?}"),
    }
}

/// A sign from the reference's integer.
pub fn sign_of(s: i8) -> Sign {
    match s {
        1 => Sign::Positive,
        0 => Sign::Zero,
        _ => Sign::Negative,
    }
}

/// The five points as explicit arrays, if all of them are explicit.
pub fn explicit_arrays(points: &[AnyPoint]) -> Option<Vec<[f64; 3]>> {
    points
        .iter()
        .map(|p| match p {
            AnyPoint::Explicit(e) => Some(*e.approx()),
            _ => None,
        })
        .collect()
}

/// A growable arena for the calls that take one.
pub fn arena() -> Scratch<'static> {
    Scratch::growable(1 << 12)
}

/// The exact sign of `orient3d` on the first four points.
pub fn exact_orient3d(points: &[AnyPoint]) -> Sign {
    let c: Vec<_> = points.iter().map(exact_coordinates).collect();
    sign_of(exact::orient3d(&c[0], &c[1], &c[2], &c[3]))
}

/// The exact sign of `insphere` on the five points.
pub fn exact_insphere(points: &[AnyPoint]) -> Sign {
    let c: Vec<_> = points.iter().map(exact_coordinates).collect();
    sign_of(exact::insphere(&c[0], &c[1], &c[2], &c[3], &c[4]))
}

/// The exact sign of `dot_sign_3d`, from its definition `(p - q) . (r - q)`.
pub fn exact_dot_sign_3d(p: &AnyPoint, q: &AnyPoint, r: &AnyPoint) -> Sign {
    let [p, q, r] = [p, q, r].map(exact_coordinates);
    sign_of(exact::dot_sign_3d(&p, &q, &r))
}

/// The exact sign of `dihedral_sign`, from its definition
/// `[(b - d) x (c - d)] . [(a - d) x (c - d)]`.
pub fn exact_dihedral_sign(a: &AnyPoint, b: &AnyPoint, c: &AnyPoint, d: &AnyPoint) -> Sign {
    let [a, b, c, d] = [a, b, c, d].map(exact_coordinates);
    sign_of(exact::dihedral_sign(&a, &b, &c, &d))
}

/// Target `orient3d`: every way of calling it agrees with the exact sign.
pub fn run_orient3d(case: &Case) {
    let [a, b, c, d, _] = &case.points;
    let want = exact_orient3d(&case.points);
    assert_eq!(implicit_predicates::orient3d(a, b, c, d), want, "{case:?}");
    let mut scratch = arena();
    let got = implicit_predicates::orient3d_in(&mut scratch, a, b, c, d);
    assert_eq!(got, Ok(want), "{case:?}");
    assert_eq!(scratch.used(), 0, "{case:?}");
    if let Some(e) = explicit_arrays(&case.points) {
        assert_eq!(
            implicit_predicates::orient3d(&e[0], &e[1], &e[2], &e[3]),
            want
        );
        let got = implicit_predicates::orient3d_in(&mut scratch, &e[0], &e[1], &e[2], &e[3]);
        assert_eq!(got, Ok(want), "{case:?}");
        assert_eq!(scratch.used(), 0, "{case:?}");
    }
}

/// Target `insphere`: every way of calling it agrees with the exact sign.
pub fn run_insphere(case: &Case) {
    let [a, b, c, d, e] = &case.points;
    let want = exact_insphere(&case.points);
    assert_eq!(
        implicit_predicates::insphere(a, b, c, d, e),
        want,
        "{case:?}"
    );
    let mut scratch = arena();
    let got = implicit_predicates::insphere_in(&mut scratch, a, b, c, d, e);
    assert_eq!(got, Ok(want), "{case:?}");
    assert_eq!(scratch.used(), 0, "{case:?}");
    if let Some(p) = explicit_arrays(&case.points) {
        assert_eq!(
            implicit_predicates::insphere(&p[0], &p[1], &p[2], &p[3], &p[4]),
            want
        );
        let got = implicit_predicates::insphere_in(&mut scratch, &p[0], &p[1], &p[2], &p[3], &p[4]);
        assert_eq!(got, Ok(want), "{case:?}");
        assert_eq!(scratch.used(), 0, "{case:?}");
    }
}

/// `dot_sign_3d` and `dot_sign_3d_in`, on the three points given.
fn dot_sign_3d_of(p: &AnyPoint, q: &AnyPoint, r: &AnyPoint) -> Sign {
    let sign = implicit_predicates::dot_sign_3d(p, q, r);
    let mut scratch = arena();
    let got = implicit_predicates::dot_sign_3d_in(&mut scratch, p, q, r);
    assert_eq!(got, Ok(sign), "{p:?} {q:?} {r:?}");
    assert_eq!(scratch.used(), 0, "{p:?} {q:?} {r:?}");
    sign
}

/// Target `dot_sign_3d`: every way of calling it agrees with the exact sign
/// on the first three points, and swapping `p` and `r` does not change it.
pub fn run_dot_sign_3d(case: &Case) {
    let [p, q, r, ..] = &case.points;
    let want = exact_dot_sign_3d(p, q, r);
    assert_eq!(dot_sign_3d_of(p, q, r), want, "{case:?}");
    assert_eq!(dot_sign_3d_of(r, q, p), want, "{case:?}");
    if let Some(e) = explicit_arrays(&case.points[..3]) {
        assert_eq!(
            implicit_predicates::dot_sign_3d(&e[0], &e[1], &e[2]),
            want,
            "{case:?}"
        );
        let mut scratch = arena();
        let got = implicit_predicates::dot_sign_3d_in(&mut scratch, &e[0], &e[1], &e[2]);
        assert_eq!(got, Ok(want), "{case:?}");
        assert_eq!(scratch.used(), 0, "{case:?}");
    }
}

/// `dihedral_sign` and `dihedral_sign_in`, on the four points given.
fn dihedral_sign_of(a: &AnyPoint, b: &AnyPoint, c: &AnyPoint, d: &AnyPoint) -> Sign {
    let sign = implicit_predicates::dihedral_sign(a, b, c, d);
    let mut scratch = arena();
    let got = implicit_predicates::dihedral_sign_in(&mut scratch, a, b, c, d);
    assert_eq!(got, Ok(sign), "{a:?} {b:?} {c:?} {d:?}");
    assert_eq!(scratch.used(), 0, "{a:?} {b:?} {c:?} {d:?}");
    sign
}

/// Target `dihedral_sign`: every way of calling it agrees with the exact
/// sign on the first four points, and swapping `a` and `b`, or `c` and `d`,
/// does not change it.
pub fn run_dihedral_sign(case: &Case) {
    let [a, b, c, d, _] = &case.points;
    let want = exact_dihedral_sign(a, b, c, d);
    assert_eq!(dihedral_sign_of(a, b, c, d), want, "{case:?}");
    assert_eq!(dihedral_sign_of(b, a, c, d), want, "{case:?}");
    assert_eq!(dihedral_sign_of(a, b, d, c), want, "{case:?}");
    if let Some(e) = explicit_arrays(&case.points[..4]) {
        assert_eq!(
            implicit_predicates::dihedral_sign(&e[0], &e[1], &e[2], &e[3]),
            want,
            "{case:?}"
        );
        let mut scratch = arena();
        let got = implicit_predicates::dihedral_sign_in(&mut scratch, &e[0], &e[1], &e[2], &e[3]);
        assert_eq!(got, Ok(want), "{case:?}");
        assert_eq!(scratch.used(), 0, "{case:?}");
    }
}

/// The sign of a permutation of `0..5`: positive when even.
pub fn parity(order: &[usize; 5]) -> Sign {
    let inversions = (0..5)
        .flat_map(|i| (i + 1..5).map(move |j| (i, j)))
        .filter(|&(i, j)| order[i] > order[j])
        .count();
    if inversions % 2 == 0 {
        Sign::Positive
    } else {
        Sign::Negative
    }
}

fn sos_of(points: &[AnyPoint; 5], idx: [usize; 5]) -> Sign {
    let [a, b, c, d, e] = points;
    let sign = implicit_predicates::insphere_sos(a, b, c, d, e, idx);
    let mut scratch = arena();
    let got = implicit_predicates::insphere_sos_in(&mut scratch, a, b, c, d, e, idx);
    assert_eq!(got, Ok(sign), "{points:?} {idx:?}");
    assert_eq!(scratch.used(), 0);
    sign
}

/// Target `insphere_sos`: it is `insphere` when that is not zero, it is
/// never zero unless the five points are coplanar, and permuting the points
/// with their indices changes its sign by the parity of the permutation.
pub fn run_insphere_sos(case: &Case) {
    let exact = exact_insphere(&case.points);
    let sos = sos_of(&case.points, case.idx);
    if exact != Sign::Zero {
        assert_eq!(sos, exact, "{case:?}");
    } else if sos == Sign::Zero {
        // A tie that stays a tie needs every four of the five coplanar.
        for skip in 0..5 {
            let four: Vec<AnyPoint> = (0..5)
                .filter(|&k| k != skip)
                .map(|k| case.points[k])
                .collect();
            assert_eq!(exact_orient3d(&four), Sign::Zero, "{case:?}");
        }
    }
    let permuted = case.order.map(|k| case.points[k]);
    let idx = case.order.map(|k| case.idx[k]);
    let again = sos_of(&permuted, idx);
    let want = if parity(&case.order) == Sign::Positive {
        sos
    } else {
        -sos
    };
    assert_eq!(again, want, "{case:?}");
}

/// Target `point_check`: each point keeps the contract of `Point`.
pub fn run_point_check(case: &Case) {
    let mut scratch = arena();
    for p in &case.points {
        assert_eq!(implicit_predicates::check(p, &mut scratch), Ok(()), "{p:?}");
        if let AnyPoint::Bpt(b) = p {
            let (pp, qq, rr, u, v) = (*b.p(), *b.q(), *b.r(), b.u(), b.v());
            // The same point built with `new` and without the checks, and
            // the unchecked points with the parameters swapped and outside
            // the range of `new`.
            // (`PartialEq` on the points is not used: an approximation can
            // be NaN.)
            let same = Bpt::new(pp, qq, rr, u, v).expect("accepted when decoded");
            assert_eq!(
                (
                    same.approx().map(f64::to_bits),
                    same.err().map(f64::to_bits)
                ),
                (b.approx().map(f64::to_bits), b.err().map(f64::to_bits)),
                "{p:?}"
            );
            for (u, v) in [(u, v), (v, u), (-u, 1.0 + v), (1.0 + u, -v)] {
                let unchecked = Bpt::new_unchecked(pp, qq, rr, u, v);
                assert_eq!(
                    implicit_predicates::check(&unchecked, &mut scratch),
                    Ok(()),
                    "{unchecked:?}"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    /// A xorshift generator: the inputs are the same on every run.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        /// A random input of random length.
        fn input(&mut self) -> Vec<u8> {
            let len = (self.next() % 400) as usize;
            (0..len).map(|_| (self.next() >> 24) as u8).collect()
        }
    }

    fn cases(n: usize) -> impl Iterator<Item = Case> {
        let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
        (0..n).filter_map(move |_| decode(&rng.input()))
    }

    fn coordinates(p: &AnyPoint) -> Vec<f64> {
        match p {
            AnyPoint::Explicit(e) => e.approx().to_vec(),
            AnyPoint::Lnc(l) => [&l.p()[..], &l.q()[..], &[l.t()]].concat(),
            AnyPoint::Bpt(b) => [&b.p()[..], &b.q()[..], &b.r()[..], &[b.u(), b.v()]].concat(),
            other => panic!("no coordinates for {other:?}"),
        }
    }

    #[test]
    fn decode_never_panics_and_is_finite() {
        let mut rng = Rng(12345);
        let mut decoded = 0;
        for _ in 0..100_000 {
            let input = rng.input();
            if let Some(case) = decode(&input) {
                decoded += 1;
                assert!(case.points.iter().flat_map(coordinates).all(f64::is_finite));
            }
        }
        // Prefixes of every length decode or not, without panicking.
        let long: Vec<u8> = (0..600).map(|_| (rng.next() >> 24) as u8).collect();
        for len in 0..long.len() {
            let _ = decode(&long[..len]);
        }
        assert!(decoded > 10_000, "{decoded}");
    }

    #[test]
    fn indices_are_distinct_and_order_is_a_permutation() {
        for case in cases(100_000) {
            assert_eq!(case.idx.iter().collect::<BTreeSet<_>>().len(), 5);
            assert_eq!(case.order.iter().collect::<BTreeSet<_>>().len(), 5);
            assert!(case.order.iter().all(|&k| k < 5));
        }
    }

    #[test]
    fn every_mode_and_feature_appears() {
        let mut modes = BTreeSet::new();
        let (mut grid, mut shared, mut repeated) = (false, false, false);
        let (mut t0, mut t1, mut tiny, mut huge, mut nudged) = (false, false, false, false, false);
        let (mut full_bits, mut offset, mut sphere_lnc, mut scales) =
            (false, false, false, BTreeSet::new());
        let mut kinds = [false; 2];
        for case in cases(100_000) {
            modes.insert(case.mode as u8);
            for (k, p) in case.points.iter().enumerate() {
                let AnyPoint::Lnc(l) = p else {
                    kinds[0] = true;
                    continue;
                };
                kinds[1] = true;
                if case.mode == Mode::NearSpherical {
                    sphere_lnc |= l.t() == 0.0 || l.t() == 1.0 || l.t() > 0.999999 || l.t() < 1e-6;
                }
                t0 |= l.t() == 0.0;
                t1 |= l.t() == 1.0;
                for other in &case.points[..k] {
                    if let AnyPoint::Lnc(m) = other {
                        shared |= case.mode == Mode::SharedParents && m.p() == l.p();
                    }
                }
            }
            for c in case.points.iter().flat_map(coordinates) {
                match case.mode {
                    Mode::Grid => grid |= c.fract() == 0.0 && c.abs() <= 4.0,
                    Mode::Nudged => nudged |= c.fract() != 0.0 && c.abs() < 5.0,
                    Mode::NearCoplanar => full_bits |= c.to_bits() & 1 == 1 && c.abs() <= 8.0,
                    Mode::Translated => offset |= c.abs() >= 2f64.powi(19),
                    _ => {}
                }
                if case.mode == Mode::NearSpherical && c != 0.0 {
                    scales.insert(c.abs().log2().floor() as i32 / 8);
                }
                tiny |= c != 0.0 && c.abs() < 1e-200;
                huge |= c.abs() > 1e200;
            }
            repeated |= case.mode == Mode::Repeated
                && (0..5).any(|i| (0..i).any(|j| case.points[i] == case.points[j]));
        }
        assert_eq!(modes.len(), Mode::ALL.len());
        assert!(grid && shared && repeated && t0 && t1 && tiny && huge && nudged && full_bits);
        assert!(offset && sphere_lnc, "{offset} {sphere_lnc}");
        assert!(scales.len() > 4, "{scales:?}");
        assert!(kinds[0] && kinds[1]);
    }

    /// Whether the five points are exactly cospherical with four of them
    /// not coplanar, which is when `insphere_sos` has a tie to break.
    fn is_proper_tie(points: &[AnyPoint; 5]) -> bool {
        exact_insphere(points) == Sign::Zero
            && (0..5).any(|skip| {
                let four: Vec<AnyPoint> =
                    (0..5).filter(|&k| k != skip).map(|k| points[k]).collect();
                exact_orient3d(&four) != Sign::Zero
            })
    }

    #[test]
    fn exact_ties_are_reached() {
        let (mut all, mut sphere, mut sphere_nonzero, mut sphere_cases) = (0, 0, 0, 0);
        for case in cases(100_000) {
            let tie = is_proper_tie(&case.points);
            all += usize::from(tie);
            if case.mode == Mode::NearSpherical {
                sphere_cases += 1;
                sphere += usize::from(tie);
                sphere_nonzero += usize::from(exact_insphere(&case.points) != Sign::Zero);
            }
        }
        println!(
            "proper ties: {all} of all modes, {sphere} of {sphere_cases} near-spherical, {sphere_nonzero} near-spherical with non-zero insphere"
        );
        assert!(sphere >= 200, "{sphere}");
        assert!(sphere_nonzero >= 200, "{sphere_nonzero}");
        assert!(all >= sphere);
    }

    #[test]
    fn short_input_is_none() {
        assert!(decode(&[]).is_none());
        assert!(decode(&[0]).is_none());
        assert!(decode(&[0; 10]).is_none());
    }

    #[test]
    fn parity_of_a_swap_is_negative() {
        assert_eq!(parity(&[0, 1, 2, 3, 4]), Sign::Positive);
        assert_eq!(parity(&[1, 0, 2, 3, 4]), Sign::Negative);
        assert_eq!(parity(&[1, 2, 0, 3, 4]), Sign::Positive);
        let all: BTreeSet<_> = (0..120).map(permutation).collect();
        assert_eq!(all.len(), 120);
    }

    /// The checks of the targets hold on the decoded inputs.
    #[test]
    fn targets_pass_on_decoded_cases() {
        for case in cases(3_000) {
            run_orient3d(&case);
            run_insphere(&case);
            run_insphere_sos(&case);
            run_point_check(&case);
            run_dot_sign_3d(&case);
            run_dihedral_sign(&case);
        }
    }

    /// The exact value of a `Big`.
    fn dyadic_of(scratch: &Scratch<'_>, b: implicit_predicates::Big) -> Dyadic {
        let words: Vec<u64> = (0..b.len()).map(|i| scratch.big_limb(b, i)).collect();
        Dyadic::from_words(b.is_negative(), &words, 64 * i64::from(b.exponent()))
    }

    /// The exact coordinates that the fuzzing oracle computes from the
    /// definition of a `Bpt` are the ones of its `exact_big`.
    #[test]
    fn bpt_oracle_agrees_with_exact_big() {
        let mut seen = 0;
        let mut modes = BTreeSet::new();
        for case in cases(20_000) {
            for p in &case.points {
                let AnyPoint::Bpt(b) = p else { continue };
                let mut scratch = Scratch::growable(1 << 12);
                let big = b.exact_big(&mut scratch).expect("finite parents");
                let got = [big.x, big.y, big.z].map(|c| dyadic_of(&scratch, c));
                let want = exact_coordinates(p);
                for k in 0..3 {
                    assert!((&got[k] - &want[k]).is_zero(), "{b:?} {k}");
                }
                seen += 1;
                modes.insert(case.mode as u8);
            }
        }
        assert!(seen > 10_000, "{seen}");
        assert!(modes.len() >= 12, "{modes:?}");
    }

    /// The oracle itself, on a point worked out by hand: the centroid of a
    /// triangle, and the vertices.
    #[test]
    fn bpt_oracle_on_a_known_point() {
        let (p, q, r) = ([4.0, 0.0, 0.0], [0.0, 4.0, 0.0], [0.0, 0.0, 4.0]);
        let mid = AnyPoint::from(Bpt::new(p, q, r, 0.25, 0.5).unwrap());
        let want = [1.0, 2.0, 1.0].map(Dyadic::from_f64);
        let got = exact_coordinates(&mid);
        assert!((0..3).all(|k| (&got[k] - &want[k]).is_zero()));
        let at_r = AnyPoint::from(Bpt::new(p, q, r, 0.0, 0.0).unwrap());
        assert!(
            exact_coordinates(&at_r)
                .iter()
                .zip(r)
                .all(|(a, b)| (a - &Dyadic::from_f64(b)).is_zero())
        );
    }

    /// Counts of the signs of a predicate over the decoded cases: among all
    /// cases, per mode of the decoder, and among the cases where some
    /// argument is a `Bpt`, whatever its parameters.
    #[derive(Default, Debug)]
    struct Signs {
        all: [usize; 3],
        with_bpt: [usize; 3],
        per_mode: BTreeMap<u8, [usize; 3]>,
    }

    impl Signs {
        fn add(&mut self, case: &Case, args: &[AnyPoint], s: Sign) {
            let k = match s {
                Sign::Negative => 0,
                Sign::Zero => 1,
                _ => 2,
            };
            self.all[k] += 1;
            self.per_mode.entry(case.mode as u8).or_default()[k] += 1;
            if args.iter().any(|p| matches!(p, AnyPoint::Bpt(_))) {
                self.with_bpt[k] += 1;
            }
        }
    }

    fn measure(n: usize, arity: usize, sign: impl Fn(&[AnyPoint]) -> Sign) -> (Signs, usize) {
        let mut signs = Signs::default();
        let mut total = 0;
        for case in cases(n) {
            total += 1;
            signs.add(&case, &case.points[..arity], sign(&case.points));
        }
        (signs, total)
    }

    fn check_fractions(name: &str, signs: &Signs, total: usize) {
        let pct = |n: usize, d: usize| 100.0 * n as f64 / d as f64;
        println!(
            "{name}: {total} cases; negative {:.2}%, zero {:.2}%, positive {:.2}%; with a Bpt: negative {}, zero {}, positive {}",
            pct(signs.all[0], total),
            pct(signs.all[1], total),
            pct(signs.all[2], total),
            signs.with_bpt[0],
            signs.with_bpt[1],
            signs.with_bpt[2],
        );
        for (m, c) in &signs.per_mode {
            println!("  mode {:?}: {:?}", Mode::ALL[usize::from(*m)], c);
        }
        for k in 0..3 {
            assert!(signs.all[k] * 100 >= total, "{name}: sign {k}: {signs:?}");
            assert!(
                signs.with_bpt[k] >= 1000,
                "{name}: sign {k} with Bpt: {signs:?}"
            );
        }
        // The zeros come from the modes aimed at them too.
        for m in [
            Mode::BptVertex,
            Mode::BptEdge,
            Mode::SharedPoints,
            Mode::RightAngle,
        ] {
            let c = signs.per_mode[&(m as u8)];
            assert!(c[1] >= 100, "{name}: {m:?}: {c:?}");
        }
    }

    #[test]
    fn dot_sign_3d_has_exact_zeros_and_both_signs() {
        let (signs, total) = measure(100_000, 3, |p| exact_dot_sign_3d(&p[0], &p[1], &p[2]));
        check_fractions("dot_sign_3d", &signs, total);
    }

    #[test]
    fn dihedral_sign_has_exact_zeros_and_both_signs() {
        let (signs, total) = measure(100_000, 4, |p| {
            exact_dihedral_sign(&p[0], &p[1], &p[2], &p[3])
        });
        check_fractions("dihedral_sign", &signs, total);
    }

    /// Points on a triangle appear at vertices and on edges, with parents
    /// that are other points of the call, with short and with full
    /// parameters, and `Lnc` points have short and full parameters too.
    #[test]
    fn points_on_a_triangle_appear_in_every_form() {
        let (mut vertex, mut edge, mut shared, mut interior) = (false, false, false, false);
        let (mut bpt_short, mut bpt_full, mut lnc_short, mut lnc_full) =
            (false, false, false, false);
        let (mut right_dot, mut right_dihedral, mut near_right) = (false, false, false);
        let short = |t: f64| (t * 256.0).fract() == 0.0;
        let full = |t: f64| t.to_bits() & 0xfffff != 0 && t > 0.0 && t < 1.0;
        for case in cases(100_000) {
            if case.mode == Mode::RightAngle {
                let [p, q, r, ..] = &case.points;
                let dot = exact_dot_sign_3d(p, q, r);
                right_dot |= dot == Sign::Zero && case.points.iter().all(|p| !p.is_explicit());
                let [a, b, c, d, _] = &case.points;
                let dih = exact_dihedral_sign(a, b, c, d);
                right_dihedral |= dih == Sign::Zero && !a.is_explicit();
                near_right |= dot != Sign::Zero || dih != Sign::Zero;
            }
            for (k, p) in case.points.iter().enumerate() {
                match p {
                    AnyPoint::Bpt(b) => {
                        let (u, v) = (b.u(), b.v());
                        vertex |=
                            (u, v) == (1.0, 0.0) || (u, v) == (0.0, 1.0) || (u, v) == (0.0, 0.0);
                        edge |= case.mode == Mode::BptEdge && u > 0.0 && v > 0.0 && u + v == 1.0;
                        interior |= u > 0.0 && v > 0.0 && u + v < 1.0;
                        bpt_short |= u > 0.0 && u < 1.0 && short(u);
                        bpt_full |= full(u) || full(v);
                        shared |= case.mode == Mode::SharedPoints
                            && case.points[..k].iter().any(|o| o.approx() == b.p());
                    }
                    AnyPoint::Lnc(l) => {
                        lnc_short |= l.t() > 0.0 && l.t() < 1.0 && short(l.t());
                        lnc_full |= full(l.t());
                    }
                    _ => {}
                }
            }
        }
        assert!(
            vertex && edge && shared && interior,
            "{vertex} {edge} {shared} {interior}"
        );
        assert!(bpt_short && bpt_full && lnc_short && lnc_full);
        assert!(right_dot && right_dihedral && near_right);
    }
}
