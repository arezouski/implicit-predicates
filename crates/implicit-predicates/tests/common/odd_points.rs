//! Points of types made for the tests, which no point of this crate can
//! be: exact coordinates with a weight other than one, no exact coordinates
//! at all, an exact coordinate with any number of components, no exact
//! coordinates as expansions, and a point that breaks one obligation of the
//! contract of `Point` ([`Liar`]).

use implicit_predicates::{BigCoords, ExactCoords, ExactError, Mark, Point, Scratch, Span, Weight};
use implicit_predicates_oracle::Dyadic;

/// A point at the origin whose exact coordinates have the weight
/// `Weight::Expansion`, with an infinite error so that the filter never
/// decides.
pub struct Weighted;

impl Point for Weighted {
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
        let zero = scratch.push(&[])?;
        let one = scratch.push(&[1.0])?;
        Ok(ExactCoords {
            x: zero,
            y: zero,
            z: zero,
            w: Weight::Expansion(one),
        })
    }

    fn exact_big(&self, _: &mut Scratch<'_>) -> Result<BigCoords, ExactError> {
        Err(ExactError::Weighted)
    }
}

/// A point at the origin whose exact coordinates are outside the range of
/// the expansions and have a weight other than one, so that only the stage
/// on big integers can see the weight, with an infinite error so that the
/// filter never decides.
pub struct WeightedOutOfRange;

impl Point for WeightedOutOfRange {
    fn approx(&self) -> &[f64; 3] {
        &[0.0; 3]
    }

    fn err(&self) -> &[f64; 3] {
        &[f64::INFINITY; 3]
    }

    fn is_explicit(&self) -> bool {
        false
    }

    fn exact(&self, _: &mut Scratch<'_>) -> Result<ExactCoords, ExactError> {
        Err(ExactError::OutOfRange)
    }

    fn exact_big(&self, _: &mut Scratch<'_>) -> Result<BigCoords, ExactError> {
        Err(ExactError::Weighted)
    }
}

/// A point at the origin without exact coordinates, as expansions or as big
/// integers, with an infinite error so that the filter never decides.
pub struct NoCoordinates;

impl Point for NoCoordinates {
    fn approx(&self) -> &[f64; 3] {
        &[0.0; 3]
    }

    fn err(&self) -> &[f64; 3] {
        &[f64::INFINITY; 3]
    }

    fn is_explicit(&self) -> bool {
        false
    }

    fn exact(&self, _: &mut Scratch<'_>) -> Result<ExactCoords, ExactError> {
        Err(ExactError::OutOfRange)
    }

    fn exact_big(&self, _: &mut Scratch<'_>) -> Result<BigCoords, ExactError> {
        Err(ExactError::OutOfRange)
    }
}

/// A point whose first coordinate is the sum of `components`, a valid
/// expansion, with an infinite error so that the filter never decides; it
/// counts the calls to `exact` and to `exact_big`.
pub struct Long {
    components: Vec<f64>,
    approx: [f64; 3],
    pub exact_calls: std::cell::Cell<usize>,
    pub big_calls: std::cell::Cell<usize>,
}

impl Long {
    /// The point `(sum of components, y, z)`.
    pub fn new(components: Vec<f64>, y: f64, z: f64) -> Long {
        let x = components.iter().rev().sum();
        Long {
            components,
            approx: [x, y, z],
            exact_calls: std::cell::Cell::new(0),
            big_calls: std::cell::Cell::new(0),
        }
    }

    /// The exact coordinates of the point.
    pub fn exact_value(&self) -> [Dyadic; 3] {
        let x = self
            .components
            .iter()
            .fold(Dyadic::from_f64(0.0), |acc, &c| &acc + &Dyadic::from_f64(c));
        [
            x,
            Dyadic::from_f64(self.approx[1]),
            Dyadic::from_f64(self.approx[2]),
        ]
    }
}

impl Point for Long {
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
        self.exact_calls.set(self.exact_calls.get() + 1);
        fn push(scratch: &mut Scratch<'_>, e: &[f64]) -> Result<Span, ExactError> {
            Ok(scratch.push(e)?)
        }
        let mark = scratch.mark();
        // A zero coordinate is the empty expansion.
        fn nonzero(x: &[f64]) -> &[f64] {
            if x[0] == 0.0 { &[] } else { x }
        }
        let spans = push(scratch, &self.components).and_then(|x| {
            let y = push(scratch, nonzero(&self.approx[1..2]))?;
            Ok((x, y, push(scratch, nonzero(&self.approx[2..]))?))
        });
        let (x, y, z) = spans.inspect_err(|_| scratch.reset(mark))?;
        Ok(ExactCoords {
            x,
            y,
            z,
            w: Weight::One,
        })
    }

    fn exact_big(&self, scratch: &mut Scratch<'_>) -> Result<BigCoords, ExactError> {
        self.big_calls.set(self.big_calls.get() + 1);
        let mut x = scratch.big_from_f64(0.0)?;
        for &c in &self.components {
            let c = scratch.big_from_f64(c)?;
            x = scratch.big_add(x, c)?;
        }
        let y = scratch.big_from_f64(self.approx[1])?;
        let z = scratch.big_from_f64(self.approx[2])?;
        Ok(BigCoords::new(x, y, z))
    }
}

