//! The arenas of `orient3d` with implicit points: an arena too small is
//! reported and left as it was, the arena of each thread grows for long
//! coordinates, a point whose exact coordinates call a predicate does not
//! make it panic, and the result does not depend on earlier calls.

// `orient3d` needs the `std` feature.
#![cfg(feature = "std")]

mod common;

use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};

use common::args::{Arg, Args, any_points, exact_sign};
use common::generated::exact_at;
use implicit_predicates::{
    ExactCoords, ExactError, Lnc, Point, Scratch, ScratchExhausted, Sign, Weight, orient3d,
    orient3d_in,
};
use implicit_predicates_oracle::Dyadic;
use implicit_predicates_test_support::{coplanar4, near_coplanar4, sample};

/// The initial size of the arena of each thread, in `f64`.
const THREAD_ARENA: usize = 16_384;

/// A point given by its exact coordinates as expansions: its approximation
/// is the largest component of each, with an infinite error, so the filter
/// never decides and the exact coordinates are always read.
#[derive(Clone, Debug)]
struct Expansions {
    coords: [Vec<f64>; 3],
    approx: [f64; 3],
}

impl Expansions {
    fn new(coords: [Vec<f64>; 3]) -> Expansions {
        let approx = std::array::from_fn(|k| coords[k].last().copied().unwrap_or(0.0));
        Expansions { coords, approx }
    }

    fn exact(&self) -> [Dyadic; 3] {
        self.coords.each_ref().map(|e| Dyadic::sum_of(e))
    }
}

impl Point for Expansions {
    fn approx(&self) -> &[f64; 3] {
        &self.approx
    }

    fn err(&self) -> &[f64; 3] {
        &[f64::INFINITY; 3]
    }

    fn is_explicit(&self) -> bool {
        false
    }

    fn exact(&self, scratch: &mut Scratch<'_>) -> Result<ExactCoords, ExactError> {
        let [x, y, z] = &self.coords;
        Ok(ExactCoords {
            x: scratch.push(x)?,
            y: scratch.push(y)?,
            z: scratch.push(z)?,
            w: Weight::One,
        })
    }
}

/// A coordinate of 16 components of 20 significant bits taken from `seed`,
/// each with a random sign, the lowest bit of component `i` at
/// `offset - 188 + 26 i`, so that no two overlap; for `offset` in `0..=6`
/// they are within `2^-188` and `2^229`.
///
/// Different offsets for different coordinates keep the sums of the exact
/// stages from being exact in `f64`, so their expansions keep many
/// components.
fn long_coordinate(offset: i32, seed: u64) -> Vec<f64> {
    let mut state = seed | 1;
    (0..16)
        .map(|i| {
            // A step of xorshift for the significand, odd so that its
            // lowest bit is where it is placed.
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let significand = (1 << 19) | (state & ((1 << 19) - 1)) | 1;
            let x = significand as f64 * 2f64.powi(offset - 188 + 26 * i);
            if (state >> 60) & 1 == 1 { -x } else { x }
        })
        .collect()
}

/// Four points with coordinates of 16 components each, from
/// [`long_coordinate`] with offsets from `offset` on.
fn long_points(seed: u64, offset: i32) -> [Expansions; 4] {
    std::array::from_fn(|i| {
        Expansions::new(std::array::from_fn(|k| {
            let n = i * 3 + k;
            long_coordinate((offset + n as i32) % 7, seed ^ (n as u64 * 77_777))
        }))
    })
}

/// Four points on lines of a plane: the filter cannot decide them, so the
/// stages over spans run.
fn coplanar_on_lines() -> Vec<Args> {
    sample(coplanar4(), 20)
        .into_iter()
        .chain(sample(near_coplanar4(), 20))
        .map(|p| std::array::from_fn(|i| Arg::Lnc(p[i], p[(i + 1) % 4], 0.375)))
        .collect()
}

/// `orient3d_in` with an arena of `size` `f64`; checks that the arena is
/// left as it was.
fn in_arena_of<A: Point, B: Point, C: Point, D: Point>(
    size: usize,
    a: &A,
    b: &B,
    c: &C,
    d: &D,
) -> (Result<Option<Sign>, ScratchExhausted>, usize) {
    let mut buf = vec![0.0; size];
    let mut scratch = Scratch::borrowed(&mut buf);
    let result = orient3d_in(&mut scratch, a, b, c, d);
    assert_eq!(scratch.used(), 0, "size {size}");
    (result, scratch.high_water())
}

