use super::{FilterBound, filter_bound};
use crate::differences::{DiffGraph, DiffNode, difference_graph};
use crate::dyadic::Dyadic;
use crate::parse::{Position, parse};

const ORIENT3D: &str = include_str!("../../tests/fixtures/orient3d.pred");
const INSPHERE: &str = include_str!("../../tests/fixtures/insphere.pred");
const SOURCES: [&str; 3] = [
    ORIENT3D,
    INSPHERE,
    "fn p(a: Point3, b: Point3) -> Sign { a.x - b.x }",
];

/// The difference form and the number of points of the first predicate.
fn graph(source: &str) -> (DiffGraph, usize) {
    let p = parse(source, "t.pred").unwrap().remove(0);
    let g = difference_graph(&p.main, "t.pred", p.position).unwrap();
    (g, p.params.len())
}

fn bound_of(g: &DiffGraph) -> FilterBound {
    filter_bound(g, "t.pred", Position { line: 1, column: 1 }).unwrap()
}

fn int(n: i64) -> Dyadic {
    Dyadic::integer(n)
}

fn abs(x: &Dyadic) -> Dyadic {
    if *x < int(0) { &int(0) - x } else { x.clone() }
}

fn power(x: &Dyadic, n: u32) -> Dyadic {
    (0..n).fold(int(1), |acc, _| &acc * x)
}

fn max_magnitude(b: &FilterBound) -> Dyadic {
    b.nodes.iter().map(|n| n.c.clone()).max().unwrap()
}

/// `c_x * P^(g_x) <= 2^1022` at every node, for `P = 2^j`.
fn no_overflow(b: &FilterBound, j: i64) -> bool {
    let limit = Dyadic::pow2(1022);
    b.nodes
        .iter()
        .all(|n| &n.c * &Dyadic::pow2(j * i64::from(n.degree)) <= limit)
}

/// The two requirements on `d_min = P = 2^j`.
fn underflow_covered(b: &FilterBound, j: i64) -> bool {
    let eps_pk = &b.nodes[b.root].alpha * &Dyadic::pow2(j * i64::from(b.degree));
    let products = int(i64::from(b.products) * i64::from(b.degree));
    let underflow = &(&products * &max_magnitude(b)) * &Dyadic::pow2(-1074);
    &eps_pk * &Dyadic::pow2(-32) >= underflow && eps_pk >= Dyadic::pow2(-1021)
}

fn exponent_of(power_of_two: f64) -> i64 {
    let e = ((power_of_two.to_bits() >> 52) & 0x7ff) as i64 - 1023;
    assert_eq!(2f64.powi(e as i32), power_of_two);
    e
}

#[test]
fn d_max_is_the_largest_power_of_two_without_overflow() {
    for source in SOURCES {
        let b = bound_of(&graph(source).0);
        let j = exponent_of(b.constants().d_max);
        assert!(no_overflow(&b, j), "{source}");
        assert!(!no_overflow(&b, j + 1), "{source}");
    }
}

#[test]
fn d_min_is_the_smallest_power_of_two_covering_underflow() {
    for source in SOURCES {
        let b = bound_of(&graph(source).0);
        let j = exponent_of(b.constants().d_min);
        assert!(underflow_covered(&b, j), "{source}");
        assert!(!underflow_covered(&b, j - 1), "{source}");
    }
}

#[test]
fn emitted_constants_are_the_inflated_values_rounded_up() {
    let inflate = &int(1) + &Dyadic::pow2(-30);
    for source in SOURCES {
        let b = bound_of(&graph(source).0);
        let c = b.constants();
        let root = &b.nodes[b.root];
        for (emitted, exact) in [(c.epsilon, &root.alpha), (c.kappa, &root.beta)] {
            let target = exact * &inflate;
            assert!(Dyadic::from_f64(emitted) >= target);
            assert!(Dyadic::from_f64(emitted.next_down()) < target);
        }
    }
}

