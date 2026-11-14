//! The second test of the filter of `insphere`, `filter_axes`, against the
//! independent oracle, `implicit_predicates_corpus::exact::insphere`, and the
//! exact value of the determinant on `Dyadic`: its error bound covers the
//! error of the computed value, and it never gives a sign other than the
//! oracle's, on random, grid, nearly cospherical and anisotropic points, with
//! explicit points, points on a line and points known within a box; it
//! decides a call that the filter does not; its certain zero and its guards
//! on the uncertainty and on the inputs; the ends of its range on each axis,
//! reached by the coordinates and by the uncertainty; the order of the chain;
//! and that `filter_axes_points` takes the errors of the points into
//! account.
//!
//! The value in `f64` is computed by running the statements of the emitted
//! `filter_axes` itself, read from its source, so it is the value that the
//! stage compares with its bound; the bound is evaluated exactly from
//! `AXES_EPSILON` and `AXES_KAPPA`, with the exact sizes `D_k = delta_k +
//! E_k` of each call, where `delta_k` is the largest magnitude of the
//! computed leaves of axis `k` and `E_k` the exact sum of the two largest
//! errors of the points on that axis.

mod common;

use common::args::{Arg, any_points};
use common::insphere::{Points5, oracle, oracle_at, refs};
use common::pow2;
use implicit_predicates::stages::insphere::{
    self as stages, AXES_D_MAX, AXES_D_MIN, AXES_EPSILON, AXES_KAPPA,
};
use implicit_predicates::{AnyPoint, Explicit, Lnc, Point, Scratch, Sign};
use implicit_predicates_core::filter::uncertainty;
use implicit_predicates_oracle::Dyadic;
use implicit_predicates_test_support::{
    config, cospherical5, grid5, lnc_params, near_cospherical5, random5, sample,
};
use proptest::prelude::*;

const SOURCE: &str = include_str!("../src/generated/insphere.rs");

/// One statement of the emitted `filter_axes`, up to `det`.
#[derive(Clone, Copy, Debug)]
enum Op {
    /// `p[minuend][axis] - p[subtrahend][axis]`.
    Leaf {
        minuend: usize,
        subtrahend: usize,
        axis: usize,
    },
    Neg(usize),
    Add(usize, usize),
    Sub(usize, usize),
    Mul(usize, usize),
}

/// The statements of the emitted `filter_axes` that compute `det`, in their
/// order, read from its source: the last is `det`.
fn program() -> Vec<Op> {
    let start = SOURCE
        .find("pub fn filter_axes(")
        .expect("filter_axes is emitted");
    let mut ops = Vec::new();
    for line in SOURCE[start..].lines().skip(1) {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("let ") else {
            panic!("an unexpected statement in filter_axes: {line}");
        };
        let (name, rhs) = rest.split_once(" = ").expect("an assignment");
        let rhs = rhs.strip_suffix(';').expect("a statement");
        let temp = |t: &str| -> usize {
            let n: usize = t.strip_prefix('t').expect("a temporary").parse().unwrap();
            assert!(n < ops.len(), "{t} before its definition");
            n
        };
        let leaf = |x: &str| -> Option<(usize, usize)> {
            let x = x.strip_prefix("p[")?;
            let (i, x) = x.split_once("][")?;
            let k = x.strip_suffix(']')?;
            Some((i.parse().ok()?, k.parse().ok()?))
        };
        let op = if let Some(x) = rhs.strip_prefix('-') {
            Op::Neg(temp(x))
        } else {
            let [a, o, b] = rhs.split(' ').collect::<Vec<_>>()[..] else {
                panic!("{line}");
            };
            match (leaf(a), leaf(b)) {
                (Some((i, k)), Some((j, l))) => {
                    assert_eq!((o, k), ("-", l), "{line}");
                    Op::Leaf {
                        minuend: i,
                        subtrahend: j,
                        axis: k,
                    }
                }
                _ => {
                    let (a, b) = (temp(a), temp(b));
                    match o {
                        "+" => Op::Add(a, b),
                        "-" => Op::Sub(a, b),
                        "*" => Op::Mul(a, b),
                        _ => panic!("{line}"),
                    }
                }
            }
        };
        if name == "det" {
            ops.push(op);
            return ops;
        }
        assert_eq!(name, format!("t{}", ops.len()), "{line}");
        ops.push(op);
    }
    panic!("no det in filter_axes")
}

