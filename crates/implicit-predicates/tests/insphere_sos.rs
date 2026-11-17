//! `insphere_sos` and `insphere_sos_in`: the sign of `insphere` when it is
//! not zero; otherwise the sign of the determinant of `insphere` with the
//! lifted coordinate of each point perturbed by a power of two ordered by
//! its index, evaluated exactly by an oracle that does not come from the
//! tie-break; the parity of permutations; no zero unless the five points
//! are coplanar; indices of any size; and the arena of `insphere_sos_in`.
//!
//! The tests of `insphere_sos_in` run without `std` too; with `std` each
//! call is also given to `insphere_sos`, which must agree.

mod common;

#[path = "generators/insphere_calls.rs"]
mod insphere_calls;

use std::cell::RefCell;
use std::collections::BTreeSet;

use common::args::{Arg, Call5, any_points, with_concrete5};
use common::insphere::{Points5, oracle, oracle_args};
use common::sign_of;
use implicit_predicates::{Lnc, Point, Scratch, ScratchExhausted, Sign, insphere_sos_in};
use implicit_predicates_corpus::exact;
use implicit_predicates_oracle::Dyadic;
use implicit_predicates_test_support::{config, grid5, random5};
use insphere_calls::{Args5, calls, cospherical_args, extreme_args, triangle_at};
use proptest::prelude::*;

/// Five points with integer coordinates.
type Grid5 = [[i32; 3]; 5];

/// The points of `p` as `f64`, exactly.
fn to_f64(p: &Grid5) -> Points5 {
    p.map(|q| q.map(f64::from))
}

thread_local! {
    /// The arena of [`Sos`], allocated once.
    static BUF: RefCell<Vec<f64>> = RefCell::new(vec![0.0; 1 << 16]);
    /// Every permutation of five elements, with its parity.
    static PERMUTATIONS: Vec<([usize; 5], Sign)> = all_permutations();
}

/// `insphere_sos_in` with an arena large enough for every call of these
/// tests, which must leave it as it was; with `std`, also `insphere_sos`,
/// which must give the same sign.
struct Sos([usize; 5]);

impl Call5 for Sos {
    type Output = Sign;
    fn call<A: Point, B: Point, C: Point, D: Point, E: Point>(
        &mut self,
        a: &A,
        b: &B,
        c: &C,
        d: &D,
        e: &E,
    ) -> Sign {
        let sign = BUF.with_borrow_mut(|buf| {
            let mut scratch = Scratch::borrowed(buf);
            let sign = insphere_sos_in(&mut scratch, a, b, c, d, e, self.0);
            assert_eq!(scratch.used(), 0);
            sign.expect("a large arena")
        });
        #[cfg(feature = "std")]
        assert_eq!(
            implicit_predicates::insphere_sos(a, b, c, d, e, self.0),
            sign
        );
        sign
    }
}

/// The sign of the tie-break on the five arguments in every type: arrays,
/// `Explicit` and `Lnc`, and `AnyPoint`; checks that they agree.
fn sos_args(args: &Args5, idx: [usize; 5]) -> Sign {
    let sign = with_concrete5(args, true, &mut Sos(idx));
    assert_eq!(with_concrete5(args, false, &mut Sos(idx)), sign, "{args:?}");
    let [a, b, c, d, e] = any_points(args);
    assert_eq!(Sos(idx).call(&a, &b, &c, &d, &e), sign, "{args:?}");
    sign
}

/// The sign of the tie-break on explicit points.
fn sos(p: &Points5, idx: [usize; 5]) -> Sign {
    let [a, b, c, d, e] = p;
    Sos(idx).call(a, b, c, d, e)
}

/// Every permutation of five elements, with its parity.
fn permutations() -> Vec<([usize; 5], Sign)> {
    PERMUTATIONS.with(Clone::clone)
}

