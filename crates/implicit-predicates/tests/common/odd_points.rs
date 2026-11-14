//! Points of types made for the tests, which no point of this crate can
//! be: exact coordinates with a weight other than one, no exact coordinates
//! at all, an exact coordinate with any number of components, and no exact
//! coordinates as expansions.

use implicit_predicates::{BigCoords, ExactCoords, ExactError, Point, Scratch, Span, Weight};
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