/// The value of `det` in `f64` at `p` as `filter_axes` computes it, and the
/// largest magnitude of the computed leaves of each axis.
fn computed(ops: &[Op], p: &Points5) -> (f64, [f64; 3]) {
    let mut v: Vec<f64> = Vec::with_capacity(ops.len());
    let mut delta = [0.0f64; 3];
    for &op in ops {
        let x = match op {
            Op::Leaf {
                minuend,
                subtrahend,
                axis,
            } => {
                let x = p[minuend][axis] - p[subtrahend][axis];
                delta[axis] = delta[axis].max(x.abs());
                x
            }
            Op::Neg(a) => -v[a],
            Op::Add(a, b) => v[a] + v[b],
            Op::Sub(a, b) => v[a] - v[b],
            Op::Mul(a, b) => v[a] * v[b],
        };
        v.push(x);
    }
    (*v.last().unwrap(), delta)
}

/// The exact value of the in-sphere determinant at the points with the
/// exact coordinates `c`, by a cofactor expansion along the lifted column;
/// its sign is checked against the oracle wherever it is used.
fn value_at(c: &[[Dyadic; 3]; 5]) -> Dyadic {
    let rows: [[Dyadic; 3]; 4] =
        std::array::from_fn(|i| std::array::from_fn(|k| &c[i][k] - &c[4][k]));
    let lifted = |r: &[Dyadic; 3]| &(&(&r[0] * &r[0]) + &(&r[1] * &r[1])) + &(&r[2] * &r[2]);
    let det3 = |r: [&[Dyadic; 3]; 3]| {
        let minor = |j: usize, k: usize| &(&r[1][j] * &r[2][k]) - &(&r[1][k] * &r[2][j]);
        &(&(&r[0][0] * &minor(1, 2)) - &(&r[0][1] * &minor(0, 2))) + &(&r[0][2] * &minor(0, 1))
    };
    let mut det = Dyadic::from_f64(0.0);
    for i in 0..4 {
        let others: Vec<&[Dyadic; 3]> = (0..4).filter(|&j| j != i).map(|j| &rows[j]).collect();
        let term = &lifted(&rows[i]) * &det3([others[0], others[1], others[2]]);
        det = if i % 2 == 1 {
            &det + &term
        } else {
            &det - &term
        };
    }
    det
}

fn abs(x: &Dyadic) -> Dyadic {
    if x.sign() < 0 { -x } else { x.clone() }
}

/// The exact bound by parts with the emitted constants, at the sizes `d` and
/// the uncertainties `e`.
fn bound_at(d: &[Dyadic; 3], e: &[Dyadic; 3]) -> Dyadic {
    let power = |start: Dyadic, g: [u32; 3]| {
        let mut x = start;
        for k in 0..3 {
            for _ in 0..g[k] {
                x = &x * &d[k];
            }
        }
        x
    };
    let mut bound = Dyadic::from_f64(0.0);
    for (g, c) in AXES_EPSILON {
        bound = &bound + &power(Dyadic::from_f64(c), g);
    }
    for (k, h, c) in AXES_KAPPA {
        bound = &bound + &power(&Dyadic::from_f64(c) * &e[k], h);
    }
    bound
}

/// `num / den`, both nonnegative and `den` positive, to about six digits,
/// or zero below `2^-1000`.
fn ratio(num: &Dyadic, den: &Dyadic) -> f64 {
    if num.is_zero() {
        return 0.0;
    }
    let scaled = |e: i32| {
        let mut x = den.clone();
        let mut left = e;
        while left != 0 {
            let step = left.clamp(-1000, 1000);
            x = &x * &Dyadic::from_f64(pow2(step));
            left -= step;
        }
        x
    };
    // The exponent of the ratio: `den 2^e <= num < den 2^(e + 1)`.
    let mut e = 0;
    while *num < scaled(e) {
        e -= 1;
        if e < -1000 {
            return 0.0;
        }
    }
    while *num >= scaled(e + 1) {
        e += 1;
    }
    // Twenty bits of the mantissa, by bisection.
    let base = scaled(e - 20);
    let (mut low, mut high) = (1u32 << 20, 1u32 << 21);
    while high - low > 1 {
        let middle = (low + high) / 2;
        if *num >= &base * &Dyadic::from_f64(f64::from(middle)) {
            low = middle;
        } else {
            high = middle;
        }
    }
    f64::from(low) * pow2(e - 20)
}

