use super::{AxisBound, AxisConstants, FilterForm, filter_form};
use crate::bound::{FilterBound, filter_bound};
use crate::differences::{DiffGraph, DiffNode, difference_graph};
use crate::dyadic::Dyadic;
use crate::parse::{Position, parse};

const ORIENT3D: &str = include_str!("../../tests/fixtures/orient3d.pred");
const SOURCES: [&str; 4] = [
    ORIENT3D,
    "fn p(a: Point3, b: Point3) -> Sign { a.y - b.y }",
    "fn p(a: Point3, b: Point3, c: Point3) -> Sign {
        (a.x - b.x) * (a.x - c.x) * (b.y - c.y) - (a.x - c.x) * (b.x - c.x) * (a.y - b.y)
    }",
    "fn p(a: Point3, b: Point3, c: Point3, d: Point3) -> Sign {
        let s = (a.z - d.z) * (b.z - c.z);
        -s * ((b.x - d.x) * (c.y - d.y) - (c.x - d.x) * (b.y - d.y)) + s * (b.x - c.x) * (a.y - d.y)
    }",
];

/// The difference form and the number of points of the first predicate.
fn graph(source: &str) -> (DiffGraph, usize) {
    let p = parse(source, "t.pred").unwrap().remove(0);
    let g = difference_graph(&p.main, "t.pred", p.position).unwrap();
    (g, p.params.len())
}

fn axis_of(g: &DiffGraph) -> AxisBound {
    match filter_form(g, "t.pred", Position { line: 1, column: 1 }).unwrap() {
        FilterForm::PerAxis(b) => b,
        FilterForm::Global(_) => panic!("expected the per-axis form"),
    }
}

fn global_of(g: &DiffGraph) -> FilterBound {
    filter_bound(g, "t.pred", Position { line: 1, column: 1 }).unwrap()
}

fn int(n: i64) -> Dyadic {
    Dyadic::integer(n)
}

fn abs(x: &Dyadic) -> Dyadic {
    if *x < int(0) { &int(0) - x } else { x.clone() }
}

/// `D^g`, the product of `d[k]^g[k]`.
fn power(d: &[Dyadic; 3], g: [u32; 3]) -> Dyadic {
    (0..3).fold(int(1), |acc, k| (0..g[k]).fold(acc, |acc, _| &acc * &d[k]))
}

fn total(g: [u32; 3]) -> i64 {
    g.iter().map(|&x| i64::from(x)).sum()
}

#[test]
fn the_rules_split_the_beta_of_the_global_form_by_axis() {
    for source in SOURCES {
        let (g, _) = graph(source);
        let (axis, global) = (axis_of(&g), global_of(&g));
        assert_eq!(axis.nodes.len(), global.nodes.len());
        for (a, n) in axis.nodes.iter().zip(&global.nodes) {
            assert_eq!(total(a.degrees), i64::from(n.degree));
            assert_eq!((&a.m, &a.c, &a.alpha), (&n.m, &n.c, &n.alpha));
            let sum = &(&a.beta[0] + &a.beta[1]) + &a.beta[2];
            assert_eq!(sum, n.beta);
            for k in 0..3 {
                assert!(a.degrees[k] > 0 || a.beta[k] == int(0), "{source}");
            }
        }
    }
}

/// `c_x * 2^(j |g_x|) <= 2^1022` at every node.
fn no_overflow(b: &AxisBound, j: i64) -> bool {
    let limit = Dyadic::pow2(1022);
    b.nodes
        .iter()
        .all(|n| &n.c * &Dyadic::pow2(j * total(n.degrees)) <= limit)
}

/// Conditions 1 and 2 on `P = 2^j`, with `K` the total degree.
fn underflow_covered(g: &DiffGraph, b: &AxisBound, j: i64) -> bool {
    let products = g
        .nodes
        .iter()
        .filter(|n| matches!(n, DiffNode::Mul(..)))
        .count();
    let k = total(b.degrees);
    let eps_pk = &b.nodes[b.root].alpha * &Dyadic::pow2(j * k);
    let c_max = b.nodes.iter().map(|n| n.c.clone()).max().unwrap();
    let factor = int(i64::try_from(products).unwrap() * k);
    let underflow = &(&factor * &c_max) * &Dyadic::pow2(-1074);
    &eps_pk * &Dyadic::pow2(-32) >= underflow && eps_pk >= Dyadic::pow2(-1021)
}

