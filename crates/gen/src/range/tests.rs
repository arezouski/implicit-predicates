use super::shape::Shape;
use super::{ExactPlan, Leaves, grids_fit, magnitudes_fit, plan};
use crate::differences::{DiffGraph, DiffNode, difference_graph};
use crate::dyadic::Dyadic;
use crate::parse::parse;

const ORIENT3D: &str = include_str!("../../tests/fixtures/orient3d.pred");
const INSPHERE: &str = include_str!("../../tests/fixtures/insphere.pred");

/// The exponents of the product domain of the arithmetic core,
/// `[2^-480, 2^480]`; the generator passes the core's own constants.
const LOW: i64 = -480;
const HIGH: i64 = 480;

/// A difference form with its kind of leaves, its shape, its plan and its
/// number of points.
struct Case {
    name: &'static str,
    graph: DiffGraph,
    leaves: Leaves,
    shape: Shape,
    plan: ExactPlan,
    points: usize,
}

fn case(name: &'static str, source: &str, leaves: Leaves) -> Case {
    let p = parse(source, "t.pred").unwrap().remove(0);
    let graph = difference_graph(&p.main, "t.pred", p.position).unwrap();
    let shape = Shape::differences(&graph, leaves).unwrap();
    let plan = plan(&shape, LOW, HIGH).unwrap();
    Case {
        name,
        graph,
        leaves,
        shape,
        plan,
        points: p.params.len(),
    }
}

fn cases() -> Vec<Case> {
    vec![
        case("orient3d one", ORIENT3D, Leaves::One),
        case("orient3d two", ORIENT3D, Leaves::Two),
        case("insphere one", INSPHERE, Leaves::One),
        case("insphere two", INSPHERE, Leaves::Two),
    ]
}

fn exponent(power_of_two: f64) -> i64 {
    i64::from(power_of_two.log2() as i32)
}

#[test]
fn every_product_operand_stays_in_the_domain_at_the_edges() {
    for c in cases() {
        let (m, big_m) = (exponent(c.plan.range.min), exponent(c.plan.range.max));
        assert!(grids_fit(&c.shape, m, LOW), "{}", c.name);
        assert!(magnitudes_fit(&c.shape, big_m, HIGH), "{}", c.name);
        // The interval is the widest: one step further out fails, or leaves
        // the domain itself. The table of exponents in the integration tests
        // is the check of the rule that does not go through this code.
        assert!(
            m - 1 < LOW || !grids_fit(&c.shape, m - 1, LOW),
            "{}",
            c.name
        );
        assert!(
            big_m + 1 > HIGH || !magnitudes_fit(&c.shape, big_m + 1, HIGH),
            "{}",
            c.name
        );
    }
}

#[test]
fn the_component_factor_is_close_to_one() {
    // The rounding factor of the component bound is at most `1 + 2^-20` in
    // these forms.
    for c in cases() {
        let big_m = exponent(c.plan.range.max);
        let with = c.shape.component_bounds(big_m);
        let without = c.shape.magnitudes(big_m);
        let slack = &Dyadic::integer(1) + &Dyadic::pow2(-20);
        for (w, s) in with.iter().zip(&without) {
            assert!(s <= w && *w <= s * &slack, "{}", c.name);
        }
    }
}

/// A small deterministic generator.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// A value at an edge of `[2^m, 2^big_m]`: zero, `2^m`, `2^big_m`, or a
    /// full 53-bit mantissa in `[2^m, 2^(m + 1))` or `[2^(big_m - 1), 2^big_m)`,
    /// with a random sign.
    fn edge(&mut self, m: i64, big_m: i64) -> f64 {
        let fraction = self.next() & ((1 << 52) - 1);
        let (mantissa, e) = match self.next() % 8 {
            0 => return 0.0,
            1 => (1 << 52, m),
            2 => (1 << 52, big_m),
            3..=5 => ((1 << 52) | fraction, m),
            _ => ((1 << 52) | fraction, big_m - 1),
        };
        let magnitude = mantissa as f64 * 2f64.powi(i32::try_from(e - 52).unwrap());
        if self.next() % 2 == 0 {
            magnitude
        } else {
            -magnitude
        }
    }
}

/// The exact value of every node, with the leaves drawn at the edges of the
/// range.
fn evaluate(c: &Case, rng: &mut Rng) -> Vec<Dyadic> {
    let (m, big_m) = (exponent(c.plan.range.min), exponent(c.plan.range.max));
    let coordinates: Vec<[Dyadic; 3]> = (0..c.points)
        .map(|_| [(); 3].map(|()| Dyadic::from_f64(rng.edge(m, big_m))))
        .collect();
    let mut values: Vec<Dyadic> = Vec::new();
    for node in &c.graph.nodes {
        let value = match *node {
            // Every leaf is an exact difference in its own range.
            DiffNode::Leaf { .. } if c.leaves == Leaves::One => {
                Dyadic::from_f64(rng.edge(m, big_m))
            }
            DiffNode::Leaf {
                axis,
                minuend,
                subtrahend,
            } => {
                let axis = usize::from(axis);
                &coordinates[minuend][axis] - &coordinates[subtrahend][axis]
            }
            DiffNode::Neg(x) => &Dyadic::integer(0) - &values[x],
            DiffNode::Add(x, y) => &values[x] + &values[y],
            DiffNode::Sub(x, y) => &values[x] - &values[y],
            DiffNode::Mul(x, y) => &values[x] * &values[y],
        };
        values.push(value);
    }
    values
}

#[test]
fn node_values_lie_on_the_grid_and_within_the_magnitude_bound() {
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    for c in cases() {
        let (m, big_m) = (exponent(c.plan.range.min), exponent(c.plan.range.max));
        let grids = c.shape.grids(m);
        let magnitudes = c.shape.magnitudes(big_m);
        let runs = if c.name.starts_with("orient3d") {
            3000
        } else {
            300
        };
        for _ in 0..runs {
            for (id, value) in evaluate(&c, &mut rng).iter().enumerate() {
                match grids[id] {
                    Some(grid) => assert!(value.is_multiple_of_pow2(grid), "{} {id}", c.name),
                    None => assert_eq!(*value, Dyadic::integer(0), "{} {id}", c.name),
                }
                let negated = &Dyadic::integer(0) - value;
                let magnitude = if *value < Dyadic::integer(0) {
                    &negated
                } else {
                    value
                };
                assert!(*magnitude <= magnitudes[id], "{} {id}", c.name);
            }
        }
    }
}