/// A call of the properties: the approximations of the points, their exact
/// coordinates, the error bound of each point on each axis, as the points
/// give it, and the points themselves when some is on a line.
#[derive(Clone, Debug)]
struct Case {
    family: &'static str,
    approx: Points5,
    exact: [[Dyadic; 3]; 5],
    err: [[f64; 3]; 5],
    points: Option<[AnyPoint; 5]>,
}

impl Case {
    fn explicit(family: &'static str, p: Points5) -> Case {
        Case {
            family,
            approx: p,
            exact: p.map(|q| q.map(Dyadic::from_f64)),
            err: [[0.0; 3]; 5],
            points: None,
        }
    }

    fn lines(family: &'static str, args: [Arg; 5]) -> Case {
        let points = any_points(&args);
        Case {
            family,
            approx: points.each_ref().map(|q| *q.approx()),
            exact: args.each_ref().map(Arg::exact),
            err: points.each_ref().map(|q| *q.err()),
            points: Some(points),
        }
    }

    /// The uncertainty given to the stage: that of the error bounds.
    fn e(&self) -> [f64; 3] {
        uncertainty(self.err.each_ref())
    }

    /// The exact sum of the two largest actual errors on each axis.
    fn actual_e(&self) -> [Dyadic; 3] {
        std::array::from_fn(|k| {
            let mut errors: Vec<Dyadic> = (0..5)
                .map(|i| abs(&(&Dyadic::from_f64(self.approx[i][k]) - &self.exact[i][k])))
                .collect();
            errors.sort();
            &errors[4] + &errors[3]
        })
    }
}

/// `x` moved by `n` units in the last place, up for `n > 0`.
fn stepped(x: f64, n: i32) -> f64 {
    let step = if n >= 0 { f64::next_up } else { f64::next_down };
    (0..n.unsigned_abs()).fold(x, |y, _| step(y))
}

/// The position of `x` in the order of the finite `f64`s.
fn key(x: f64) -> i128 {
    let magnitude = i128::from(x.to_bits() & !(1 << 63));
    if x.is_sign_negative() {
        -magnitude
    } else {
        magnitude
    }
}

fn from_key(k: i128) -> f64 {
    let bits = u64::try_from(k.unsigned_abs()).unwrap();
    f64::from_bits(if k < 0 { bits | 1 << 63 } else { bits })
}

fn sign_at(p: &Points5) -> i8 {
    value_at(&p.map(|q| q.map(Dyadic::from_f64))).sign()
}

/// `p` with coordinate `axis` of point `point` moved to where the exact value
/// changes sign, when it does within twice the largest coordinate of that
/// axis of `p`, and then `steps` units in the last place away from there;
/// `p` itself otherwise. Such points are nearly degenerate, whatever their
/// shape.
fn to_the_edge(mut p: Points5, point: usize, axis: usize, steps: i32) -> Points5 {
    let c = p[point][axis];
    let reach = 2.0 * p.iter().fold(0.0f64, |m, q| m.max(q[axis].abs()));
    if reach == 0.0 || !reach.is_finite() {
        return p;
    }
    let at = |p: &Points5, x: f64| {
        let mut q = *p;
        q[point][axis] = x;
        sign_at(&q)
    };
    let here = at(&p, c);
    if here == 0 {
        return p;
    }
    let (mut same, mut other) = match [c - reach, c + reach]
        .into_iter()
        .find(|&x| x.is_finite() && at(&p, x) != here)
    {
        Some(x) => (key(c), key(x)),
        None => return p,
    };
    while (other - same).abs() > 1 {
        let middle = same + (other - same) / 2;
        if at(&p, from_key(middle)) == here {
            same = middle;
        } else {
            other = middle;
        }
    }
    let away = if other > same { -steps } else { steps };
    p[point][axis] = stepped(from_key(same), away);
    p
}

/// `p` with axis `axis` scaled by `2^-shift`, exactly unless it underflows.
fn squeezed(mut p: Points5, axis: usize, shift: i32) -> Points5 {
    for q in &mut p {
        q[axis] *= pow2(-shift);
    }
    p
}

/// A simple deterministic sequence for the choices that are not points.
struct Choices(u64);

impl Choices {
    fn below(&mut self, n: u64) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        (z ^ (z >> 31)) % n
    }
}