#[test]
fn the_exact_values_are_reported_as_fractions() {
    let b = bound_of(&graph(ORIENT3D).0);
    let root = &b.nodes[b.root];
    assert_eq!(b.epsilon_in_u, (&root.alpha * &Dyadic::pow2(53)).fraction());
    assert_eq!(b.kappa_exact, root.beta.fraction());
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

    /// A value in `[-1, 1]` with 53 random bits.
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / 2f64.powi(52) - 1.0
    }
}

/// Points as approximations with an error bound each, and the true points
/// within those bounds.
struct Points {
    approx: Vec<[f64; 3]>,
    exact: Vec<[Dyadic; 3]>,
    error: Vec<f64>,
}

fn random_points(rng: &mut Rng, count: usize) -> Points {
    let scale = 2f64.powi((rng.next() % 40) as i32 - 20);
    let base = [rng.unit(), rng.unit(), rng.unit()].map(|x| x * scale);
    // Points near one another, so that the leaves cancel.
    let spread = scale * 2f64.powi(-((rng.next() % 50) as i32));
    let mut points = Points {
        approx: Vec::new(),
        exact: Vec::new(),
        error: Vec::new(),
    };
    for _ in 0..count {
        let approx = base.map(|x| x + rng.unit() * spread);
        let error = match rng.next() % 3 {
            0 => 0.0,
            _ => spread * 2f64.powi(-((rng.next() % 60) as i32)),
        };
        let exact = approx.map(|x| {
            let offset = &Dyadic::from_f64(rng.unit()) * &Dyadic::from_f64(error);
            &Dyadic::from_f64(x) + &offset
        });
        points.approx.push(approx);
        points.exact.push(exact);
        points.error.push(error);
    }
    points
}

/// Checks the three per-node bounds on one call.
fn check_call(g: &DiffGraph, b: &FilterBound, pts: &Points) {
    let mut errors = pts.error.clone();
    errors.sort_by(|x, y| y.total_cmp(x));
    let e = &Dyadic::from_f64(errors[0]) + &Dyadic::from_f64(errors[1]);
    let mut computed: Vec<f64> = Vec::new();
    let mut exact: Vec<Dyadic> = Vec::new();
    for node in &g.nodes {
        let (value, truth) = match *node {
            DiffNode::Leaf {
                axis,
                minuend,
                subtrahend,
            } => {
                let a = usize::from(axis);
                (
                    pts.approx[minuend][a] - pts.approx[subtrahend][a],
                    &pts.exact[minuend][a] - &pts.exact[subtrahend][a],
                )
            }
            DiffNode::Neg(x) => (-computed[x], &int(0) - &exact[x]),
            DiffNode::Add(x, y) => (computed[x] + computed[y], &exact[x] + &exact[y]),
            DiffNode::Sub(x, y) => (computed[x] - computed[y], &exact[x] - &exact[y]),
            DiffNode::Mul(x, y) => (computed[x] * computed[y], &exact[x] * &exact[y]),
        };
        computed.push(value);
        exact.push(truth);
    }
    let delta = g
        .nodes
        .iter()
        .zip(&computed)
        .filter(|(n, _)| matches!(n, DiffNode::Leaf { .. }))
        .map(|(_, v)| Dyadic::from_f64(v.abs()))
        .max()
        .unwrap();
    let d = &delta + &e;
    for (id, n) in b.nodes.iter().enumerate() {
        let dg = power(&d, n.degree);
        let value = Dyadic::from_f64(computed[id]);
        assert!(abs(&exact[id]) <= &n.m * &dg, "true value at node {id}");
        assert!(abs(&value) <= &n.c * &dg, "computed value at node {id}");
        let bound = &(&(&n.alpha * &d) + &(&n.beta * &e)) * &power(&d, n.degree - 1);
        assert!(abs(&(&value - &exact[id])) <= bound, "error at node {id}");
    }
}

#[test]
fn the_per_node_bounds_hold_on_random_points() {
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    for source in SOURCES {
        let (g, count) = graph(source);
        let b = bound_of(&g);
        assert_eq!(b.nodes.len(), g.nodes.len());
        for _ in 0..300 {
            check_call(&g, &b, &random_points(&mut rng, count));
        }
    }
}
