//! The arguments of a call with implicit points: each an explicit point or a
//! point on a line, with its exact coordinates, the calls of the chain on
//! them, and the condition under which the chain is documented to decide.

use implicit_predicates::stages::orient3d as stages;
use implicit_predicates::{AnyPoint, Explicit, Lnc, Point, Scratch, Sign, Weight};
use implicit_predicates_oracle::Dyadic;

use super::generated::exact_at;
use super::point_checks::exact_lnc;

/// One argument: an explicit point, or `Lnc { p, q, t }`, the point
/// `q + t (p - q)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Arg {
    Explicit([f64; 3]),
    Lnc([f64; 3], [f64; 3], f64),
}

/// The four arguments of `orient3d`.
pub type Args = [Arg; 4];

impl Arg {
    /// The point, as an [`AnyPoint`], built without validation.
    pub fn any(&self) -> AnyPoint {
        match *self {
            Arg::Explicit(p) => AnyPoint::Explicit(Explicit::new_unchecked(p)),
            Arg::Lnc(p, q, t) => AnyPoint::Lnc(Lnc::new_unchecked(p, q, t)),
        }
    }

    /// The inputs of the point, in the order of its declaration.
    pub fn inputs(&self) -> Vec<f64> {
        match *self {
            Arg::Explicit(p) => p.to_vec(),
            Arg::Lnc(p, q, t) => p.into_iter().chain(q).chain([t]).collect(),
        }
    }

    /// The point with its inputs replaced by `inputs`, in the order of
    /// [`Arg::inputs`].
    pub fn with_inputs(&self, inputs: &[f64]) -> Arg {
        let point = |i: usize| [inputs[i], inputs[i + 1], inputs[i + 2]];
        match self {
            Arg::Explicit(_) => Arg::Explicit(point(0)),
            Arg::Lnc(..) => Arg::Lnc(point(0), point(3), inputs[6]),
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
/// `Explicit` for an explicit point as `$raw` says, and `Lnc` otherwise,
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
pub struct Orient3d;

impl Call for Orient3d {
    type Output = Option<Sign>;
    fn call<A: Point, B: Point, C: Point, D: Point>(
        &mut self,
        a: &A,
        b: &B,
        c: &C,
        d: &D,
    ) -> Option<Sign> {
        implicit_predicates::orient3d(a, b, c, d)
    }
}

/// `orient3d_in` on points of any type, with an arena large enough for
/// every call of the tests; checks that the arena is left as it was.
pub struct Orient3dIn;

impl Call for Orient3dIn {
    type Output = Option<Sign>;
    fn call<A: Point, B: Point, C: Point, D: Point>(
        &mut self,
        a: &A,
        b: &B,
        c: &C,
        d: &D,
    ) -> Option<Sign> {
        let mut buf = vec![0.0; 1 << 16];
        let mut scratch = Scratch::borrowed(&mut buf);
        let result = implicit_predicates::orient3d_in(&mut scratch, a, b, c, d);
        assert_eq!(scratch.used(), 0);
        result.expect("a large enough arena")
    }
}

/// The points of `args` as [`AnyPoint`]s.
pub fn any_points(args: &Args) -> [AnyPoint; 4] {
    args.each_ref().map(Arg::any)
}

/// Every way of running the chain or one of its stages on `args`, by name:
/// the filter on the points, the stages over spans on their exact
/// coordinates (when every point has them), `staged_in`, `orient3d` and
/// `orient3d_in` on `AnyPoint`s and on the concrete types.
pub fn every_stage(args: &Args) -> Vec<(&'static str, Option<Sign>)> {
    let [a, b, c, d] = any_points(args);
    let mut buf = vec![0.0; 1 << 16];
    let mut scratch = Scratch::borrowed(&mut buf);
    let mut results = vec![
        ("filter_points", stages::filter_points(&a, &b, &c, &d)),
        (
            "staged_in",
            stages::staged_in(&mut scratch, &a, &b, &c, &d).unwrap(),
        ),
        (
            "exact_stages_in",
            stages::exact_stages_in(&mut scratch, &a, &b, &c, &d).unwrap(),
        ),
    ];
    assert_eq!(scratch.used(), 0);
    let exact: Option<Vec<_>> = [a, b, c, d]
        .iter()
        .map(|p| p.exact(&mut scratch).ok())
        .collect();
    if let Some(q) = exact {
        let used = scratch.used();
        let q = [&q[0], &q[1], &q[2], &q[3]];
        let translated = stages::expansions_translated_in(&mut scratch, q).unwrap();
        let untranslated = stages::expansions_untranslated_in(&mut scratch, q).unwrap();
        assert_eq!(scratch.used(), used);
        results.push(("expansions_translated_in", translated));
        results.push(("expansions_untranslated_in", untranslated));
    }
    results.extend([
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
    ]);
    results
}

/// Whether every explicit argument is given exactly by its coordinates.
pub fn all_explicit(args: &Args) -> bool {
    args.iter().all(|a| matches!(a, Arg::Explicit(_)))
}

/// Whether the stages over spans are documented to decide on `args`: every
/// point has exact coordinates (its inputs are finite and in the range of
/// its own exact arithmetic) of weight one, with at most
/// `MAX_COORD_COMPONENTS` components each, every one zero or of a magnitude
/// in `SPANS_UNTRANSLATED_RANGE`.
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
                            .all(|&x| super::in_range(x, stages::SPANS_UNTRANSLATED_RANGE))
                })
        }
        Err(_) => false,
    })
}

/// Whether the chain is documented to decide on `args`: for four explicit
/// points, every coordinate is zero or of a magnitude in
/// `UNTRANSLATED_RANGE`, which rules out values that are not finite;
/// otherwise [`spans_decidable`].
pub fn decidable(args: &Args) -> bool {
    if all_explicit(args) {
        args.iter().all(|a| {
            a.inputs()
                .iter()
                .all(|&x| super::in_range(x, stages::UNTRANSLATED_RANGE))
        })
    } else {
        spans_decidable(args)
    }
}
