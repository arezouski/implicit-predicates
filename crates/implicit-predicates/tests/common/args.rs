//! The arguments of a call with implicit points: each an explicit point or a
//! point on a line, with its exact coordinates; for `orient3d`, the calls of
//! the chain on them and the condition under which the chain is documented
//! to decide; for `insphere`, the five points of a call of any concrete
//! types.

use implicit_predicates::stages::orient3d as stages;
use implicit_predicates::{AnyPoint, Bpt, Explicit, Lnc, Point, Scratch, Sign, Weight};
use implicit_predicates_oracle::Dyadic;

use super::bpt::exact_bpt_by_hand;
use super::generated::exact_at;
use super::point_checks::exact_lnc;

/// One argument: an explicit point, `Lnc { p, q, t }`, the point
/// `q + t (p - q)`, or `Bpt { p, q, r, u, v }`, the point
/// `r + u (p - r) + v (q - r)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Arg {
    Explicit([f64; 3]),
    Lnc([f64; 3], [f64; 3], f64),
    Bpt([f64; 3], [f64; 3], [f64; 3], f64, f64),
}

/// The four arguments of `orient3d`.
pub type Args = [Arg; 4];

impl Arg {
    /// The point, as an [`AnyPoint`], built without validation.
    pub fn any(&self) -> AnyPoint {
        match *self {
            Arg::Explicit(p) => AnyPoint::Explicit(Explicit::new_unchecked(p)),
            Arg::Lnc(p, q, t) => AnyPoint::Lnc(Lnc::new_unchecked(p, q, t)),
            Arg::Bpt(p, q, r, u, v) => AnyPoint::Bpt(Bpt::new_unchecked(p, q, r, u, v)),
        }
    }

    /// The inputs of the point, in the order of its declaration.
    pub fn inputs(&self) -> Vec<f64> {
        match *self {
            Arg::Explicit(p) => p.to_vec(),
            Arg::Lnc(p, q, t) => p.into_iter().chain(q).chain([t]).collect(),
            Arg::Bpt(p, q, r, u, v) => p.into_iter().chain(q).chain(r).chain([u, v]).collect(),
        }
    }

    /// The point with its inputs replaced by `inputs`, in the order of
    /// [`Arg::inputs`].
    pub fn with_inputs(&self, inputs: &[f64]) -> Arg {
        let point = |i: usize| [inputs[i], inputs[i + 1], inputs[i + 2]];
        match self {
            Arg::Explicit(_) => Arg::Explicit(point(0)),
            Arg::Lnc(..) => Arg::Lnc(point(0), point(3), inputs[6]),
            Arg::Bpt(..) => Arg::Bpt(point(0), point(3), point(6), inputs[9], inputs[10]),
        }
    }

    /// Whether every input is finite.
    pub fn is_finite(&self) -> bool {
        self.inputs().iter().all(|x| x.is_finite())
    }

    /// The exact coordinates of the point.
    ///
    /// # Panics
    ///
    /// Panics if an input is not finite.
    pub fn exact(&self) -> [Dyadic; 3] {
        match *self {
            Arg::Explicit(p) => p.map(Dyadic::from_f64),
            Arg::Lnc(p, q, t) => exact_lnc([p, q], [t]),
            Arg::Bpt(p, q, r, u, v) => exact_bpt_by_hand(&p, &q, &r, u, v),
        }
    }
}

/// The exact sign of `orient3d` at the points of `args`, which must have
/// finite inputs.
pub fn exact_sign(args: &Args) -> Sign {
    exact_at(&args.each_ref().map(Arg::exact))
}

/// A call of the chain on points of any type, given to [`with_concrete`].
pub trait Call {
    type Output;
    fn call<A: Point, B: Point, C: Point, D: Point>(
        &mut self,
        a: &A,
        b: &B,
        c: &C,
        d: &D,
    ) -> Self::Output;
}

