//! The checks of the contract of `Point` that the predicates run in debug
//! builds: points that break an obligation, each at every position of the
//! arguments, panic with a message that names the position and the
//! obligation; in release builds they do not. The checks take nothing from
//! the arena of the caller, and a point whose exact coordinates call a
//! predicate passes them.

// The tests that cannot run on wasm32 leave their helpers unused there.
#![cfg_attr(target_arch = "wasm32", allow(dead_code, unused_imports))]

mod common;

use std::cell::Cell;

use common::odd_points::{LIAR_AT, Liar, Lie};
use implicit_predicates::{
    BigCoords, ExactCoords, ExactError, Lnc, Point, Scratch, Sign, Violation, dihedral_sign_in,
    dot_sign_3d_in, insphere_in, insphere_sos_in, orient3d_in,
};

/// How every panic of a check of the contract begins.
const PREFIX: &str = "breaks the contract of `Point`: ";

/// Three points of the plane `x + y + z = -0.25`, which holds [`LIAR_AT`].
/// It is not parallel to an axis, on which the filter would see a zero.
const PLANE: [[f64; 3]; 3] = [[-0.25, 0.0, 0.0], [0.0, -0.25, 0.0], [0.0, 0.0, -0.25]];

/// Four points of the unit sphere centred at `(0.5, -2, 0.25)`, which holds
/// [`LIAR_AT`].
const SPHERE: [[f64; 3]; 4] = [
    [0.5, -1.0, 0.25],
    [0.5, -2.0, 1.25],
    [-0.5, -2.0, 0.25],
    [0.5, -3.0, 0.25],
];

/// An argument of a predicate: an explicit point, or a liar.
enum Arg<'a> {
    Explicit([f64; 3]),
    Liar(&'a Liar),
}

impl Point for Arg<'_> {
    fn approx(&self) -> &[f64; 3] {
        match self {
            Arg::Explicit(p) => p.approx(),
            Arg::Liar(p) => p.approx(),
        }
    }

    fn err(&self) -> &[f64; 3] {
        match self {
            Arg::Explicit(p) => p.err(),
            Arg::Liar(p) => p.err(),
        }
    }

    fn is_explicit(&self) -> bool {
        match self {
            Arg::Explicit(p) => p.is_explicit(),
            Arg::Liar(p) => p.is_explicit(),
        }
    }

    fn exact(&self, scratch: &mut Scratch<'_>) -> Result<ExactCoords, ExactError> {
        match self {
            Arg::Explicit(p) => p.exact(scratch),
            Arg::Liar(p) => p.exact(scratch),
        }
    }

    fn exact_big(&self, scratch: &mut Scratch<'_>) -> Result<BigCoords, ExactError> {
        match self {
            Arg::Explicit(p) => p.exact_big(scratch),
            Arg::Liar(p) => p.exact_big(scratch),
        }
    }
}

/// A public predicate.
// Without `std` every variant is an `_in` variant.
#[allow(clippy::enum_variant_names)]
#[derive(Clone, Copy, Debug, PartialEq)]
enum Predicate {
    #[cfg(feature = "std")]
    Orient3d,
    Orient3dIn,
    #[cfg(feature = "std")]
    Insphere,
    InsphereIn,
    #[cfg(feature = "std")]
    InsphereSos,
    InsphereSosIn,
    #[cfg(feature = "std")]
    DotSign3d,
    DotSign3dIn,
    #[cfg(feature = "std")]
    DihedralSign,
    DihedralSignIn,
}

use Predicate::*;

/// Every public predicate.
const PREDICATES: &[Predicate] = &[
    #[cfg(feature = "std")]
    Orient3d,
    Orient3dIn,
    #[cfg(feature = "std")]
    Insphere,
    InsphereIn,
    #[cfg(feature = "std")]
    InsphereSos,
    InsphereSosIn,
    #[cfg(feature = "std")]
    DotSign3d,
    DotSign3dIn,
    #[cfg(feature = "std")]
    DihedralSign,
    DihedralSignIn,
];