/// Base points: random, on a grid, cospherical and nearly cospherical; half
/// of them anisotropic, with one axis `2^-20` to `2^-60` times the others;
/// and most of them then moved to the edge of the zero of the value.
fn base(count: usize, seed: u64) -> Vec<(&'static str, Points5)> {
    let points = prop_oneof![random5(), grid5(), cospherical5(), near_cospherical5()];
    let mut choices = Choices(seed);
    sample(points, count)
        .into_iter()
        .map(|p| {
            let (name, p) = if choices.below(2) == 0 {
                ("isotropic", p)
            } else {
                let axis = choices.below(3) as usize;
                let shift = 20 + choices.below(41) as i32;
                ("anisotropic", squeezed(p, axis, shift))
            };
            if choices.below(4) == 0 {
                return (name, p);
            }
            let point = choices.below(5) as usize;
            let axis = choices.below(3) as usize;
            let steps = choices.below(4) as i32;
            (name, to_the_edge(p, point, axis, steps))
        })
        .collect()
}

/// The calls of the properties, from [`base`]: explicit; with one to five
/// points replaced by points on a line, each with an exact value at the
/// point (parents both the point), near it (parents a few units in the last
/// place apart), at it within the rounding of far parents (the midpoint of
/// two parents `2^j` times as far, whose error is large), or anywhere (from
/// `lnc_params`); and known within a box, each coordinate of the exact
/// points at a corner of a box of a power of two around the approximation,
/// the corner that moves the value furthest each way to first order.
fn cases(count: usize) -> Vec<Case> {
    let lines = sample(prop::collection::vec(lnc_params(), 5), count);
    let mut choices = Choices(0x5eed_a1e5);
    let mut out = Vec::new();
    for ((name, p), any_lines) in base(count, 0x0a11_a1e5).into_iter().zip(lines) {
        out.push(Case::explicit(name, p));
        // One to five points on a line, at random positions.
        let n = 1 + choices.below(5) as usize;
        let mut at: Vec<usize> = (0..5).collect();
        for i in 0..5 {
            at.swap(i, i + choices.below((5 - i) as u64) as usize);
        }
        let mut args = p.map(Arg::Explicit);
        for &i in &at[..n] {
            let x = p[i];
            args[i] = match choices.below(4) {
                0 => Arg::Lnc(x, x, 0.375),
                1 => {
                    let mut s = || choices.below(7) as i32 - 3;
                    let y = [stepped(x[0], s()), stepped(x[1], s()), stepped(x[2], s())];
                    Arg::Lnc(x, y, 0.625)
                }
                2 => {
                    let size = x.iter().fold(pow2(-60), |m, v| m.max(v.abs()));
                    let far = size * pow2(1 + choices.below(30) as i32);
                    let w: [f64; 3] = std::array::from_fn(|_| choices.below(3) as f64 - 1.0);
                    let a = std::array::from_fn(|k| x[k] + far * w[k]);
                    let b = std::array::from_fn(|k| x[k] - far * w[k]);
                    Arg::Lnc(a, b, 0.5)
                }
                _ => {
                    let (a, b, t) = any_lines[i];
                    Arg::Lnc(a, b, t)
                }
            };
        }
        if args.iter().all(Arg::is_finite) {
            let case = Case::lines(name, args);
            // Points on a line whose approximation overflows are left out.
            if case
                .approx
                .iter()
                .chain(&case.err)
                .flatten()
                .all(|x| x.is_finite())
            {
                out.push(case);
            }
        }
        // Within boxes, of `2^-m` times the size of each axis.
        let size: [f64; 3] =
            std::array::from_fn(|k| p.iter().fold(0.0f64, |s, q| s.max(q[k].abs())));
        if size.iter().any(|&s| s == 0.0 || s < pow2(-900)) {
            continue;
        }
        let m = 8 + choices.below(53) as i32;
        let exponent = |s: f64| s.log2().floor() as i32;
        let mut err = [[0.0; 3]; 5];
        for k in 0..3 {
            // The error on every point, or on two of them only.
            let two = (choices.below(5) as usize, choices.below(5) as usize);
            let all = choices.below(4) == 0;
            for (i, row) in err.iter_mut().enumerate() {
                if all || i == two.0 || i == two.1 {
                    row[k] = pow2((exponent(size[k]) - m).max(-1000));
                }
            }
        }
        for up in [true, false] {
            out.push(boxed(name, p, err, up));
        }
    }
    out
}