/// Checks that `orient3d_in` reports an arena too small until it fits, and
/// then gives `expected`; returns the smallest size that fits.
fn fits_from<A: Point, B: Point, C: Point, D: Point>(
    a: &A,
    b: &B,
    c: &C,
    d: &D,
    expected: Sign,
) -> usize {
    // The arena a large call needs, at most its high-water mark.
    let (result, high_water) = in_arena_of(1 << 20, a, b, c, d);
    assert_eq!(result, Ok(Some(expected)));
    assert_eq!(in_arena_of(high_water, a, b, c, d).0, Ok(Some(expected)));
    // Below the smallest size that fits, every size is too small.
    let (mut low, mut high) = (0, high_water);
    while low < high {
        let mid = (low + high) / 2;
        match in_arena_of(mid, a, b, c, d).0 {
            Ok(result) => {
                assert_eq!(result, Some(expected), "size {mid}");
                high = mid;
            }
            Err(_) => low = mid + 1,
        }
    }
    let sizes = (0..low)
        .step_by(1 + low / 64)
        .chain([low.saturating_sub(1)]);
    for size in sizes.filter(|&s| s < low) {
        assert!(in_arena_of(size, a, b, c, d).0.is_err(), "size {size}");
    }
    for size in [low, low + 1, low + 100, 2 * low] {
        assert_eq!(
            in_arena_of(size, a, b, c, d).0,
            Ok(Some(expected)),
            "size {size}"
        );
    }
    low
}

#[test]
fn an_arena_too_small_is_reported_until_it_fits() {
    for args in coplanar_on_lines() {
        let [a, b, c, d] = any_points(&args);
        let low = fits_from(&a, &b, &c, &d, exact_sign(&args));
        assert!(low > 0, "{args:?}");
    }
}

#[test]
fn long_coordinates_grow_the_arena_of_the_thread() {
    let mut beyond = 0;
    let lows = [0, 3];
    for (seed, low) in [1, 99, 12_345, 0xdead_beef]
        .into_iter()
        .flat_map(|seed| lows.map(|low| (seed, low)))
    {
        let [a, b, c, d] = long_points(seed, low);
        let expected = exact_at(&[a.exact(), b.exact(), c.exact(), d.exact()]);
        let low = fits_from(&a, &b, &c, &d, expected);
        beyond += usize::from(low > THREAD_ARENA);
        // The arena of the thread grows to what the call needs.
        assert_eq!(orient3d(&a, &b, &c, &d), Some(expected));
        assert_eq!(orient3d(&a, &b, &c, &d), Some(expected));
    }
    // Some calls need more than the initial arena of the thread.
    assert!(beyond > 0);
}

/// A point on a line whose exact coordinates are written after a call to
/// `orient3d` with implicit points, which needs an arena while the arena
/// of the thread is in use.
struct Reentrant {
    point: Lnc,
    inner: Args,
    calls: Cell<usize>,
}

impl Point for Reentrant {
    fn approx(&self) -> &[f64; 3] {
        self.point.approx()
    }

    fn err(&self) -> &[f64; 3] {
        self.point.err()
    }

    fn is_explicit(&self) -> bool {
        false
    }

    fn exact(&self, scratch: &mut Scratch<'_>) -> Result<ExactCoords, ExactError> {
        let [a, b, c, d] = any_points(&self.inner);
        assert_eq!(orient3d(&a, &b, &c, &d), Some(exact_sign(&self.inner)));
        self.calls.set(self.calls.get() + 1);
        self.point.exact(scratch)
    }
}

#[test]
fn a_point_that_calls_a_predicate_does_not_make_it_panic() {
    let cases = coplanar_on_lines();
    for (n, args) in cases.iter().enumerate() {
        let Arg::Lnc(p, q, t) = args[0] else {
            unreachable!()
        };
        let a = Reentrant {
            point: Lnc::new_unchecked(p, q, t),
            inner: cases[(n + 1) % cases.len()],
            calls: Cell::new(0),
        };
        let [_, b, c, d] = any_points(args);
        assert_eq!(orient3d(&a, &b, &c, &d), Some(exact_sign(args)), "{args:?}");
        assert_eq!(a.calls.get(), 1);
    }
}