fn exponent_of(power_of_two: f64) -> i64 {
    let e = ((power_of_two.to_bits() >> 52) & 0x7ff) as i64 - 1023;
    assert_eq!(2f64.powi(e as i32), power_of_two);
    e
}

#[test]
fn the_range_is_the_widest_that_the_proof_allows() {
    for source in SOURCES {
        let (g, _) = graph(source);
        let b = axis_of(&g);
        let c = b.constants();
        let (low, high) = (exponent_of(c.d_min), exponent_of(c.d_max));
        assert!(
            no_overflow(&b, high) && !no_overflow(&b, high + 1),
            "{source}"
        );
        assert!(underflow_covered(&g, &b, low), "{source}");
        assert!(!underflow_covered(&g, &b, low - 1), "{source}");
    }
}

#[test]
fn emitted_constants_are_the_inflated_values_rounded_up() {
    let inflate = &int(1) + &Dyadic::pow2(-30);
    for source in SOURCES {
        let b = axis_of(&graph(source).0);
        let c = b.constants();
        let root = &b.nodes[b.root];
        let exact = [&root.alpha, &root.beta[0], &root.beta[1], &root.beta[2]];
        let emitted = [c.epsilon, c.kappa[0], c.kappa[1], c.kappa[2]];
        for (emitted, exact) in emitted.into_iter().zip(exact) {
            let target = exact * &inflate;
            assert!(Dyadic::from_f64(emitted) >= target);
            assert!(emitted == 0.0 || Dyadic::from_f64(emitted.next_down()) < target);
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

    /// A value in `[-1, 1]` with 53 random bits.
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / 2f64.powi(52) - 1.0
    }

    fn below(&mut self, n: u64) -> i32 {
        (self.next() % n) as i32
    }
}

/// Approximations of points, an error bound per point and axis, and true
/// points within those bounds.
struct Call {
    approx: Vec<[f64; 3]>,
    exact: Vec<[Dyadic; 3]>,
    error: Vec<[f64; 3]>,
}

impl Call {
    fn new(approx: Vec<[f64; 3]>, error: Vec<[f64; 3]>, rng: &mut Rng) -> Call {
        let exact = approx
            .iter()
            .zip(&error)
            .map(|(p, e)| {
                [0, 1, 2].map(|k| {
                    let offset = &Dyadic::from_f64(rng.unit()) * &Dyadic::from_f64(e[k]);
                    &Dyadic::from_f64(p[k]) + &offset
                })
            })
            .collect();
        Call {
            approx,
            exact,
            error,
        }
    }

    /// `E_k`: the sum of the two largest errors on each axis, rounded up.
    fn e(&self) -> [f64; 3] {
        [0, 1, 2].map(|k| {
            let mut errors: Vec<f64> = self.error.iter().map(|e| e[k]).collect();
            errors.sort_by(|x, y| y.total_cmp(x));
            (&Dyadic::from_f64(errors[0]) + &Dyadic::from_f64(errors[1])).round_up()
        })
    }
}

/// Points near one another, with a different scale on each axis.
fn random_call(rng: &mut Rng, count: usize) -> Call {
    let scale: [f64; 3] = [0; 3].map(|_| 2f64.powi(rng.below(80) - 40));
    let base = scale.map(|s| rng.unit() * s);
    let spread = scale.map(|s| s * 2f64.powi(-rng.below(50)));
    let approx: Vec<[f64; 3]> = (0..count)
        .map(|_| [0, 1, 2].map(|k| base[k] + rng.unit() * spread[k]))
        .collect();
    let error: Vec<[f64; 3]> = (0..count)
        .map(|_| {
            [0, 1, 2].map(|k| match rng.next() % 3 {
                0 => 0.0,
                _ => spread[k] * 2f64.powi(-rng.below(60)),
            })
        })
        .collect();
    Call::new(approx, error, rng)
}

/// The computed and the exact value of every node.
fn evaluate(g: &DiffGraph, call: &Call) -> (Vec<f64>, Vec<Dyadic>) {
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
                    call.approx[minuend][a] - call.approx[subtrahend][a],
                    &call.exact[minuend][a] - &call.exact[subtrahend][a],
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
    (computed, exact)
}

/// `delta_k`: the largest magnitude of the computed leaves of each axis.
fn delta(g: &DiffGraph, computed: &[f64]) -> [f64; 3] {
    let mut delta = [0.0f64; 3];
    for (node, value) in g.nodes.iter().zip(computed) {
        if let DiffNode::Leaf { axis, .. } = *node {
            let k = usize::from(axis);
            delta[k] = delta[k].max(value.abs());
        }
    }
    delta
}

/// The error term `alpha D^g + sum of beta_k E_k D^(g - e_k)` of a node.
fn error_bound(
    degrees: [u32; 3],
    alpha: &Dyadic,
    beta: &[Dyadic; 3],
    d: &[Dyadic; 3],
    e: &[Dyadic; 3],
) -> Dyadic {
    let mut bound = alpha * &power(d, degrees);
    for k in (0..3).filter(|&k| degrees[k] > 0) {
        let mut g = degrees;
        g[k] -= 1;
        bound = &bound + &(&(&beta[k] * &e[k]) * &power(d, g));
    }
    bound
}

/// Checks the three per-node bounds on one call.
fn check_nodes(g: &DiffGraph, b: &AxisBound, call: &Call) {
    let (computed, exact) = evaluate(g, call);
    let e = call.e().map(Dyadic::from_f64);
    let delta = delta(g, &computed).map(Dyadic::from_f64);
    let d = [0, 1, 2].map(|k| &delta[k] + &e[k]);
    for (id, n) in b.nodes.iter().enumerate() {
        let dg = power(&d, n.degrees);
        let value = Dyadic::from_f64(computed[id]);
        assert!(abs(&exact[id]) <= &n.m * &dg, "true value at node {id}");
        assert!(abs(&value) <= &n.c * &dg, "computed value at node {id}");
        let bound = error_bound(n.degrees, &n.alpha, &n.beta, &d, &e);
        assert!(abs(&(&value - &exact[id])) <= bound, "error at node {id}");
    }
}

#[test]
fn the_per_node_bounds_hold_on_random_points() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    for source in SOURCES {
        let (g, count) = graph(source);
        let b = axis_of(&g);
        for _ in 0..300 {
            check_nodes(&g, &b, &random_call(&mut rng, count));
        }
    }
}