/// The points within `err` of `p` at the corner that moves the value
/// furthest up, or down, to first order: each coordinate pushed by the sign
/// of the exact central difference of the value across its box.
fn boxed(family: &'static str, p: Points5, err: [[f64; 3]; 5], up: bool) -> Case {
    let centre = p.map(|q| q.map(Dyadic::from_f64));
    let mut exact = centre.clone();
    for i in 0..5 {
        for k in 0..3 {
            let e = Dyadic::from_f64(err[i][k]);
            let (mut above, mut under) = (centre.clone(), centre.clone());
            above[i][k] = &centre[i][k] + &e;
            under[i][k] = &centre[i][k] - &e;
            let slope = (&value_at(&above) - &value_at(&under)).sign();
            exact[i][k] = if (slope >= 0) == up {
                &centre[i][k] + &e
            } else {
                &centre[i][k] - &e
            };
        }
    }
    Case {
        family,
        approx: p,
        exact,
        err,
        points: None,
    }
}

/// What the properties found in one family.
#[derive(Default)]
struct Found {
    cases: usize,
    out_of_range: usize,
    decided: usize,
    only_axes: usize,
    near: usize,
    far: usize,
    worst: f64,
}

/// The two properties, on every call of [`cases`]:
///
/// - the bound: `|value_f64 - value_exact|` is at most the exact bound by
///   parts with the emitted constants, at `D_k = delta_k + E_k` with the
///   exact `E_k` of the call, whenever every `D_k` is in the range
///   `AXES_D_MIN..=AXES_D_MAX`;
/// - the sign: when `filter_axes`, with the uncertainty of the error bounds
///   of the points, or `filter_axes_points`, on the points themselves,
///   decides, its sign is the sign of the oracle at the exact points.
///
/// It prints, for each family, how close the error came to the bound: the
/// largest ratio of the error to the bound, and how many calls came within
/// a factor of 32 and of 64 of it. Most calls are near the edge of the zero
/// of the value, and the boxes move the value by the most they can to first
/// order, so the error reaches a fair fraction of the terms of kappa.
#[test]
fn the_bound_by_parts_covers_the_error_and_the_sign_is_that_of_the_oracle() {
    let ops = program();
    let count = (config().cases as usize / 2).max(50);
    let mut found: Vec<(String, Found)> = Vec::new();
    for case in cases(count) {
        let kind = match (&case.points, case.err.iter().flatten().any(|&x| x > 0.0)) {
            (Some(_), _) => "lines",
            (None, true) => "boxes",
            (None, false) => "explicit",
        };
        let family = format!("{kind}, {}", case.family);
        let index = match found.iter().position(|(f, _)| *f == family) {
            Some(i) => i,
            None => {
                found.push((family.clone(), Found::default()));
                found.len() - 1
            }
        };
        let f = &mut found[index].1;
        f.cases += 1;
        let exact = value_at(&case.exact);
        let truth = oracle_at(&case.exact);
        assert_eq!(i8::from(truth), exact.sign(), "{case:?}");
        // The sign.
        let e = case.e();
        let decision = stages::filter_axes(refs(&case.approx), e);
        if let Some(sign) = decision {
            assert_eq!(sign, truth, "filter_axes {case:?} e = {e:?}");
            f.decided += 1;
            let global = stages::filter(refs(&case.approx), e.iter().fold(0.0, |m, &x| m + x));
            f.only_axes += usize::from(global.is_none());
        }
        if let Some(points) = &case.points {
            let [a, b, c, d, q] = points;
            let sign = stages::filter_axes_points(a, b, c, d, q);
            assert_eq!(sign, decision, "{case:?}");
        }
        // The bound.
        let (value, delta) = computed(&ops, &case.approx);
        if let Some(sign) = decision {
            if sign != Sign::Zero {
                assert_eq!(value > 0.0, sign == Sign::Positive, "{case:?}");
            }
        }
        let actual = case.actual_e();
        let d: [Dyadic; 3] = std::array::from_fn(|k| &Dyadic::from_f64(delta[k]) + &actual[k]);
        let (low, high) = (Dyadic::from_f64(AXES_D_MIN), Dyadic::from_f64(AXES_D_MAX));
        if !value.is_finite() || d.iter().any(|x| *x < low || *x > high) {
            f.out_of_range += 1;
            continue;
        }
        let bound = bound_at(&d, &actual);
        let error = abs(&(&Dyadic::from_f64(value) - &exact));
        assert!(error <= bound, "{case:?}: the error exceeds the bound");
        let times = |n: f64| &error * &Dyadic::from_f64(n) >= bound;
        f.near += usize::from(times(32.0));
        f.far += usize::from(times(64.0));
        if times(1.0 / f.worst.max(1e-300)) || f.worst == 0.0 {
            f.worst = f.worst.max(ratio(&error, &bound));
        }
    }
    let mut total = Found::default();
    for (family, f) in &found {
        println!(
            "{family}: {} calls, {} outside the range, {} decided, {} of them that the filter \
             does not decide; error over bound at most {:.4}, {} within 32 times, {} within 64 \
             times",
            f.cases, f.out_of_range, f.decided, f.only_axes, f.worst, f.near, f.far
        );
        total.cases += f.cases;
        total.decided += f.decided;
        total.only_axes += f.only_axes;
        total.near += f.near;
        total.worst = total.worst.max(f.worst);
    }
    assert_eq!(found.len(), 6, "every family");
    assert!(total.decided > 0 && total.decided < total.cases);
    assert!(
        total.only_axes > 0,
        "filter_axes never decides beyond the filter"
    );
    // The properties must reach the bound, not stay far below it.
    // The largest measured, about 0.047, is in the boxes: the terms of
    // kappa add up the largest coefficient of every term, which a single
    // configuration never reaches together.
    assert!(total.worst > 0.02, "{}", total.worst);
    assert!(total.near > 0);
}

