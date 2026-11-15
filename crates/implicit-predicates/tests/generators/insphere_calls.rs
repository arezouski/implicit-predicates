//! The calls of the property tests of `insphere`: five arguments, each an
//! explicit point or a point on a line, from random, grid, cospherical and
//! extreme points. Shared by the test binaries that declare it with
//! `#[path]`; they also declare `common`.

use implicit_predicates_test_support::{
    cospherical5, extreme_value, grid5, lnc_parameter, lnc_params, near_cospherical5, random5,
};
use proptest::prelude::*;

use crate::common::args::Arg;
use crate::common::insphere::Points5;

/// The five arguments of a call.
pub type Args5 = [Arg; 5];

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

/// The points `p` with none to five of them, at any positions, replaced by
/// points on a line from [`line_for`].
pub fn with_lines(p: Points5) -> impl Strategy<Value = Args5> {
    (
        prop::sample::subsequence(vec![0usize, 1, 2, 3, 4], 0..=5),
        [
            line_for(p[0]),
            line_for(p[1]),
            line_for(p[2]),
            line_for(p[3]),
            line_for(p[4]),
        ],
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

/// Calls on random points, points of a grid of small integers and points
/// of which four lie on a sphere and one is a few units in the last place
/// off it, with points on a line among them.
pub fn calls() -> impl Strategy<Value = Args5> {
    prop_oneof![random5(), grid5(), near_cospherical5()].prop_flat_map(with_lines)
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

/// Calls whose exact points lie on one sphere: from [`cospherical5`], with
/// a point repeated in half of them, and none to five points replaced by
/// points on a line from [`line_at`].
pub fn cospherical_args() -> impl Strategy<Value = Args5> {
    let line = || (0usize..5, 1usize..5, 0.0..=1.0f64);
    (
        cospherical5(),
        prop::option::of((0usize..5, 1usize..5)),
        prop::sample::subsequence(vec![0usize, 1, 2, 3, 4], 0..=5),
        [line(), line(), line(), line(), line()],
    )
        .prop_map(|(mut p, repeated, at, lines)| {
            if let Some((i, k)) = repeated {
                p[(i + k) % 5] = p[i];
            }
            std::array::from_fn(|i| {
                let (kind, k, t) = lines[i];
                if at.contains(&i) {
                    line_at(&p, i, (i + k) % 5, kind, t)
                } else {
                    Arg::Explicit(p[i])
                }
            })
        })
}

/// Five arguments whose inputs are values of one family of
/// [`extreme_value`], with points on a line, in any number from none to
/// five, at any positions; the others are explicit.
pub fn extreme_args() -> impl Strategy<Value = Args5> {
    (0usize..5).prop_flat_map(|family| {
        let v = move || extreme_value(family);
        let point = move || [v(), v(), v()];
        let line = move || (point(), point(), lnc_parameter());
        (
            [point(), point(), point(), point(), point()],
            [line(), line(), line(), line(), line()],
            prop::sample::subsequence(vec![0usize, 1, 2, 3, 4], 0..=5),
        )
            .prop_map(|(explicit, lines, at)| {
                std::array::from_fn(|i| {
                    if at.contains(&i) {
                        let (p, q, t) = lines[i];
                        Arg::Lnc(p, q, t)
                    } else {
                        Arg::Explicit(explicit[i])
                    }
                })
            })
    })
}