/// Builds the point of `$arg` as its concrete type, `[f64; 3]` or
/// `Explicit` for an explicit point as `$raw` says, `Lnc` or `Bpt` otherwise,
/// and evaluates `$body` with it bound to `$x`.
macro_rules! concrete {
    ($arg:expr, $raw:expr, |$x:ident| $body:expr) => {
        match $arg {
            Arg::Explicit(p) if $raw => {
                let $x = &p;
                $body
            }
            Arg::Explicit(p) => {
                let $x = &Explicit::new_unchecked(p);
                $body
            }
            Arg::Lnc(p, q, t) => {
                let $x = &Lnc::new_unchecked(p, q, t);
                $body
            }
            Arg::Bpt(p, q, r, u, v) => {
                let $x = &Bpt::new_unchecked(p, q, r, u, v);
                $body
            }
        }
    };
}

/// `f` on the points of `args`, each of its concrete type: an explicit
/// point is a `[f64; 3]` when `raw` is true and an [`Explicit`] otherwise.
pub fn with_concrete<F: Call>(args: &Args, raw: bool, f: &mut F) -> F::Output {
    concrete!(args[0], raw, |a| concrete!(args[1], raw, |b| concrete!(
        args[2],
        raw,
        |c| concrete!(args[3], raw, |d| f.call(a, b, c, d))
    )))
}

/// `orient3d` on points of any type.
#[cfg(feature = "std")]
pub struct Orient3d;

#[cfg(feature = "std")]
impl Call for Orient3d {
    type Output = Sign;
    fn call<A: Point, B: Point, C: Point, D: Point>(&mut self, a: &A, b: &B, c: &C, d: &D) -> Sign {
        implicit_predicates::orient3d(a, b, c, d)
    }
}

/// `orient3d_in` on points of any type, with an arena large enough for
/// every call of the tests; checks that the arena is left as it was.
pub struct Orient3dIn;

impl Call for Orient3dIn {
    type Output = Sign;
    fn call<A: Point, B: Point, C: Point, D: Point>(&mut self, a: &A, b: &B, c: &C, d: &D) -> Sign {
        let mut buf = vec![0.0; 1 << 16];
        let mut scratch = Scratch::borrowed(&mut buf);
        let result = implicit_predicates::orient3d_in(&mut scratch, a, b, c, d);
        assert_eq!(scratch.used(), 0);
        result.expect("a large enough arena")
    }
}

/// The points of `args` as [`AnyPoint`]s.
pub fn any_points<const N: usize>(args: &[Arg; N]) -> [AnyPoint; N] {
    args.each_ref().map(Arg::any)
}

/// A call on five points of any type, given to [`with_concrete5`].
pub trait Call5 {
    type Output;
    fn call<A: Point, B: Point, C: Point, D: Point, E: Point>(
        &mut self,
        a: &A,
        b: &B,
        c: &C,
        d: &D,
        e: &E,
    ) -> Self::Output;
}

/// `f` on the five points of `args`, each of its concrete type, as
/// [`with_concrete`] builds them.
pub fn with_concrete5<F: Call5>(args: &[Arg; 5], raw: bool, f: &mut F) -> F::Output {
    concrete!(args[0], raw, |a| concrete!(args[1], raw, |b| concrete!(
        args[2],
        raw,
        |c| concrete!(args[3], raw, |d| concrete!(args[4], raw, |e| f
            .call(a, b, c, d, e)))
    )))
}