/// The points of the axes, `[1, 0, 0]`, `[0, 1, 0]`, `[0, 0, h]`,
/// `[-1, 0, 0]` and the origin, with `h = 2^-50`: the value, `2 h`, is below
/// the bound of the filter, of order `u` with one size for every axis, and
/// far above the bound by parts, of order `u h`.
const FLAT: Points5 = [
    [1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, 0.0, 8.881784197001252e-16],
    [-1.0, 0.0, 0.0],
    [0.0, 0.0, 0.0],
];

#[test]
fn filter_axes_decides_a_flat_call_that_the_filter_does_not() {
    assert_eq!(FLAT[2][2], pow2(-50));
    let expected = oracle(&FLAT);
    assert_ne!(expected, Sign::Zero);
    assert_eq!(stages::filter(refs(&FLAT), 0.0), None);
    assert_eq!(stages::filter_axes(refs(&FLAT), [0.0; 3]), Some(expected));
    // The same on every axis, and with the points scaled.
    for k in 0..3 {
        for scale in [pow2(-100), 1.0, pow2(100)] {
            let p = FLAT.map(|q| std::array::from_fn(|j| q[(j + 3 - k) % 3] * scale));
            let expected = oracle(&p);
            assert_eq!(stages::filter(refs(&p), 0.0), None, "{k} {scale:e}");
            assert_eq!(
                stages::filter_axes(refs(&p), [0.0; 3]),
                Some(expected),
                "{k} {scale:e}"
            );
        }
    }
}

/// The chain on [`FLAT`], which the filter does not decide and
/// `filter_axes` does: `staged`, `exact_stages_explicit_in`,
/// `exact_stages_in` and `staged_in` give its sign without touching the
/// arena, whose high water stays at zero, with the points as arrays, as
/// `Explicit` and as points on a line whose parents are both the point.
#[test]
fn the_chain_runs_filter_axes_before_the_big_integers() {
    let expected = oracle(&FLAT);
    assert_eq!(stages::staged(refs(&FLAT)), Some(expected));
    let mut buf = vec![0.0; 1 << 12];
    let mut scratch = Scratch::borrowed(&mut buf);
    assert_eq!(
        stages::exact_stages_explicit_in(&mut scratch, refs(&FLAT)),
        Ok(expected)
    );
    let [a, b, c, d, e] = &FLAT;
    assert_eq!(
        stages::exact_stages_in(&mut scratch, a, b, c, d, e),
        Ok(expected)
    );
    assert_eq!(stages::staged_in(&mut scratch, a, b, c, d, e), Ok(expected));
    let [a, b, c, d, e] = FLAT.map(Explicit::new_unchecked);
    assert_eq!(
        stages::exact_stages_in(&mut scratch, &a, &b, &c, &d, &e),
        Ok(expected)
    );
    assert_eq!(
        stages::staged_in(&mut scratch, &a, &b, &c, &d, &e),
        Ok(expected)
    );
    let lines = FLAT.map(|x| Lnc::new_unchecked(x, x, 0.375));
    assert!(lines.iter().all(|l| *l.err() == [0.0; 3]));
    let [a, b, c, d, e] = &lines;
    assert_eq!(stages::filter_points(a, b, c, d, e), None);
    assert_eq!(
        stages::exact_stages_in(&mut scratch, a, b, c, d, e),
        Ok(expected)
    );
    assert_eq!(stages::staged_in(&mut scratch, a, b, c, d, e), Ok(expected));
    assert_eq!(scratch.high_water(), 0, "the arena was used");
}