fn all_permutations() -> Vec<([usize; 5], Sign)> {
    (0..5usize.pow(5))
        .map(|n| std::array::from_fn(|k| n / 5usize.pow(k as u32) % 5))
        .filter(|p: &[usize; 5]| p.iter().collect::<BTreeSet<_>>().len() == 5)
        .map(|p| {
            let inversions = (0..5)
                .flat_map(|i| (i + 1..5).map(move |j| (i, j)))
                .filter(|&(i, j)| p[i] > p[j])
                .count();
            let parity = if inversions % 2 == 0 {
                Sign::Positive
            } else {
                Sign::Negative
            };
            (p, parity)
        })
        .collect()
}

/// The rank of the index of the point `k` among the five, 1 for the
/// smallest.
fn rank(idx: &[usize; 5], k: usize) -> usize {
    1 + idx.iter().filter(|&&i| i < idx[k]).count()
}

/// How far apart, in powers of two, the perturbations of two consecutive
/// ranks are: the point of rank `r` is perturbed by `2^(-STEP r)`.
const STEP: i32 = 200;

/// The largest magnitude of a coordinate of the points of the oracle.
const COORDINATE_BOUND: i32 = 8;

/// A bound on `orient3d` of four of those points: each entry of its 3x3
/// determinant of differences is at most `2 * 8 = 16` in magnitude, so the
/// determinant, a sum of `3! = 6` products of three entries, is at most
/// `6 * 16^3 = 24,576`, below `2^15`.
const MINOR_BOUND: i64 = 6 * 16 * 16 * 16;

// Why each term of the perturbed determinant dominates the ones after it.
// The determinant is linear in the lifted column, so its perturbed value is
// `D - sum_r 2^(-STEP r) C_r`, where `D` is the unperturbed value and `C_r`
// the cofactor of the lifted entry of the point of rank `r`, which is plus
// or minus `orient3d` of the other four points. With integer coordinates,
// `D` and every `C_r` are integers, and `|C_r| <= MINOR_BOUND < 2^15`. If
// `D` is not zero, `|D| >= 1 > 5 * 2^15 * 2^(-STEP)`, more than all the
// perturbation. If `D` is zero and `C_r` is the first cofactor that is not,
// `2^(-STEP r) |C_r| >= 2^(-STEP r)`, while the terms after it add up to at
// most `4 * 2^15 * 2^(-STEP (r + 1))`, which is smaller since
// `2^17 < 2^STEP`. So the sign of the perturbed determinant is that of the
// first term that is not zero, as the tie-break assumes of an infinitesimal
// perturbation.
const _: () = assert!(MINOR_BOUND < 1 << 15 && 17 < STEP);

/// `orient3d` of the points of `p` other than `j`, in their order, on
/// integers.
fn minor(p: &Grid5, j: usize) -> i64 {
    let q: Vec<[i64; 3]> = (0..5)
        .filter(|&k| k != j)
        .map(|k| p[k].map(i64::from))
        .collect();
    let r: [[i64; 3]; 3] = std::array::from_fn(|i| std::array::from_fn(|k| q[i][k] - q[3][k]));
    r[0][0] * (r[1][1] * r[2][2] - r[1][2] * r[2][1])
        - r[0][1] * (r[1][0] * r[2][2] - r[1][2] * r[2][0])
        + r[0][2] * (r[1][0] * r[2][1] - r[1][1] * r[2][0])
}

/// Fails unless the points of `p` are in the bounds under which each term
/// of the perturbed determinant dominates the ones after it.
fn assert_terms_dominate(p: &Grid5) {
    assert!(
        p.iter().flatten().all(|c| c.abs() <= COORDINATE_BOUND),
        "{p:?}"
    );
    for j in 0..5 {
        assert!(minor(p, j).abs() <= MINOR_BOUND, "{p:?}");
    }
}

/// The 5x5 determinant of `rows`, as the sum over the permutations.
fn det5(rows: &[[Dyadic; 5]; 5]) -> Dyadic {
    let zero = Dyadic::from_f64(0.0);
    permutations()
        .into_iter()
        .fold(zero.clone(), |sum, (p, parity)| {
            let product = (1..5).fold(rows[0][p[0]].clone(), |x, i| &x * &rows[i][p[i]]);
            match parity {
                Sign::Positive => &sum + &product,
                _ => &sum - &product,
            }
        })
}