impl Predicate {
    /// The number of points it takes.
    fn points(self) -> usize {
        match self {
            #[cfg(feature = "std")]
            Orient3d | DihedralSign => 4,
            Orient3dIn | DihedralSignIn => 4,
            #[cfg(feature = "std")]
            DotSign3d => 3,
            DotSign3dIn => 3,
            _ => 5,
        }
    }

    /// The points of a right angle (for `dot_sign_3d`) or a right dihedral
    /// angle (for `dihedral_sign`), none of them on an axis plane of the
    /// others, as offsets from any one of them: the call places the liar at
    /// `position` and the others at the same offsets from it.
    fn right_angle(self) -> Option<Vec<[f64; 3]>> {
        match self {
            // `p - q = (1, 1, 1)` and `r - q = (1, -2, 1)` are perpendicular.
            DotSign3dIn => Some(vec![[1.0, 1.0, 1.0], [0.0; 3], [1.0, -2.0, 1.0]]),
            // The edge `d`, `c` is `(1, 1, 0)`, the normal of the triangle of
            // `a` is `(0, 1, 2) x (1, 1, 0) = (-2, 2, -1)`, and `b` is on it.
            DihedralSignIn => Some(vec![
                [0.0, 1.0, 2.0],
                [-2.0, 2.0, -1.0],
                [1.0, 1.0, 0.0],
                [0.0; 3],
            ]),
            #[cfg(feature = "std")]
            DotSign3d => DotSign3dIn.right_angle(),
            #[cfg(feature = "std")]
            DihedralSign => DihedralSignIn.right_angle(),
            _ => None,
        }
    }

    /// Calls it on the points of [`PLANE`] or [`SPHERE`] with a new liar
    /// that tells `lie` inserted at `position`, and returns the message of
    /// its panic, if it panics. Every call reaches the exact stages, since
    /// the exact coordinates of the liar are coplanar or cospherical with
    /// the others.
    fn call(self, lie: Lie, position: usize) -> Option<String> {
        let liar = Liar::new(lie);
        let p: Vec<Arg> = if let Some(offsets) = self.right_angle() {
            let shift: [f64; 3] = std::array::from_fn(|k| LIAR_AT[k] - offsets[position][k]);
            (0..offsets.len())
                .map(|i| {
                    if i == position {
                        Arg::Liar(&liar)
                    } else {
                        Arg::Explicit(std::array::from_fn(|k| offsets[i][k] + shift[k]))
                    }
                })
                .collect()
        } else {
            let mut p: Vec<Arg> = if self.points() == 4 {
                PLANE.iter().map(|&x| Arg::Explicit(x)).collect()
            } else {
                SPHERE.iter().map(|&x| Arg::Explicit(x)).collect()
            };
            p.insert(position, Arg::Liar(&liar));
            p
        };
        // The liar has the largest index, so that the first point left out
        // of a tie is another one.
        let mut idx = [0, 1, 2, 3, 4];
        idx.swap(position, 4);
        let mut buf = vec![0.0; 1 << 12];
        let mut scratch = Scratch::borrowed(&mut buf);
        panic_message(|| match self {
            #[cfg(feature = "std")]
            Orient3d => {
                implicit_predicates::orient3d(&p[0], &p[1], &p[2], &p[3]);
            }
            Orient3dIn => {
                let _ = orient3d_in(&mut scratch, &p[0], &p[1], &p[2], &p[3]);
            }
            #[cfg(feature = "std")]
            Insphere => {
                implicit_predicates::insphere(&p[0], &p[1], &p[2], &p[3], &p[4]);
            }
            InsphereIn => {
                let _ = insphere_in(&mut scratch, &p[0], &p[1], &p[2], &p[3], &p[4]);
            }
            #[cfg(feature = "std")]
            InsphereSos => {
                implicit_predicates::insphere_sos(&p[0], &p[1], &p[2], &p[3], &p[4], idx);
            }
            InsphereSosIn => {
                let _ = insphere_sos_in(&mut scratch, &p[0], &p[1], &p[2], &p[3], &p[4], idx);
            }
            #[cfg(feature = "std")]
            DotSign3d => {
                implicit_predicates::dot_sign_3d(&p[0], &p[1], &p[2]);
            }
            DotSign3dIn => {
                let _ = dot_sign_3d_in(&mut scratch, &p[0], &p[1], &p[2]);
            }
            #[cfg(feature = "std")]
            DihedralSign => {
                implicit_predicates::dihedral_sign(&p[0], &p[1], &p[2], &p[3]);
            }
            DihedralSignIn => {
                let _ = dihedral_sign_in(&mut scratch, &p[0], &p[1], &p[2], &p[3]);
            }
        })
    }