/// A point whose exact coordinates panic.
struct Panics(Lnc);

impl Point for Panics {
    fn approx(&self) -> &[f64; 3] {
        self.0.approx()
    }

    fn err(&self) -> &[f64; 3] {
        &[f64::INFINITY; 3]
    }

    fn is_explicit(&self) -> bool {
        false
    }

    fn exact(&self, scratch: &mut Scratch<'_>) -> Result<ExactCoords, ExactError> {
        // Leave something in the arena first.
        scratch.push(&[1.0])?;
        panic!("exact coordinates that panic");
    }
}

/// The results of `orient3d` on every case, in order, on a new thread.
fn results_on_a_new_thread(cases: &[Args]) -> Vec<Option<Sign>> {
    let cases = cases.to_vec();
    std::thread::spawn(move || {
        cases
            .iter()
            .map(|args| {
                let [a, b, c, d] = any_points(args);
                orient3d(&a, &b, &c, &d)
            })
            .collect()
    })
    .join()
    .unwrap()
}

#[test]
fn the_result_does_not_depend_on_earlier_calls() {
    let cases = coplanar_on_lines();
    let alone: Vec<Option<Sign>> = cases
        .iter()
        .map(|args| results_on_a_new_thread(&[*args])[0])
        .collect();
    let expected: Vec<Option<Sign>> = cases.iter().map(|args| Some(exact_sign(args))).collect();
    assert_eq!(alone, expected);
    assert_eq!(results_on_a_new_thread(&cases), expected);
    // In reverse order, after long coordinates, and after a call that
    // panicked with the arena of the thread in use.
    let mut reversed = cases.clone();
    reversed.reverse();
    let mut expected_reversed = expected.clone();
    expected_reversed.reverse();
    std::thread::spawn(move || {
        let [a, b, c, d] = long_points(99, 3);
        assert!(orient3d(&a, &b, &c, &d).is_some());
        let Arg::Lnc(p, q, t) = reversed[0][0] else {
            unreachable!()
        };
        let [_, b, c, d] = any_points(&reversed[0]);
        let panicked = catch_unwind(AssertUnwindSafe(|| {
            orient3d(&Panics(Lnc::new_unchecked(p, q, t)), &b, &c, &d)
        }));
        assert!(panicked.is_err());
        let results: Vec<Option<Sign>> = reversed
            .iter()
            .map(|args| {
                let [a, b, c, d] = any_points(args);
                orient3d(&a, &b, &c, &d)
            })
            .collect();
        assert_eq!(results, expected_reversed);
    })
    .join()
    .unwrap();
}

thread_local! {
    /// The `used()` of the arena that the last [`Probe`] was given.
    static USED_ON_ENTRY: Cell<usize> = const { Cell::new(usize::MAX) };
}

/// A point at the origin, not explicit, that records the `used()` of the
/// arena on entry to its exact coordinates; when `panics`, it then fills
/// part of the arena and panics.
struct Probe {
    panics: bool,
}

impl Point for Probe {
    fn approx(&self) -> &[f64; 3] {
        &[0.0; 3]
    }

    fn err(&self) -> &[f64; 3] {
        &[f64::INFINITY; 3]
    }

    fn is_explicit(&self) -> bool {
        false
    }

    fn exact(&self, scratch: &mut Scratch<'_>) -> Result<ExactCoords, ExactError> {
        USED_ON_ENTRY.with(|u| u.set(scratch.used()));
        if self.panics {
            for _ in 0..1000 {
                scratch.push(&[1.0])?;
            }
            panic!("exact coordinates that panic");
        }
        [0.0; 3].exact(scratch)
    }
}

#[test]
fn the_arena_of_the_thread_is_empty_after_a_call_that_panicked() {
    std::thread::spawn(|| {
        let (a, b, c) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
        for _ in 0..3 {
            let panicked = catch_unwind(AssertUnwindSafe(|| {
                orient3d(&Probe { panics: true }, &a, &b, &c)
            }));
            assert!(panicked.is_err());
        }
        // The first point is the first to write to the arena.
        let sign = orient3d(&Probe { panics: false }, &a, &b, &c);
        assert_eq!(sign, orient3d(&[0.0; 3], &a, &b, &c));
        assert_eq!(USED_ON_ENTRY.with(Cell::get), 0);
    })
    .join()
    .unwrap();
}