/// The bound as the filter evaluates it: a sum of chains of products, each
/// starting with its constant.
fn emitted_bound(c: &AxisConstants, d: [f64; 3], e: [f64; 3]) -> f64 {
    let chain = |start: f64, slot: Option<usize>| {
        let mut value = start;
        for k in 0..3 {
            for i in 0..c.degrees[k] {
                value *= if slot == Some(k) && i == 0 {
                    e[k]
                } else {
                    d[k]
                };
            }
        }
        value
    };
    let mut bound = chain(c.epsilon, None);
    for k in (0..3).filter(|&k| c.degrees[k] > 0) {
        bound += chain(c.kappa[k], Some(k));
    }
    bound
}

/// The outcome of one call: the error of the computed value, the exact
/// bound with `D_k = delta_k + E_k`, and the bound the filter computes.
fn outcome(g: &DiffGraph, b: &AxisBound, call: &Call) -> (Dyadic, Dyadic, f64, [f64; 3]) {
    let (computed, exact) = evaluate(g, call);
    let e = call.e();
    let delta = delta(g, &computed);
    let d = [0, 1, 2].map(|k| delta[k] + e[k]);
    let exact_d = [0, 1, 2].map(|k| &Dyadic::from_f64(delta[k]) + &Dyadic::from_f64(e[k]));
    let root = &b.nodes[b.root];
    let exact_bound = error_bound(
        b.degrees,
        &root.alpha,
        &root.beta,
        &exact_d,
        &e.map(Dyadic::from_f64),
    );
    let error = abs(&(&Dyadic::from_f64(computed[g.root]) - &exact[g.root]));
    (error, exact_bound, emitted_bound(&b.constants(), d, e), d)
}