    /// Whether it runs the complete check of the contract when a call
    /// reaches the exact stages: only the predicates that take no arena.
    fn checks_fully(self) -> bool {
        match self {
            #[cfg(feature = "std")]
            Orient3d | Insphere | InsphereSos | DotSign3d | DihedralSign => true,
            _ => false,
        }
    }
}

/// The message of the panic of `f`, if it panics.
fn panic_message(f: impl FnOnce()) -> Option<String> {
    let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).err()?;
    Some(
        payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_string()))
            .unwrap_or_default(),
    )
}

/// What a predicate does in a debug build with a liar.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Outcome {
    /// It panics with a check of the contract.
    Panics,
    /// It does not panic.
    Quiet,
    /// It depends on where the liar is, which the test does not follow.
    Either,
}

/// A lie, the obligation that the message of its panic names, and what each
/// predicate does with it in a debug build.
struct Case {
    lie: Lie,
    obligation: String,
    outcome: fn(Predicate) -> Outcome,
    /// Whether no call panics in a release build. A liar whose coordinates
    /// lie above the top of the arena can make an exact stage read past its
    /// end, which panics without the message of a check.
    release_quiet: bool,
}

/// The liars that the checks without memory catch in the filter, in every
/// predicate.
fn filter_cases() -> Vec<Case> {
    vec![
        Case {
            lie: Lie::InvalidErr(0, f64::NAN),
            obligation: "an error bound is NaN or negative".into(),
            outcome: |_| Outcome::Panics,
            release_quiet: true,
        },
        Case {
            lie: Lie::InvalidErr(1, -1.0),
            obligation: "an error bound is NaN or negative".into(),
            outcome: |_| Outcome::Panics,
            release_quiet: true,
        },
        Case {
            lie: Lie::ExplicitWithErr(2),
            obligation: "says it is explicit, but its error bound is not zero".into(),
            outcome: |_| Outcome::Panics,
            release_quiet: true,
        },
    ]
}

/// The liars that the checks without memory catch in the exact stages, and
/// the complete check catches before them.
fn exact_stage_cases() -> Vec<Case> {
    let expansions = |p: Predicate| match p {
        Orient3dIn | DotSign3dIn | DihedralSignIn => Outcome::Panics,
        // In-sphere never asks for expansions.
        InsphereIn => Outcome::Quiet,
        // Only the ties ask for them, in `orient3d_in`.
        InsphereSosIn => Outcome::Either,
        #[cfg(feature = "std")]
        _ => Outcome::Panics,
    };
    vec![
        Case {
            lie: Lie::InvalidExpansion(1),
            obligation: Violation::InvalidExpansion { axis: 1 }.to_string(),
            outcome: expansions,
            release_quiet: true,
        },
        Case {
            lie: Lie::SpanAboveTop(0),
            obligation: Violation::InvalidExpansion { axis: 0 }.to_string(),
            outcome: expansions,
            release_quiet: false,
        },
        Case {
            lie: Lie::BigAboveTop,
            obligation: Violation::NonCanonicalBig { axis: 0 }.to_string(),
            // The stage over spans of the predicates that have one decides
            // first.
            outcome: |p| {
                if matches!(p, Orient3dIn | DotSign3dIn | DihedralSignIn) {
                    Outcome::Quiet
                } else {
                    Outcome::Panics
                }
            },
            release_quiet: false,
        },
    ]
}