/// Every way of running the chain or one of its stages on `args`, by name:
/// the filter on the points, the stages on the stack (when every point is
/// explicit), the stage over spans on their exact coordinates (when every
/// point has them), `exact_stages_in`, `staged_in`,
/// `orient3d` and `orient3d_in` on `AnyPoint`s and on the concrete types.
/// The stages that always decide give `Some`, so that every result has one
/// type.
#[cfg(feature = "std")]
pub fn every_stage(args: &Args) -> Vec<(&'static str, Option<Sign>)> {
    let [a, b, c, d] = any_points(args);
    let mut buf = vec![0.0; 1 << 16];
    let mut scratch = Scratch::borrowed(&mut buf);
    let mut results = vec![
        ("filter_points", stages::filter_points(&a, &b, &c, &d)),
        (
            "staged_in",
            Some(stages::staged_in(&mut scratch, &a, &b, &c, &d).unwrap()),
        ),
        (
            "exact_stages_in",
            Some(stages::exact_stages_in(&mut scratch, &a, &b, &c, &d).unwrap()),
        ),
    ];
    assert_eq!(scratch.used(), 0);
    if all_explicit(args) {
        let p = args.map(|arg| arg.inputs());
        let p: [[f64; 3]; 4] = std::array::from_fn(|i| [p[i][0], p[i][1], p[i][2]]);
        results.push(("exact_stages", stages::exact_stages(p.each_ref())));
    }
    let exact: Option<Vec<_>> = [a, b, c, d]
        .iter()
        .map(|p| p.exact(&mut scratch).ok())
        .collect();
    if let Some(q) = exact {
        let used = scratch.used();
        let q = [&q[0], &q[1], &q[2], &q[3]];
        let translated = stages::expansions_translated_in(&mut scratch, q).unwrap();
        assert_eq!(scratch.used(), used);
        results.push(("expansions_translated_in", translated));
    }
    results.extend(
        [
            ("orient3d", implicit_predicates::orient3d(&a, &b, &c, &d)),
            ("orient3d_in", Orient3dIn.call(&a, &b, &c, &d)),
            (
                "orient3d, concrete",
                with_concrete(args, false, &mut Orient3d),
            ),
            ("orient3d, arrays", with_concrete(args, true, &mut Orient3d)),
            (
                "orient3d_in, concrete",
                with_concrete(args, false, &mut Orient3dIn),
            ),
        ]
        .map(|(name, sign)| (name, Some(sign))),
    );
    results
}

/// Whether a stage before the one on big integers decides, in the results
/// of [`every_stage`]: the filter, the stages on the stack or the stage over
/// spans.
pub fn decided_on_expansions(results: &[(&str, Option<Sign>)]) -> bool {
    results.iter().any(|(name, result)| {
        ["filter_points", "exact_stages", "expansions_translated_in"].contains(name)
            && result.is_some()
    })
}

/// Whether every explicit argument is given exactly by its coordinates.
pub fn all_explicit(args: &Args) -> bool {
    args.iter().all(|a| matches!(a, Arg::Explicit(_)))
}

/// Whether the stages over spans are documented to decide on `args`: every
/// point has exact coordinates (its inputs are finite and in the range of
/// its own exact arithmetic) of weight one, with at most
/// `MAX_COORD_COMPONENTS` components each, every one zero or of a magnitude
/// in `SPANS_TRANSLATED_RANGE`.
pub fn spans_decidable(args: &Args) -> bool {
    let mut buf = vec![0.0; 1 << 12];
    let mut scratch = Scratch::borrowed(&mut buf);
    args.iter().all(|arg| match arg.any().exact(&mut scratch) {
        Ok(q) => {
            q.w == Weight::One
                && [q.x, q.y, q.z].iter().all(|&s| {
                    let e = scratch.get(s);
                    e.len() <= stages::MAX_COORD_COMPONENTS
                        && e.iter()
                            .all(|&x| super::in_range(x, stages::SPANS_TRANSLATED_RANGE))
                })
        }
        Err(_) => false,
    })
}

/// Whether the chain decides on `args` without the stage on big integers:
/// for four explicit points, every coordinate is zero or of a magnitude in
/// `TRANSLATED_RANGE`, which rules out values that are not finite;
/// otherwise [`spans_decidable`].
pub fn decidable(args: &Args) -> bool {
    if all_explicit(args) {
        args.iter().all(|a| {
            a.inputs()
                .iter()
                .all(|&x| super::in_range(x, stages::TRANSLATED_RANGE))
        })
    } else {
        spans_decidable(args)
    }
}