/// The point `point`, whose `Point::exact` panics, explicit or not as
/// `explicit` says, without error: a chain that does not ask for exact
/// coordinates as expansions takes it, and gets its coordinates as big
/// integers from `exact_big` of `point`.
pub struct NoExpansions {
    pub point: [f64; 3],
    pub explicit: bool,
}

impl Point for NoExpansions {
    fn approx(&self) -> &[f64; 3] {
        &self.point
    }

    fn err(&self) -> &[f64; 3] {
        &[0.0; 3]
    }

    fn is_explicit(&self) -> bool {
        self.explicit
    }

    fn exact(&self, _: &mut Scratch<'_>) -> Result<ExactCoords, ExactError> {
        panic!("`Point::exact` called on {:?}", self.point)
    }

    fn exact_big(&self, scratch: &mut Scratch<'_>) -> Result<BigCoords, ExactError> {
        self.point.exact_big(scratch)
    }
}

/// A way for [`Liar`] to break the contract of `Point`, each the
/// obligation of one variant of `Violation`, on one axis when it has one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Lie {
    /// `err` on the axis is this value, NaN or negative.
    InvalidErr(usize, f64),
    /// `approx` on the axis is infinite, with a finite `err`.
    ApproxNotFinite(usize),
    /// `exact_big` fails with `OutOfRange`.
    NotFinite,
    /// `exact_big` fails with `Weighted`.
    WeightedBig,
    /// `exact` gives a weight other than one, and `exact_big` succeeds.
    WeightedExpansions,
    /// `approx` on the axis is further from the coordinate than `err`.
    ErrTooSmall(usize),
    /// `is_explicit` is `true` with an error on the axis.
    ExplicitWithErr(usize),
    /// `exact` gives a span that is not a valid expansion on the axis.
    InvalidExpansion(usize),
    /// `exact` and `exact_big` give different values on the axis.
    ExactMismatch(usize),
    /// `approx` changes from one call to the next.
    UnstableApprox,
    /// `err` changes from one call to the next.
    UnstableErr,
    /// `is_explicit` changes from one call to the next.
    UnstableExplicit,
    /// `exact_big` changes from one call to the next.
    UnstableExactBig,
    /// `exact` changes from one call to the next.
    UnstableExact,
    /// `exact` fails with `OutOfRange` after the first call.
    ExactLeavesRange,
    /// The first call to `exact_big` resets the arena to the mark given to
    /// [`Liar::lowering_to`], below the top it was given, and writes the
    /// coordinates there.
    LowersTopInFirstBig,
    /// `exact` resets the arena to the top that the first call to
    /// `exact_big` was given, below the top it was given itself, and writes
    /// the coordinates there.
    LowersTopInExact,
    /// The second call to `exact_big` resets the arena to the top that the
    /// first call was given and writes the coordinates there again.
    LowersTopInSecondBig,
    /// `exact` gives, for the axis, a span it released with a reset: above
    /// the top.
    SpanAboveTop(usize),
    /// The first call to `exact_big` releases the coordinates it wrote, so
    /// that they lie above the top.
    BigAboveTop,
    /// The second call to `exact_big` releases the coordinates it wrote.
    StaleSecondBig,
}

/// The explicit point [`LIAR_AT`], which keeps the contract of `Point`
/// except for one lie.
pub struct Liar {
    lie: Lie,
    approx: [[f64; 3]; 2],
    err: [[f64; 3]; 2],
    calls: std::cell::Cell<usize>,
    big_calls: std::cell::Cell<usize>,
    exact_calls: std::cell::Cell<usize>,
    mark: std::cell::Cell<Option<Mark>>,
}

/// The coordinates of every [`Liar`].
pub const LIAR_AT: [f64; 3] = [1.5, -2.0, 0.25];

impl Liar {
    /// The point that tells `lie`.
    pub fn new(lie: Lie) -> Liar {
        let mut approx = LIAR_AT;
        let mut err = [0.0; 3];
        match lie {
            Lie::InvalidErr(axis, e) => err[axis] = e,
            Lie::ApproxNotFinite(axis) => approx[axis] = f64::INFINITY,
            // Two units in the last place off, with a bound of one.
            Lie::ErrTooSmall(axis) => {
                approx[axis] = LIAR_AT[axis].next_up().next_up();
                err[axis] = LIAR_AT[axis].next_up() - LIAR_AT[axis];
            }
            Lie::ExplicitWithErr(axis) => err[axis] = 2f64.powi(-60),
            _ => {}
        }
        let mut second = (approx, err);
        match lie {
            Lie::UnstableApprox => second.0[1] = LIAR_AT[1].next_up(),
            Lie::UnstableErr => second.1[2] = 1.0,
            _ => {}
        }
        Liar {
            lie,
            approx: [approx, second.0],
            err: [err, second.1],
            calls: std::cell::Cell::new(0),
            big_calls: std::cell::Cell::new(0),
            exact_calls: std::cell::Cell::new(0),
            mark: std::cell::Cell::new(None),
        }
    }

