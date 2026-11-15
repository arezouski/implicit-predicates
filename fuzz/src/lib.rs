//! Decoder of fuzzer input into five points and the checks that the fuzz
//! targets run on them.
//!
//! The first byte of an input chooses a [`Mode`]; the rest gives the points,
//! their indices and a permutation. Every mode produces finite points, and
//! the modes concentrate on the cases where the predicates are hardest:
//! small integer grids, where exact zeros are common, points that share
//! their parents, repeated points, parameters at the ends of `[0, 1]`,
//! coordinates at the ends of the range of `f64`, and grid points moved by a
//! few units in the last place.

use implicit_predicates::{AnyPoint, Explicit, Lnc, Point, Scratch, Sign};
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
    /// Points on a common sphere up to a few units in the last place, of
    /// any scale from 2^-32 to 2^32; implicit points have their parents on
    /// the sphere and a parameter near an end, so that they land near it.
    NearSpherical,
}

impl Mode {
    /// Every mode.
    pub const ALL: [Mode; 9] = [
        Mode::Grid,
        Mode::SharedParents,
        Mode::Repeated,
        Mode::EndParameters,
        Mode::ExtremeExponents,
        Mode::Nudged,
        Mode::NearCoplanar,
        Mode::Translated,
        Mode::NearSpherical,
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
            let (dx, dy, dz) = (unit(r)?, unit(r)?, unit(r)?);
            let norm = (dx * dx + dy * dy + dz * dz).sqrt();
            let dir = if norm > 1e-3 {
                [dx / norm, dy / norm, dz / norm]
            } else {
                [1.0, 0.0, 0.0]
            };
            let radius = 1.0 + pool[1][0].abs();
            let scale = 2f64.powi(8 * pool[2][0] as i32);
            let mut p = [0.0; 3];
            for k in 0..3 {
                let c = (pool[0][k] + radius * dir[k]) * scale;
                p[k] = nudge(c, i64::from(r.byte()? % 3));
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
        // A fraction with a denominator that is a power of two.
        _ => f64::from(b / 16) / 16.0,
    })
}

/// The point of the input: explicit or `Lnc`.
fn point(mode: Mode, pool: &[[f64; 3]; 3], r: &mut Reader<'_>) -> Option<AnyPoint> {
    let kind = r.byte()?;
    if kind % 2 == 0 {
        return Some(Explicit::new(point3(mode, pool, r)?).ok()?.into());
    }
    let (p, q) = if mode == Mode::SharedParents {
        (
            pool[usize::from(kind >> 1) % 3],
            pool[usize::from(kind >> 3) % 3],
        )
    } else {
        (point3(mode, pool, r)?, point3(mode, pool, r)?)
    };
    Some(Lnc::new(p, q, parameter(mode, r)?).ok()?.into())
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
    let none = [[0.0; 3]; 3];
    let pool = [
        point3(grid, &none, &mut r)?,
        point3(grid, &none, &mut r)?,
        point3(grid, &none, &mut r)?,
    ];
    let mut points = Vec::with_capacity(5);
    for _ in 0..5 {
        points.push(point(mode, &pool, &mut r)?);
    }
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

/// The exact coordinates of a point: for an `Lnc`, `q + t * (p - q)`.
pub fn exact_coordinates(p: &AnyPoint) -> [Dyadic; 3] {
    match p {
        AnyPoint::Explicit(e) => e.approx().map(Dyadic::from_f64),
        AnyPoint::Lnc(l) => {
            let (p, q) = (l.p().map(Dyadic::from_f64), l.q().map(Dyadic::from_f64));
            let t = Dyadic::from_f64(l.t());
            std::array::from_fn(|i| &q[i] + &(&t * &(&p[i] - &q[i])))
        }
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
            AnyPoint::Lnc(_) => None,
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

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
        }
    }
}