/// Five points with the coordinate `0.75` on axis `k`, and the others on a
/// sphere, nearly: a flat axis, whose value is exactly zero.
fn flat_on(k: usize) -> Points5 {
    let rows = [
        [1.0, 0.0],
        [0.0, 1.0],
        [-1.0, 0.0],
        [0.0, -1.0],
        [0.25, 0.5],
    ];
    rows.map(|r| {
        let mut q = [0.75; 3];
        q[(k + 1) % 3] = r[0];
        q[(k + 2) % 3] = r[1];
        q
    })
}

/// The certain zero: on a flat axis with no uncertainty on it, the value is
/// exactly zero, whatever the uncertainty on the other two axes; with any
/// uncertainty on the flat axis, from the least subnormal up, or an
/// uncertainty that is negative, infinite or NaN on any axis, the stage
/// gives no zero and no other sign.
#[test]
fn filter_axes_gives_a_certain_zero_only_on_a_flat_axis_without_uncertainty() {
    let bad = [
        -pow2(-1074),
        -1.0,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
    ];
    for k in 0..3 {
        let p = flat_on(k);
        assert_eq!(oracle(&p), Sign::Zero);
        assert_eq!(stages::filter(refs(&p), 0.0), None);
        assert_eq!(stages::filter_axes(refs(&p), [0.0; 3]), Some(Sign::Zero));
        let mut others = [pow2(-10); 3];
        others[k] = 0.0;
        assert_eq!(
            stages::filter_axes(refs(&p), others),
            Some(Sign::Zero),
            "{k}"
        );
        for on_axis in [pow2(-1074), pow2(-1022), pow2(-60), pow2(-10), 1.0] {
            let mut e = [0.0; 3];
            e[k] = on_axis;
            assert_eq!(stages::filter_axes(refs(&p), e), None, "{k} {e:?}");
        }
        for j in 0..3 {
            for x in bad {
                let mut e = [0.0; 3];
                e[j] = x;
                assert_eq!(stages::filter_axes(refs(&p), e), None, "{k} {e:?}");
            }
        }
        // A coordinate that is not finite, at every position and on every
        // axis, the flat one included.
        for i in 0..5 {
            for j in 0..3 {
                for x in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                    let mut q = p;
                    q[i][j] = x;
                    assert_eq!(stages::filter_axes(refs(&q), [0.0; 3]), None, "{i} {j}");
                }
            }
        }
    }
}

/// On a call that `filter_axes` decides, an uncertainty that is negative,
/// infinite or NaN on any axis, or a coordinate that is not finite at any
/// position and on any axis, makes it decline.
#[test]
fn filter_axes_declines_an_invalid_uncertainty_or_input() {
    let expected = oracle(&FLAT);
    assert_eq!(stages::filter_axes(refs(&FLAT), [0.0; 3]), Some(expected));
    for k in 0..3 {
        for x in [
            -pow2(-1074),
            -1.0,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
        ] {
            let mut e = [0.0; 3];
            e[k] = x;
            assert_eq!(stages::filter_axes(refs(&FLAT), e), None, "{e:?}");
        }
    }
    for i in 0..5 {
        for j in 0..3 {
            for x in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                let mut q = FLAT;
                q[i][j] = x;
                assert_eq!(stages::filter_axes(refs(&q), [0.0; 3]), None, "{i} {j}");
            }
        }
    }
}

/// Points of the axes and the origin whose leaves of axis `k` have the
/// largest magnitude `s` and those of the other two 1: with two points on
/// axis `k`, on either side, so that the value grows as `s^3`, or with one,
/// so that it shrinks as `s`, as the bound does at each end of the range.
fn sized(k: usize, s: f64, two: bool) -> Points5 {
    let (j, l) = if two {
        (k, (k + 1) % 3)
    } else {
        ((k + 1) % 3, k)
    };
    let mut p = [[0.0; 3]; 5];
    p[0][j] = if j == k { s } else { 1.0 };
    p[1][l] = if l == k { s } else { 1.0 };
    p[2][(k + 2) % 3] = 1.0;
    p[3][j] = -p[0][j];
    p
}