/// The liars that only the complete check catches: the predicates that
/// take an arena, `insphere_sos_in` among them, which calls only
/// `insphere_in` and `orient3d_in`, do not.
fn complete_check_cases() -> Vec<Case> {
    let outcome = |p: Predicate| {
        if p.checks_fully() {
            Outcome::Panics
        } else {
            Outcome::Quiet
        }
    };
    vec![
        Case {
            lie: Lie::ErrTooSmall(0),
            obligation: Violation::ErrTooSmall { axis: 0 }.to_string(),
            outcome,
            release_quiet: true,
        },
        Case {
            lie: Lie::ExactMismatch(1),
            obligation: Violation::ExactMismatch { axis: 1 }.to_string(),
            outcome,
            release_quiet: true,
        },
        Case {
            lie: Lie::ApproxNotFinite(2),
            obligation: Violation::ApproxNotFinite { axis: 2 }.to_string(),
            outcome,
            release_quiet: true,
        },
        Case {
            lie: Lie::UnstableExactBig,
            obligation: Violation::Unstable.to_string(),
            outcome,
            release_quiet: true,
        },
        Case {
            lie: Lie::UnstableExact,
            obligation: Violation::Unstable.to_string(),
            outcome,
            release_quiet: true,
        },
    ]
}

/// Asserts that each case does, with its liar at every position of every
/// predicate, what its outcome says in a debug build, with a message that
/// names the position and the obligation; and that in a release build no
/// call panics.
fn assert_cases(cases: &[Case]) {
    let mut panics = 0;
    for case in cases {
        for &predicate in PREDICATES {
            for position in 0..predicate.points() {
                let message = predicate.call(case.lie, position);
                let what = format!("{:?} at {position} of {predicate:?}", case.lie);
                if cfg!(not(debug_assertions)) {
                    if case.release_quiet {
                        assert_eq!(message, None, "{what}");
                    } else if let Some(message) = message {
                        assert!(!message.starts_with(PREFIX), "{what}: {message}");
                    }
                    continue;
                }
                match (case.outcome)(predicate) {
                    Outcome::Panics => {
                        let message = message.unwrap_or_else(|| panic!("{what}: no panic"));
                        let start = format!("{PREFIX}argument {position}: ");
                        assert!(message.starts_with(&start), "{what}: {message}");
                        assert!(message.contains(&case.obligation), "{what}: {message}");
                        panics += 1;
                    }
                    Outcome::Quiet => assert_eq!(message, None, "{what}"),
                    Outcome::Either => {
                        if let Some(message) = message {
                            assert!(message.starts_with(PREFIX), "{what}: {message}");
                        }
                    }
                }
            }
        }
    }
    // Without `std`, the liars that only the complete check catches make
    // no predicate panic.
    if cfg!(all(debug_assertions, feature = "std")) {
        assert!(panics > 0);
    }
}

// Excluded on wasm32: panics abort on wasm32, so they cannot be caught there.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn a_point_with_an_invalid_error_bound_stops_in_the_filter() {
    assert_cases(&filter_cases());
}

// Excluded on wasm32: panics abort on wasm32, so they cannot be caught there.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn invalid_exact_coordinates_stop_in_the_exact_stages() {
    assert_cases(&exact_stage_cases());
}

// Excluded on wasm32: panics abort on wasm32, so they cannot be caught there.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn the_predicates_without_an_arena_check_the_whole_contract() {
    assert_cases(&complete_check_cases());
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "breaks the contract of `Point`")]
fn a_negative_error_bound_panics_in_debug_builds() {
    let liar = Liar::new(Lie::InvalidErr(0, -1.0));
    let [a, b, c] = PLANE;
    let _ = orient3d_in(&mut Scratch::borrowed(&mut [0.0; 64]), &a, &b, &c, &liar);
}

#[cfg(all(debug_assertions, feature = "std"))]
#[test]
#[should_panic(expected = "breaks the contract of `Point`")]
fn an_error_bound_too_small_panics_in_debug_builds() {
    let liar = Liar::new(Lie::ErrTooSmall(0));
    let [a, b, c, d] = SPHERE;
    implicit_predicates::insphere(&a, &b, &c, &d, &liar);
}