/// Whether some product of the call is subnormal and inexact.
fn underflows(g: &DiffGraph, call: &Call) -> bool {
    let (computed, _) = evaluate(g, call);
    g.nodes.iter().enumerate().any(|(id, node)| match *node {
        DiffNode::Mul(x, y) => {
            let product = &Dyadic::from_f64(computed[x]) * &Dyadic::from_f64(computed[y]);
            computed[id].abs() < f64::MIN_POSITIVE && Dyadic::from_f64(computed[id]) != product
        }
        _ => false,
    })
}

/// Points of orient3d whose `D_k` are at `scale[k]`, with leaves of every
/// magnitude down to the subnormals, so that products underflow.
fn call_at(rng: &mut Rng, scale: [f64; 3], upper: [bool; 3]) -> Call {
    let mut approx = vec![[0.0f64; 3]; 4];
    let mut error = vec![[0.0f64; 3]; 4];
    for k in 0..3 {
        // The largest leaf, against `d` at the origin: `scale` at the lower
        // limit, a little below it at the upper limit, with room for `E_k`.
        let top = if upper[k] {
            scale[k] * (1.0 - 2f64.powi(-20))
        } else {
            scale[k]
        };
        let largest = rng.below(3) as usize;
        for (p, point) in approx.iter_mut().enumerate().take(3) {
            point[k] = if p == largest {
                top
            } else {
                scale[k] * rng.unit() * 2f64.powi(-rng.below(800))
            };
            if rng.next() % 2 == 0 {
                point[k] = -point[k];
            }
        }
        for point in &mut error {
            if rng.next() % 3 != 0 {
                point[k] = scale[k] * (rng.unit() + 1.0) * 2f64.powi(-24 - rng.below(800));
            }
        }
    }
    Call::new(approx, error, rng)
}

#[test]
fn at_the_limits_of_the_range_the_bound_covers_underflow() {
    let (g, _) = graph(ORIENT3D);
    let b = axis_of(&g);
    let c = b.constants();
    let scales = [c.d_min, 2.0 * c.d_min, c.d_max / 2.0, c.d_max];
    let slack = &int(1) + &Dyadic::pow2(-32);
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    let mut underflowing = 0;
    for _ in 0..20_000 {
        let pick = [0; 3].map(|_| rng.below(4) as usize);
        let upper = pick.map(|i| i >= 2);
        let call = call_at(&mut rng, pick.map(|i| scales[i]), upper);
        let (error, exact_bound, bound, d) = outcome(&g, &b, &call);
        assert!(d.iter().all(|&dk| c.d_min <= dk && dk <= c.d_max), "{d:?}");
        assert!(error <= &slack * &exact_bound, "exact bound at {d:?}");
        assert!(
            bound.is_finite() && error <= Dyadic::from_f64(bound),
            "{d:?}"
        );
        underflowing += usize::from(underflows(&g, &call));
    }
    assert!(underflowing > 5_000, "only {underflowing} calls underflow");
}

#[test]
fn below_the_range_the_bound_does_not_cover_underflow() {
    // `D_x = 2^100` and `D_y = D_z` about `2^-520`: `by * cz` is subnormal
    // and its rounding error, about `2^-1075`, times `ax` exceeds the bound.
    let (g, _) = graph(ORIENT3D);
    let b = axis_of(&g);
    let by = 2f64.powi(-520) + 2f64.powi(-553);
    let cz = 2f64.powi(-520) + 2f64.powi(-555);
    let approx = vec![
        [2f64.powi(100), 0.0, 0.0],
        [0.0, by, 0.0],
        [0.0, 0.0, cz],
        [0.0; 3],
    ];
    let call = Call::new(approx, vec![[0.0; 3]; 4], &mut Rng(1));
    let (error, exact_bound, bound, d) = outcome(&g, &b, &call);
    assert!(d[1] < b.constants().d_min && d[2] < b.constants().d_min);
    assert!(
        error > &exact_bound * &int(4096),
        "the error must exceed the bound"
    );
    assert!(error > Dyadic::from_f64(bound));
}