/// The rows `(x, y, z, x^2 + y^2 + z^2 - eps(k), 1)` of the points of `p`.
fn rows(p: &Grid5, eps: impl Fn(usize) -> Dyadic) -> [[Dyadic; 5]; 5] {
    std::array::from_fn(|k| {
        let [x, y, z] = p[k].map(|c| Dyadic::from_f64(f64::from(c)));
        let lifted = &(&(&x * &x) + &(&y * &y)) + &(&z * &z);
        let lifted = &lifted - &eps(k);
        [x, y, z, lifted, Dyadic::from_f64(1.0)]
    })
}

/// `2^(-STEP r)`, exactly.
fn perturbation(r: usize) -> Dyadic {
    let exponent = 1023 - STEP * r as i32;
    Dyadic::from_f64(f64::from_bits((exponent as u64) << 52))
}

/// The sign of the determinant of `insphere` at the points of `p` with the
/// lifted coordinate of each point decreased by `2^(-STEP r)`, where `r` is
/// the rank of its index.
fn perturbed(p: &Grid5, idx: [usize; 5]) -> Sign {
    sign_of(det5(&rows(p, |k| perturbation(rank(&idx, k)))).sign())
}

/// The sign of the unperturbed 5x5 determinant, which is that of `insphere`.
fn unperturbed(p: &Grid5) -> Sign {
    sign_of(det5(&rows(p, |_| Dyadic::from_f64(0.0))).sign())
}

/// Whether the five points of `p` are coplanar.
fn coplanar(p: &Grid5) -> bool {
    (0..5).all(|j| minor(p, j) == 0)
}

/// Five distinct indices of any size, in any order, or a permutation of
/// `0..5`.
fn distinct_indices() -> impl Strategy<Value = [usize; 5]> {
    let order = || Just(vec![0usize, 1, 2, 3, 4]).prop_shuffle();
    prop_oneof![
        order().prop_map(|p| std::array::from_fn(|k| p[k])),
        (prop::collection::btree_set(any::<usize>(), 5), order()).prop_map(|(values, p)| {
            let values: Vec<_> = values.into_iter().collect();
            std::array::from_fn(|k| values[p[k]])
        }),
    ]
}

/// The points of the integer lattice at squared distance `r2` from the
/// origin.
fn lattice_sphere(r2: i32) -> Vec<[i32; 3]> {
    let range = || -3i32..=3;
    range()
        .flat_map(|x| range().flat_map(move |y| range().map(move |z| [x, y, z])))
        .filter(|[x, y, z]| x * x + y * y + z * z == r2)
        .collect()
}

/// Five points of the integer lattice on a sphere of radius 1 to 3 about a
/// lattice point with coordinates in `-3..=3`: cospherical, with points
/// that repeat and four that may be coplanar.
fn cospherical_grid() -> impl Strategy<Value = Grid5> {
    let spheres: Vec<_> = [1, 2, 3, 5, 6, 9].into_iter().map(lattice_sphere).collect();
    let index = || any::<prop::sample::Index>();
    (
        prop::sample::select(spheres),
        [-3i32..=3, -3i32..=3, -3i32..=3],
        [index(), index(), index(), index(), index()],
    )
        .prop_map(|(sphere, centre, at)| {
            at.map(|i| {
                let offset = i.get(&sphere);
                std::array::from_fn(|k| centre[k] + offset[k])
            })
        })
}

/// Five coplanar points: `o + s u + t v` for small integers.
fn coplanar_grid() -> impl Strategy<Value = Grid5> {
    let v = || [-2i32..=2, -2i32..=2, -2i32..=2];
    let st = || [-1i32..=1, -1i32..=1];
    (v(), v(), v(), [st(), st(), st(), st(), st()]).prop_map(|(o, u, v, st)| {
        st.map(|[s, t]| std::array::from_fn(|k| o[k] + s * u[k] + t * v[k]))
    })
}