/// The ends of the range on each axis, with the other two of size 1: `d_k`
/// at `AXES_D_MIN`, at its predecessor, at `AXES_D_MAX` and at its
/// successor, reached by the leaves alone with no uncertainty, and by half
/// the size in the leaves and the rest in `e[k]`. Outside the range the
/// stage declines; inside, with no uncertainty, it decides with the sign of
/// the oracle, and with an uncertainty it may decline, but any sign it gives
/// is the oracle's at the approximations, which are within the error.
#[test]
fn the_ends_of_the_range_of_filter_axes_on_each_axis() {
    // The ends of the derivation, `2^-195` and `2^203`, not read from the
    // constants, so that a constant moved away from them fails here too.
    let (low, high) = (pow2(-195), pow2(203));
    let ends = [
        (low, true),
        (low.next_down(), false),
        (high, true),
        (high.next_up(), false),
    ];
    for k in 0..3 {
        for (target, inside) in ends {
            let two = target > 1.0;
            let p = sized(k, target, two);
            let expected = oracle(&p);
            assert_ne!(expected, Sign::Zero);
            let decision = stages::filter_axes(refs(&p), [0.0; 3]);
            assert_eq!(decision, inside.then_some(expected), "{k} {target:e}");
            // Half of it from the leaves, and the rest from `e[k]`, exactly.
            let half = if target > 1.0 { high / 2.0 } else { low / 2.0 };
            let p = sized(k, half, two);
            let rest = target - half;
            assert_eq!(half + rest, target);
            assert_eq!(
                &Dyadic::from_f64(half) + &Dyadic::from_f64(rest),
                Dyadic::from_f64(target)
            );
            let mut e = [0.0; 3];
            e[k] = rest;
            let decision = stages::filter_axes(refs(&p), e);
            if inside {
                assert!(decision.is_none_or(|s| s == oracle(&p)), "{k} {target:e}");
            } else {
                assert_eq!(decision, None, "{k} {target:e} with e");
            }
        }
    }
    // Checked last, so that a moved constant fails on the decisions first.
    assert_eq!((AXES_D_MIN, AXES_D_MAX), (low, high));
}

/// `filter_axes_points` uses the errors of the points: a point just inside
/// the unit sphere, as a point on a line between two far parents, whose
/// approximation has an error far larger than its distance to the sphere.
/// `filter_axes` on the approximations with no uncertainty decides; with the
/// uncertainty of the errors, as `filter_axes_points` gives it, it must not.
#[test]
fn filter_axes_points_uses_the_errors_of_the_points() {
    let h = pow2(-30);
    let far = pow2(20) + 0.375;
    for k in 0..3 {
        // A point just inside the unit sphere on axis `k`, as the midpoint of
        // two parents `far` away from it on the next axis.
        let mut x = [0.0; 3];
        x[k] = 1.0 - h;
        let parents = [-1.0, 1.0].map(|s| {
            let mut q = x;
            q[(k + 1) % 3] = s * far;
            q
        });
        let line = Lnc::new_unchecked(parents[0], parents[1], 0.5);
        let err = *line.err();
        assert!(err.iter().any(|&v| v > 1e-12), "{err:?}");
        // Four points of the unit sphere.
        let mut sphere = [[0.0; 3]; 4];
        sphere[0][k] = -1.0;
        sphere[1][(k + 1) % 3] = 1.0;
        sphere[2][(k + 2) % 3] = 1.0;
        sphere[3][(k + 1) % 3] = -1.0;
        let [a, b, c, d] = sphere.map(Explicit::new_unchecked);
        let approx = [sphere[0], sphere[1], sphere[2], sphere[3], *line.approx()];
        let mut exact = approx.map(|q| q.map(Dyadic::from_f64));
        exact[4] = Arg::Lnc(parents[0], parents[1], 0.5).exact();
        let truth = oracle_at(&exact);
        assert_ne!(truth, Sign::Zero);
        let without = stages::filter_axes(refs(&approx), [0.0; 3]);
        assert_eq!(without, Some(oracle(&approx)), "{k}: {approx:?}");
        let e = uncertainty([a.err(), b.err(), c.err(), d.err(), line.err()]);
        assert_eq!(stages::filter_axes(refs(&approx), e), None, "{k} e = {e:?}");
        assert_eq!(
            stages::filter_axes_points(&a, &b, &c, &d, &line),
            None,
            "{k}"
        );
        assert_eq!(stages::filter_points(&a, &b, &c, &d, &line), None, "{k}");
        let mut buf = vec![0.0; 1 << 16];
        let mut scratch = Scratch::borrowed(&mut buf);
        assert_eq!(
            stages::staged_in(&mut scratch, &a, &b, &c, &d, &line),
            Ok(truth),
            "{k}"
        );
    }
}