#[cfg(not(debug_assertions))]
#[test]
fn a_liar_does_not_panic_in_release_builds() {
    let liar = Liar::new(Lie::InvalidErr(0, -1.0));
    let [a, b, c] = PLANE;
    assert!(orient3d_in(&mut Scratch::borrowed(&mut [0.0; 64]), &a, &b, &c, &liar).is_ok());
    #[cfg(feature = "std")]
    {
        let liar = Liar::new(Lie::ErrTooSmall(0));
        let [a, b, c, d] = SPHERE;
        implicit_predicates::insphere(&a, &b, &c, &d, &liar);
    }
}

/// The smallest arena, in `f64`, with which `f` decides, and the high
/// water of an arena with room to spare after `f` decides.
fn smallest_arena(f: impl Fn(&mut Scratch<'_>) -> Option<Sign>) -> (usize, usize) {
    let smallest = (0..1 << 16)
        .find(|&n| {
            let mut buf = vec![0.0; n];
            f(&mut Scratch::borrowed(&mut buf)).is_some()
        })
        .expect("an arena of 2^16 suffices");
    let mut buf = vec![0.0; 1 << 16];
    let mut scratch = Scratch::borrowed(&mut buf);
    assert!(f(&mut scratch).is_some());
    (smallest, scratch.high_water())
}

/// `point`, counting the calls to `exact` and to `exact_big`.
struct Counted<P> {
    point: P,
    calls: Cell<[usize; 2]>,
}

impl<P: Point> Counted<P> {
    fn new(point: P) -> Self {
        Counted {
            point,
            calls: Cell::new([0; 2]),
        }
    }

    /// The calls to `exact` and to `exact_big` since the last time.
    fn take(&self) -> [usize; 2] {
        self.calls.replace([0; 2])
    }

    fn count(&self, method: usize) {
        let mut calls = self.calls.get();
        calls[method] += 1;
        self.calls.set(calls);
    }
}

impl<P: Point> Point for Counted<P> {
    fn approx(&self) -> &[f64; 3] {
        self.point.approx()
    }

    fn err(&self) -> &[f64; 3] {
        self.point.err()
    }

    fn is_explicit(&self) -> bool {
        self.point.is_explicit()
    }

    fn exact(&self, scratch: &mut Scratch<'_>) -> Result<ExactCoords, ExactError> {
        self.count(0);
        self.point.exact(scratch)
    }

    fn exact_big(&self, scratch: &mut Scratch<'_>) -> Result<BigCoords, ExactError> {
        self.count(1);
        self.point.exact_big(scratch)
    }
}

/// The checks of debug builds write nothing in the arena of the caller, not
/// even above its top, and ask nothing of the points through it: the
/// smallest arena with which a call decides, the high water of an arena
/// with room to spare, and the calls to `exact` and `exact_big` of the
/// point on a line in one call, all measured in a release build, are the
/// same in a debug build.
#[test]
fn a_debug_check_takes_nothing_from_the_arena_of_the_caller() {
    let (a, b, c) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
    let m = Counted::new(Lnc::new(a, b, 0.1).unwrap());
    let orient3d = smallest_arena(|s| orient3d_in(s, &a, &b, &c, &m).ok());
    assert_eq!(orient3d, (ORIENT3D_SMALLEST_ARENA, ORIENT3D_SMALLEST_ARENA));
    m.take();
    let mut buf = vec![0.0; 1 << 16];
    assert!(orient3d_in(&mut Scratch::borrowed(&mut buf), &a, &b, &c, &m).is_ok());
    assert_eq!(m.take(), ORIENT3D_CALLS);
    let d = [-1.0, 0.0, 0.0];
    let e = Counted::new(Lnc::new(a, b, 1.0).unwrap());
    let insphere = smallest_arena(|s| insphere_in(s, &a, &b, &c, &d, &e).ok());
    assert_eq!(insphere, (INSPHERE_SMALLEST_ARENA, INSPHERE_SMALLEST_ARENA));
    e.take();
    assert!(insphere_in(&mut Scratch::borrowed(&mut buf), &a, &b, &c, &d, &e).is_ok());
    assert_eq!(e.take(), INSPHERE_CALLS);
    let sos = smallest_arena(|s| insphere_sos_in(s, &a, &b, &c, &d, &e, [4, 3, 2, 1, 0]).ok());
    assert_eq!(
        sos,
        (INSPHERE_SOS_SMALLEST_ARENA, INSPHERE_SOS_SMALLEST_ARENA)
    );
    e.take();
    let mut scratch = Scratch::borrowed(&mut buf);
    assert!(insphere_sos_in(&mut scratch, &a, &b, &c, &d, &e, [4, 3, 2, 1, 0]).is_ok());
    assert_eq!(e.take(), INSPHERE_SOS_CALLS);
}

/// The calls to `exact` and to `exact_big` of the point on a line in the
/// calls of [`a_debug_check_takes_nothing_from_the_arena_of_the_caller`],
/// measured in a release build.
const ORIENT3D_CALLS: [usize; 2] = [1, 0];
const INSPHERE_CALLS: [usize; 2] = [0, 1];
const INSPHERE_SOS_CALLS: [usize; 2] = [0, 1];

/// The smallest arenas of [`a_debug_check_takes_nothing_from_the_arena_of_the_caller`],
/// measured in a release build, which are also the high waters there.
const ORIENT3D_SMALLEST_ARENA: usize = 39;
const INSPHERE_SMALLEST_ARENA: usize = 24;
const INSPHERE_SOS_SMALLEST_ARENA: usize = 24;

/// A point whose exact coordinates are those of [`LIAR_AT`], written after
/// calls to `orient3d` and `insphere` with implicit points, which reach
/// their exact stages, and so their checks in debug builds, while the
/// predicate that asked for the coordinates runs.
#[cfg(feature = "std")]
struct Reentrant;

#[cfg(feature = "std")]
impl Reentrant {
    /// Calls the predicates.
    fn call_predicates() {
        let (a, b, c) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
        let m = Lnc::new(a, b, 0.1).unwrap();
        assert_eq!(implicit_predicates::orient3d(&a, &b, &c, &m), Sign::Zero);
        let d = [-1.0, 0.0, 0.0];
        let e = Lnc::new(a, b, 1.0).unwrap();
        assert_eq!(
            implicit_predicates::insphere(&a, &b, &c, &d, &e),
            Sign::Zero
        );
    }
}

#[cfg(feature = "std")]
impl Point for Reentrant {
    fn approx(&self) -> &[f64; 3] {
        &LIAR_AT
    }

    fn err(&self) -> &[f64; 3] {
        &[f64::INFINITY; 3]
    }

    fn is_explicit(&self) -> bool {
        false
    }

    fn exact(&self, scratch: &mut Scratch<'_>) -> Result<ExactCoords, ExactError> {
        Reentrant::call_predicates();
        LIAR_AT.exact(scratch)
    }

    fn exact_big(&self, scratch: &mut Scratch<'_>) -> Result<BigCoords, ExactError> {
        Reentrant::call_predicates();
        LIAR_AT.exact_big(scratch)
    }
}

#[cfg(feature = "std")]
#[test]
fn a_point_that_calls_a_predicate_passes_the_debug_check() {
    let [a, b, c] = PLANE;
    assert_eq!(
        implicit_predicates::orient3d(&a, &b, &c, &Reentrant),
        Sign::Zero
    );
    let [a, b, c, d] = SPHERE;
    assert_eq!(
        implicit_predicates::insphere(&a, &b, &c, &d, &Reentrant),
        Sign::Zero
    );
    assert_ne!(
        implicit_predicates::insphere_sos(&a, &b, &c, &d, &Reentrant, [1, 2, 3, 4, 0]),
        Sign::Zero
    );
    let mut scratch = Scratch::growable(0);
    assert_eq!(
        insphere_in(&mut scratch, &a, &b, &c, &d, &Reentrant),
        Ok(Sign::Zero)
    );
}