    /// The point that tells [`Lie::LowersTopInFirstBig`], down to `mark`.
    pub fn lowering_to(mark: Mark) -> Liar {
        let liar = Liar::new(Lie::LowersTopInFirstBig);
        liar.mark.set(Some(mark));
        liar
    }

    /// The mark saved, or the current top when there is none.
    fn saved(&self, scratch: &Scratch<'_>) -> Mark {
        self.mark.get().unwrap_or_else(|| scratch.mark())
    }

    /// Counts a call and returns whether it is a later one, for the lies
    /// that change from one call to the next.
    fn later(&self) -> usize {
        let n = self.calls.get();
        self.calls.set(n + 1);
        usize::from(n > 0)
    }
}

impl Point for Liar {
    fn approx(&self) -> &[f64; 3] {
        let i = if self.lie == Lie::UnstableApprox {
            self.later()
        } else {
            0
        };
        &self.approx[i]
    }

    fn err(&self) -> &[f64; 3] {
        let i = if self.lie == Lie::UnstableErr {
            self.later()
        } else {
            0
        };
        &self.err[i]
    }

    fn is_explicit(&self) -> bool {
        match self.lie {
            Lie::ExplicitWithErr(_) => true,
            Lie::UnstableExplicit => self.later() == 0,
            _ => false,
        }
    }

    fn exact(&self, scratch: &mut Scratch<'_>) -> Result<ExactCoords, ExactError> {
        let later = self.exact_calls.get() > 0;
        self.exact_calls.set(self.exact_calls.get() + 1);
        match self.lie {
            Lie::UnstableExact if later => return [LIAR_AT[0], LIAR_AT[1], 0.5].exact(scratch),
            Lie::ExactLeavesRange if later => return Err(ExactError::OutOfRange),
            _ => {}
        }
        let coords = LIAR_AT.exact(scratch)?;
        let mut spans = [coords.x, coords.y, coords.z];
        let mut w = Weight::One;
        match self.lie {
            Lie::WeightedExpansions => w = Weight::Expansion(scratch.push(&[1.0])?),
            // The span of two components outlives a reset, and two pushes
            // of one component each write them in decreasing order: 1.5 as
            // [1.0, 0.5].
            Lie::InvalidExpansion(axis) => {
                let mark = scratch.mark();
                let stale = scratch.push(&[0.5, 1.0])?;
                scratch.reset(mark);
                scratch.push(&[1.0])?;
                scratch.push(&[0.5])?;
                spans[axis] = stale;
            }
            Lie::ExactMismatch(axis) => {
                spans[axis] = scratch.push(&[LIAR_AT[axis] * 2.0])?;
            }
            Lie::LowersTopInExact => {
                scratch.reset(self.saved(scratch));
                let coords = LIAR_AT.exact(scratch)?;
                spans = [coords.x, coords.y, coords.z];
            }
            Lie::SpanAboveTop(axis) => {
                let mark = scratch.mark();
                spans[axis] = scratch.push(&[LIAR_AT[axis]])?;
                scratch.reset(mark);
            }
            _ => {}
        }
        Ok(ExactCoords {
            x: spans[0],
            y: spans[1],
            z: spans[2],
            w,
        })
    }

    fn exact_big(&self, scratch: &mut Scratch<'_>) -> Result<BigCoords, ExactError> {
        let first = self.big_calls.get() == 0;
        self.big_calls.set(self.big_calls.get() + 1);
        if first && self.mark.get().is_none() {
            self.mark.set(Some(scratch.mark()));
        }
        match self.lie {
            Lie::LowersTopInFirstBig if first => scratch.reset(self.saved(scratch)),
            Lie::LowersTopInSecondBig if !first => scratch.reset(self.saved(scratch)),
            Lie::BigAboveTop | Lie::StaleSecondBig if first == (self.lie == Lie::BigAboveTop) => {
                let mark = scratch.mark();
                let coords = LIAR_AT.exact_big(scratch)?;
                scratch.reset(mark);
                return Ok(coords);
            }
            _ => {}
        }
        match self.lie {
            Lie::NotFinite => return Err(ExactError::OutOfRange),
            Lie::WeightedBig => return Err(ExactError::Weighted),
            Lie::UnstableExactBig if self.later() > 0 => {
                return [LIAR_AT[0], LIAR_AT[1], 0.5].exact_big(scratch);
            }
            _ => {}
        }
        LIAR_AT.exact_big(scratch)
    }
}