/// Points of the grid of the oracle, most of them ties: cospherical,
/// cospherical with a point repeated, coplanar, and any.
fn tie_grid() -> impl Strategy<Value = Grid5> {
    let any_point = || [-8i32..=8, -8i32..=8, -8i32..=8];
    prop_oneof![
        3 => cospherical_grid(),
        2 => (cospherical_grid(), 0usize..5, 1usize..5).prop_map(|(mut p, i, k)| {
            p[(i + k) % 5] = p[i];
            p
        }),
        1 => coplanar_grid(),
        1 => [any_point(), any_point(), any_point(), any_point(), any_point()],
    ]
}

fn mixed_calls() -> impl Strategy<Value = Args5> {
    prop_oneof![calls(), cospherical_args(), extreme_args()]
}

/// Whether the exact points of `args` are coplanar.
fn coplanar_args(args: &Args5) -> bool {
    let c = args.each_ref().map(Arg::exact);
    (0..5).all(|j| {
        let q: Vec<_> = (0..5).filter(|&k| k != j).map(|k| &c[k]).collect();
        exact::orient3d(q[0], q[1], q[2], q[3]) == 0
    })
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn it_is_insphere_when_insphere_is_not_zero(
        args in mixed_calls(),
        idx in distinct_indices(),
    ) {
        let sign = sos_args(&args, idx);
        let insphere = oracle_args(&args);
        if insphere == Sign::Zero {
            prop_assert_eq!(sign == Sign::Zero, coplanar_args(&args), "{:?}", args);
        } else {
            prop_assert_eq!(sign, insphere, "{:?}", args);
        }
    }

    #[test]
    fn it_matches_the_perturbed_determinant(p in tie_grid(), idx in distinct_indices()) {
        assert_terms_dominate(&p);
        let f = to_f64(&p);
        // The 5x5 determinant has the sign of `insphere`.
        prop_assert_eq!(unperturbed(&p), oracle(&f), "{:?}", p);
        prop_assert_eq!(sos(&f, idx), perturbed(&p, idx), "{:?} {:?}", p, idx);
    }

    #[test]
    fn a_point_on_a_triangle_has_the_sign_of_its_value(
        (p, kind) in prop_oneof![
            // The midpoint of an edge needs the grid, where `2 x - y` is exact.
            (random5(), prop::sample::select(vec![0usize, 1, 2, 3, 5])),
            (grid5(), 0usize..6),
        ],
        position in 0usize..5,
        (b, c) in (1usize..5, 1usize..5),
        t in 0.0..=1.0f64,
        idx in distinct_indices(),
    ) {
        // The sign depends on the exact points and on the indices only: a
        // point on a triangle is the point it equals, wherever it is, even
        // when that point is another argument, which a tie-break must
        // separate by the indices.
        let (i, j, l) = (position, (position + b) % 5, (position + c) % 5);
        let on_a_triangle = {
            let mut args = p.map(Arg::Explicit);
            args[position] = triangle_at(&p, i, j, l, kind, t);
            args
        };
        let mut value = p;
        value[position] = if kind == 5 { p[l] } else { p[i] };
        prop_assert_eq!(
            sos_args(&on_a_triangle, idx),
            sos_args(&value.map(Arg::Explicit), idx),
            "{:?} {:?}", on_a_triangle, idx
        );
    }

    #[test]
    fn huge_indices_sort_like_small_ones(p in tie_grid()) {
        let f = to_f64(&p);
        prop_assert_eq!(
            sos(&f, [usize::MAX, 0, usize::MAX - 1, 1, 2]),
            sos(&f, [4, 0, 3, 1, 2]),
            "{:?}", p
        );
    }
}

/// The configuration of the properties that check every ordering of the
/// indices of each case: a sixteenth of the cases, each 120 orderings.
fn every_ordering_config() -> ProptestConfig {
    let cases = config().cases;
    ProptestConfig::with_cases(cases.div_ceil(16))
}

proptest! {
    #![proptest_config(every_ordering_config())]

    #[test]
    fn permuting_the_points_with_their_indices_flips_the_sign_by_parity(
        p in cospherical_grid(),
        idx in distinct_indices(),
    ) {
        let f = to_f64(&p);
        let sign = sos(&f, idx);
        for (perm, parity) in permutations() {
            let q = perm.map(|k| f[k]);
            let jdx = perm.map(|k| idx[k]);
            prop_assert_eq!(sos(&q, jdx), parity * sign, "{:?} {:?} {:?}", p, idx, perm);
        }
    }

    #[test]
    fn it_is_never_zero_unless_the_five_points_are_coplanar(
        p in prop_oneof![cospherical_grid(), coplanar_grid()],
    ) {
        // The points, and the points with `e` replaced by each of the others.
        let variants = std::iter::once(p).chain((0..4).map(|k| {
            let mut q = p;
            q[4] = p[k];
            q
        }));
        for q in variants {
            let f = to_f64(&q);
            let flat = coplanar(&q);
            for (n, (idx, _)) in permutations().into_iter().enumerate() {
                let sign = sos(&f, idx);
                prop_assert_eq!(sign == Sign::Zero, flat, "{:?} {:?}", q, idx);
                // `e` the same point as `a`, by reference, and as a point on
                // a line whose exact value is `a`, on one ordering in ten,
                // since points on a line take much longer.
                if q[4] == q[0] && n % 10 == 0 {
                    let [a, b, c, d, _] = &f;
                    prop_assert_eq!(Sos(idx).call(a, b, c, d, a), sign);
                    let args: Args5 = std::array::from_fn(|k| match k {
                        4 => Arg::Lnc(f[0], f[1], 1.0),
                        _ => Arg::Explicit(f[k]),
                    });
                    prop_assert_eq!(sos_args(&args, idx), sign);
                    // And as the vertex of a triangle whose other corners are
                    // other points.
                    let args: Args5 = std::array::from_fn(|k| match k {
                        4 => Arg::Bpt(f[0], f[1], f[2], 1.0, 0.0),
                        _ => Arg::Explicit(f[k]),
                    });
                    prop_assert_eq!(sos_args(&args, idx), sign);
                }
            }
        }
    }
}

/// Four points of the unit sphere, positively oriented, and `e` equal to
/// `a`: the tie is broken by the term of the smaller rank of `a` and `e`,
/// which is any of the first four, so every ordering of the indices reaches
/// each of the first four terms; and the sign matches the oracle on each.
#[test]
fn every_ordering_reaches_the_terms_it_should() {
    let p: Grid5 = [[1, 0, 0], [0, 1, 0], [0, 0, 1], [-1, 0, 0], [1, 0, 0]];
    assert_terms_dominate(&p);
    let f = to_f64(&p);
    let mut reached = BTreeSet::new();
    for (idx, _) in permutations() {
        // The rank of the first term whose cofactor is not zero.
        let first = (1..=5)
            .find(|&r| minor(&p, (0..5).find(|&k| rank(&idx, k) == r).unwrap()) != 0)
            .expect("four of the points are not coplanar");
        reached.insert(first);
        assert_eq!(sos(&f, idx), perturbed(&p, idx), "{idx:?}");
    }
    assert_eq!(reached, BTreeSet::from([1, 2, 3, 4]));
}

#[test]
fn a_small_arena_is_reported_and_left_as_it_was() {
    // Five points of the sphere of radius 5: a tie, which goes to the stage
    // on big integers of `insphere_in`.
    let [a, b, c, d, e] = [
        [3.0, 4.0, 0.0],
        [5.0, 0.0, 0.0],
        [0.0, 3.0, 4.0],
        [-4.0, 0.0, 3.0],
        [0.0, -5.0, 0.0],
    ];
    let idx = [4, 3, 2, 1, 0];
    let m = Lnc::new(a, b, 1.0).unwrap();
    let mut small = [0.0; 12];
    let mut scratch = Scratch::borrowed(&mut small);
    // Some data of the caller below the call.
    scratch.push(&[1.0, 2.0]).unwrap();
    let mark = scratch.mark();
    assert_eq!(
        insphere_sos_in(&mut scratch, &a, &b, &c, &d, &e, idx),
        Err(ScratchExhausted)
    );
    assert_eq!(
        insphere_sos_in(&mut scratch, &m, &b, &c, &d, &e, idx),
        Err(ScratchExhausted)
    );
    assert_eq!(scratch.mark(), mark);
    assert_eq!(scratch.used(), 2);

    // With room, the tie is broken, through `orient3d_in` with a point on a
    // line too, and the arena is left as it was.
    let mut buf = vec![0.0; 1 << 15];
    let mut scratch = Scratch::borrowed(&mut buf);
    scratch.push(&[1.0, 2.0]).unwrap();
    let sign = insphere_sos_in(&mut scratch, &a, &b, &c, &d, &e, idx).unwrap();
    assert_ne!(sign, Sign::Zero);
    assert_eq!(
        insphere_sos_in(&mut scratch, &m, &b, &c, &d, &e, idx),
        Ok(sign)
    );
    assert_eq!(scratch.used(), 2);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "the five indices of `insphere_sos` must be distinct")]
fn repeated_indices_panic_in_debug() {
    // Not a tie: the assertion comes before `insphere`.
    let p = [
        [0.0; 3],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.1; 3],
    ];
    let mut buf = vec![0.0; 1 << 15];
    let [a, b, c, d, e] = &p;
    let _ = insphere_sos_in(
        &mut Scratch::borrowed(&mut buf),
        a,
        b,
        c,
        d,
        e,
        [0, 1, 2, 3, 0],
    );
}

#[test]
#[cfg(all(debug_assertions, feature = "std"))]
#[should_panic(expected = "the five indices of `insphere_sos` must be distinct")]
fn repeated_indices_panic_in_debug_with_the_arena_of_the_thread() {
    let p = [
        [0.0; 3],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.1; 3],
    ];
    let [a, b, c, d, e] = &p;
    let _ = implicit_predicates::insphere_sos(a, b, c, d, e, [7, 1, 2, 7, 4]);
}

/// What the documentation says of five cospherical points with `a`, `b`,
/// `c` and `d` positively oriented: `e` counts as inside when its index is
/// the smallest, wherever it is on the sphere; with the index of `a` the
/// smallest, it counts as inside or outside depending on where it is, since
/// the sign is then that of `orient3d(b, c, d, e)`.
#[test]
fn the_documented_side_of_a_cospherical_point() {
    // Points of the sphere of radius 5 about the origin.
    let (a, b, c, d) = ([5, 0, 0], [0, 5, 0], [0, 0, 5], [-5, 0, 0]);
    let outside = [0, 0, -5];
    // On the other side of the plane through b, c and d from a.
    let inside = [-3, 4, 0];
    for e in [outside, inside] {
        let p: Grid5 = [a, b, c, d, e];
        assert_terms_dominate(&p);
        assert_eq!(minor(&p, 4), 250);
        let f = to_f64(&p);
        assert_eq!(oracle(&f), Sign::Zero);
        assert_eq!(sos(&f, [1, 2, 3, 4, 0]), Sign::Positive);
        for idx in [[1, 2, 3, 4, 0], [0, 1, 2, 3, 4]] {
            assert_eq!(sos(&f, idx), perturbed(&p, idx), "{p:?} {idx:?}");
        }
    }
    assert_eq!(minor(&[a, b, c, d, inside], 0), 50);
    assert_eq!(
        sos(&to_f64(&[a, b, c, d, outside]), [0, 1, 2, 3, 4]),
        Sign::Negative
    );
    assert_eq!(
        sos(&to_f64(&[a, b, c, d, inside]), [0, 1, 2, 3, 4]),
        Sign::Positive
    );
    // The documentation does not claim that `e` is outside whenever `a` has
    // the smallest index.
    let docs: String = include_str!("../src/sos.rs")
        .lines()
        .map(|line| line.trim_start().trim_start_matches("///").trim())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(docs.contains(
        "`e` counts as inside it if its index is the smallest of the five. With a smaller \
         index on another point, `e` may count as inside or outside, depending on where \
         the points are."
    ));
    assert!(!docs.contains("as outside if the index of `a` is"));
}
