use super::{
    Budget, PartsBound, PartsConstants, PartsNode, PartsOutcome, derive, parts_bound, rules,
};
use crate::bound::{AxisBound, FilterBound, FilterForm, filter_bound, filter_form};
use crate::differences::{DiffGraph, DiffNode, difference_graph};
use crate::dyadic::Dyadic;
use crate::parse::{Position, parse};

const ORIENT3D: &str = include_str!("../../tests/fixtures/orient3d.pred");
const INSPHERE: &str = include_str!("../../tests/fixtures/insphere.pred");
const INCIRCLE: &str = include_str!("../../tests/fixtures/incircle.pred");
const ZERO_IN_AN_AXIS: &str = "fn p(a: Point3, b: Point3, c: Point3) -> Sign {
    (a.x - b.x) * (a.x - c.x) + (a.y - b.y) * (a.z - c.z)
}";
const NO_Z: &str = "fn p(a: Point3, b: Point3, c: Point3) -> Sign {
    (a.x - b.x) * (a.y - c.y) + (a.y - b.y) * (b.y - c.y)
}";
const SOURCES: [&str; 5] = [ORIENT3D, INSPHERE, INCIRCLE, ZERO_IN_AN_AXIS, NO_Z];

const AT: Position = Position { line: 1, column: 1 };

/// The difference form and the number of points of the first predicate.
fn graph(source: &str) -> (DiffGraph, usize) {
    let p = parse(source, "t.pred").unwrap().remove(0);
    let g = difference_graph(&p.main, "t.pred", p.position).unwrap();
    (g, p.params.len())
}

fn parts_of(g: &DiffGraph) -> PartsBound {
    match parts_bound(g, "t.pred", AT).unwrap() {
        PartsOutcome::Bound(b) => b,
        PartsOutcome::Unavailable(reason) => panic!("expected a bound: {reason}"),
    }
}

fn unavailable(g: &DiffGraph) -> String {
    match parts_bound(g, "t.pred", AT).unwrap() {
        PartsOutcome::Bound(_) => panic!("expected no bound"),
        PartsOutcome::Unavailable(reason) => reason,
    }
}

fn global_of(g: &DiffGraph) -> FilterBound {
    filter_bound(g, "t.pred", AT).unwrap()
}

fn axis_of(g: &DiffGraph) -> AxisBound {
    match filter_form(g, "t.pred", AT).unwrap() {
        FilterForm::PerAxis(b) => b,
        FilterForm::Global(_) => panic!("expected the per-axis form"),
    }
}

fn int(n: i64) -> Dyadic {
    Dyadic::integer(n)
}

fn abs(x: &Dyadic) -> Dyadic {
    if *x < int(0) { &int(0) - x } else { x.clone() }
}

fn u() -> Dyadic {
    Dyadic::pow2(-53)
}

/// Whether `n * unit <= value < n * unit * (1 + 2^-40)`.
fn close_above(value: &Dyadic, n: u64, unit: &Dyadic) -> bool {
    let target = &Dyadic::integer(n) * unit;
    let limit = &target * &(&int(1) + &Dyadic::pow2(-40));
    target <= *value && *value < limit
}

fn total(g: [u32; 3]) -> i64 {
    g.iter().map(|&x| i64::from(x)).sum()
}

fn sum<'a>(values: impl Iterator<Item = &'a Dyadic>) -> Dyadic {
    values.fold(int(0), |acc, v| &acc + v)
}

// The bound of insphere.

const INSPHERE_VECTORS: [[u32; 3]; 3] = [[1, 1, 3], [1, 3, 1], [3, 1, 1]];

#[test]
fn insphere_has_three_terms_of_epsilon() {
    let b = parts_of(&graph(INSPHERE).0);
    let vectors: Vec<[u32; 3]> = b.epsilon.iter().map(|t| t.degrees).collect();
    assert_eq!(vectors, INSPHERE_VECTORS);
    let expected = [([3, 1, 1], 382), ([1, 3, 1], 382), ([1, 1, 3], 358)];
    for (degrees, n) in expected {
        assert!(b.epsilon_is_u_times(degrees, n), "{degrees:?}");
        assert!(!b.epsilon_is_u_times(degrees, n - 1), "{degrees:?}");
        assert!(!b.epsilon_is_u_times(degrees, n + 1), "{degrees:?}");
        let term = b.epsilon.iter().find(|t| t.degrees == degrees).unwrap();
        assert!(close_above(&term.exact, n, &u()), "{degrees:?}");
    }
    assert!(!b.epsilon_is_u_times([2, 2, 1], 382));
}

#[test]
fn insphere_has_nine_terms_of_kappa() {
    let b = parts_of(&graph(INSPHERE).0);
    assert_eq!(b.kappa.len(), 9);
    for g in INSPHERE_VECTORS {
        for axis in 0..3 {
            let mut rest = g;
            rest[axis] -= 1;
            let n = if g[axis] == 3 { 72 } else { 24 };
            assert!(b.kappa_is(axis, rest, n), "{g:?} on axis {axis}");
            assert!(!b.kappa_is(axis, rest, n + 1), "{g:?} on axis {axis}");
            let term = b
                .kappa
                .iter()
                .find(|t| t.axis == axis && t.rest == rest)
                .unwrap();
            assert!(close_above(&term.exact, n, &int(1)));
        }
    }
    assert!(!b.kappa_is(3, [1, 1, 1], 24));
}

#[test]
fn with_all_sizes_equal_the_terms_add_up_to_the_global_bound() {
    for source in SOURCES {
        let g = graph(source).0;
        let (b, global) = (parts_of(&g), global_of(&g));
        let root = &global.nodes[global.root];
        assert_eq!(
            sum(b.epsilon.iter().map(|t| &t.exact)),
            root.alpha,
            "{source}"
        );
        assert_eq!(sum(b.kappa.iter().map(|t| &t.exact)), root.beta, "{source}");
    }
    let b = parts_of(&graph(INSPHERE).0);
    let total_epsilon = sum(b.epsilon.iter().map(|t| &t.exact));
    let total_kappa = sum(b.kappa.iter().map(|t| &t.exact));
    assert!(close_above(&total_epsilon, 1122, &u()));
    assert!(close_above(&total_kappa, 360, &int(1)));
}

#[test]
fn every_node_evaluated_at_one_gives_the_global_coefficients() {
    for source in SOURCES {
        let g = graph(source).0;
        let (nodes, global) = (rules(&g), global_of(&g));
        assert_eq!(nodes.len(), global.nodes.len());
        for (id, (p, n)) in nodes.iter().zip(&global.nodes).enumerate() {
            assert_eq!(sum(p.m.values()), n.m, "{source} at {id}");
            assert_eq!(sum(p.c.values()), n.c, "{source} at {id}");
            assert_eq!(sum(p.a.values()), n.alpha, "{source} at {id}");
            let beta = sum(p.b.iter().flat_map(|b| b.values()));
            assert_eq!(beta, n.beta, "{source} at {id}");
            // Every vector of the support has the total degree of the node,
            // and the support of each `B_k`, shifted by `e_k`, is in it.
            for v in p.c.keys() {
                assert_eq!(total(*v), i64::from(n.degree));
            }
            assert!(p.m.keys().eq(p.c.keys()) && p.a.keys().eq(p.c.keys()));
            for k in 0..3 {
                for h in p.b[k].keys() {
                    let mut v = *h;
                    v[k] += 1;
                    assert!(p.c.contains_key(&v), "{source} at {id}");
                }
            }
        }
    }
}

#[test]
fn insphere_is_certainly_zero_with_an_axis_of_size_zero() {
    let b = parts_of(&graph(INSPHERE).0);
    assert!(b.certain_zero());
    assert_eq!(b.range_axes(), [true; 3]);
    assert_eq!(b.constants().range_axes, [true; 3]);
}

/// `c_x * 2^(j |g_x|) <= 2^1022` at every node of the global form.
fn no_overflow(global: &FilterBound, j: i64) -> bool {
    let limit = Dyadic::pow2(1022);
    global
        .nodes
        .iter()
        .all(|n| &n.c * &Dyadic::pow2(j * i64::from(n.degree)) <= limit)
}

/// Conditions 1 and 2 on `P = 2^j`, with `epsilon` for the coefficient.
fn underflow_covered(global: &FilterBound, epsilon: &Dyadic, j: i64) -> bool {
    let k = i64::from(global.degree);
    let eps_pk = epsilon * &Dyadic::pow2(j * k);
    let c_max = global.nodes.iter().map(|n| n.c.clone()).max().unwrap();
    let factor = int(i64::from(global.products) * k);
    let underflow = &(&factor * &c_max) * &Dyadic::pow2(-1074);
    &eps_pk * &Dyadic::pow2(-32) >= underflow && eps_pk >= Dyadic::pow2(-1021)
}

fn exponent_of(power_of_two: f64) -> i64 {
    let e = ((power_of_two.to_bits() >> 52) & 0x7ff) as i64 - 1023;
    assert_eq!(2f64.powi(e as i32), power_of_two);
    e
}

#[test]
fn the_range_of_insphere_uses_the_smallest_coefficient_of_epsilon() {
    let g = graph(INSPHERE).0;
    let (b, global) = (parts_of(&g), global_of(&g));
    let c = b.constants();
    assert_eq!(c.d_max, global.constants().d_max);
    let high = exponent_of(c.d_max);
    assert!(no_overflow(&global, high) && !no_overflow(&global, high + 1));
    let low = exponent_of(c.d_min);
    let smallest = &int(358) * &u();
    let least = b.epsilon.iter().map(|t| t.exact.clone()).min().unwrap();
    assert!(close_above(&least, 358, &u()));
    assert!(underflow_covered(&global, &least, low));
    assert!(!underflow_covered(&global, &least, low - 1));
    // The same powers of two with 358 u exactly.
    assert!(underflow_covered(&global, &smallest, low));
    assert!(!underflow_covered(&global, &smallest, low - 1));
    // Condition 3, `P^K >= 2^-1021`, holds and is not the one that binds.
    assert!(low * 5 >= -1021);
    assert_eq!((low, high), (-195, 203));
    // The global form, whose `epsilon` is larger, has a range at least as
    // wide.
    assert!(c.d_min >= global.constants().d_min);
}

#[test]
fn the_range_is_the_widest_that_the_proof_allows() {
    for source in SOURCES {
        let g = graph(source).0;
        let (b, global) = (parts_of(&g), global_of(&g));
        let c = b.constants();
        let k = i64::from(global.degree);
        let least = b.epsilon.iter().map(|t| t.exact.clone()).min().unwrap();
        let covered = |j: i64| underflow_covered(&global, &least, j) && j * k >= -1021;
        let low = exponent_of(c.d_min);
        assert!(covered(low) && !covered(low - 1), "{source}");
        assert_eq!(c.d_max, global.constants().d_max, "{source}");
    }
}

#[test]
fn condition_3_bounds_the_range_when_epsilon_is_large() {
    // With `epsilon_min = 2^10`, conditions 1 and 2 alone allow `P = 2^-206`
    // for `K = 5`; condition 3, `P^5 >= 2^-1021`, stops at `2^-204`.
    let epsilon = Dyadic::pow2(10);
    assert_eq!(super::d_min_exponent(&epsilon, &int(1), 5, 1), Some(-206));
    assert_eq!(
        super::parts_d_min_exponent(&epsilon, &int(1), 5, 1),
        Some(-204)
    );
    // Below `epsilon_min = 1` it does not bind.
    let small = &int(358) * &u();
    assert_eq!(
        super::parts_d_min_exponent(&small, &int(1), 5, 1),
        super::d_min_exponent(&small, &int(1), 5, 1)
    );
    // From degree 1022 on, no `j <= -1` satisfies it.
    assert!(super::d_min_exponent(&epsilon, &int(1), 1022, 1).is_some());
    assert_eq!(
        super::parts_d_min_exponent(&epsilon, &int(1), 1022, 1),
        None
    );
}

#[test]
fn emitted_constants_are_the_inflated_values_rounded_up() {
    let inflate = &int(1) + &Dyadic::pow2(-30);
    for source in SOURCES {
        let b = parts_of(&graph(source).0);
        let c = b.constants();
        assert_eq!(c.degree, b.degree);
        assert_eq!(c.epsilon.len(), b.epsilon.len());
        assert_eq!(c.kappa.len(), b.kappa.len());
        let mut pairs: Vec<(f64, &Dyadic)> = Vec::new();
        for (e, t) in c.epsilon.iter().zip(&b.epsilon) {
            assert_eq!(e.degrees, t.degrees);
            pairs.push((e.value, &t.exact));
        }
        for (e, t) in c.kappa.iter().zip(&b.kappa) {
            assert_eq!((e.axis, e.rest), (t.axis, t.rest));
            pairs.push((e.value, &t.exact));
        }
        for (emitted, exact) in pairs {
            let target = exact * &inflate;
            assert!(Dyadic::from_f64(emitted) >= target);
            assert!(Dyadic::from_f64(emitted.next_down()) < target);
        }
    }
}

#[test]
fn the_exact_coefficients_are_given_as_fractions() {
    let b = parts_of(&graph(INSPHERE).0);
    for t in &b.epsilon {
        assert_eq!(
            t.coefficient_in_u,
            (&t.exact * &Dyadic::pow2(53)).fraction()
        );
    }
    for t in &b.kappa {
        assert_eq!(t.coefficient, t.exact.fraction());
    }
}

// Other forms.

#[test]
fn on_a_multihomogeneous_form_the_bound_is_the_per_axis_one() {
    let g = graph(ORIENT3D).0;
    let (b, axis) = (parts_of(&g), axis_of(&g));
    assert_eq!(b.epsilon.len(), 1);
    assert_eq!(b.epsilon[0].degrees, axis.degrees);
    assert_eq!(b.epsilon[0].coefficient_in_u, axis.epsilon_in_u);
    assert_eq!(b.kappa.len(), 3);
    for (k, t) in b.kappa.iter().enumerate() {
        let mut rest = axis.degrees;
        rest[k] -= 1;
        assert_eq!((t.axis, t.rest), (k, rest));
        assert_eq!(t.coefficient, axis.kappa_exact[k]);
    }
    let (c, a) = (b.constants(), axis.constants());
    assert_eq!(c.epsilon[0].value, a.epsilon);
    for (k, t) in c.kappa.iter().enumerate() {
        assert_eq!(t.value, a.kappa[k]);
    }
    assert_eq!((c.d_min, c.d_max), (a.d_min, a.d_max));
    assert!(b.certain_zero());
    assert_eq!(b.range_axes(), [true; 3]);
}

#[test]
fn a_vector_of_degree_zero_in_an_axis_is_not_certainly_zero() {
    let b = parts_of(&graph(ZERO_IN_AN_AXIS).0);
    let vectors: Vec<[u32; 3]> = b.epsilon.iter().map(|t| t.degrees).collect();
    assert_eq!(vectors, [[0, 1, 1], [2, 0, 0]]);
    assert!(!b.certain_zero());
    assert_eq!(b.range_axes(), [true; 3]);
}

#[test]
fn one_vector_of_degree_zero_in_an_axis_is_enough_to_lose_the_certain_zero() {
    let source = "fn p(a: Point3, b: Point3, c: Point3, d: Point3) -> Sign {
        (a.x - b.x) * (a.y - c.y) * (a.z - c.z) + (a.x - b.x) * (a.x - c.x) * (a.x - d.x)
    }";
    let b = parts_of(&graph(source).0);
    let vectors: Vec<[u32; 3]> = b.epsilon.iter().map(|t| t.degrees).collect();
    assert_eq!(vectors, [[1, 1, 1], [3, 0, 0]]);
    assert!(!b.certain_zero());
    assert_eq!(b.range_axes(), [true; 3]);
}

#[test]
fn an_axis_that_the_form_does_not_use_is_not_required() {
    let b = parts_of(&graph(NO_Z).0);
    let vectors: Vec<[u32; 3]> = b.epsilon.iter().map(|t| t.degrees).collect();
    assert_eq!(vectors, [[0, 2, 0], [1, 1, 0]]);
    assert_eq!(b.range_axes(), [true, true, false]);
    assert!(!b.certain_zero());
    assert!(b.kappa.iter().all(|t| t.axis != 2));
}

/// `(ax + ay + az)^(2^levels)`, by repeated squaring.
fn power_of_a_sum(levels: u32) -> String {
    let mut body = String::from("let s0 = (a.x - b.x) + (a.y - b.y) + (a.z - b.z);\n");
    for i in 1..=levels {
        body.push_str(&format!("let s{i} = s{} * s{};\n", i - 1, i - 1));
    }
    format!("fn p(a: Point3, b: Point3) -> Sign {{ {body} s{levels} }}")
}

#[test]
fn a_node_with_too_many_monomials_gives_no_bound() {
    // `(x + y + z)^16` has 153 monomials.
    let g = graph(&power_of_a_sum(4)).0;
    let reason = unavailable(&g);
    assert!(reason.contains("more than 64 terms"), "{reason}");
}

#[test]
fn too_many_degree_vectors_at_the_root_give_no_bound() {
    // `(x + y + z)^8` has 45 monomials.
    let g = graph(&power_of_a_sum(3)).0;
    let reason = unavailable(&g);
    assert!(reason.contains("45 degree vectors"), "{reason}");
    // `(x + y + z)^4` has 15, within the limit.
    let b = parts_of(&graph(&power_of_a_sum(2)).0);
    assert_eq!(b.epsilon.len(), 15);
}

#[test]
fn the_documented_limits_are_those_of_the_budget() {
    // The documentation of `parts_bound` and the proof of the range, which
    // counts at most 64 terms, cite these numbers.
    let b = Budget::DEFAULT;
    assert_eq!(b.monomials, 64);
    assert_eq!(b.root_vectors, 16);
    assert_eq!(b.operations, 1 << 22);
    assert_eq!(b.bits, 1 << 26);
}

#[test]
fn every_budget_is_enforced() {
    let g = graph(INSPHERE).0;
    let global = global_of(&g);
    let full = Budget::DEFAULT;
    assert!(matches!(derive(&g, &global, &full), PartsOutcome::Bound(_)));
    let cases = [
        (
            Budget {
                monomials: 2,
                ..full
            },
            "terms",
        ),
        (
            Budget {
                root_vectors: 2,
                ..full
            },
            "degree vectors",
        ),
        (
            Budget {
                operations: 100,
                ..full
            },
            "operations",
        ),
        (Budget { bits: 1000, ..full }, "bits"),
    ];
    for (budget, word) in cases {
        match derive(&g, &global, &budget) {
            PartsOutcome::Unavailable(reason) => assert!(reason.contains(word), "{reason}"),
            PartsOutcome::Bound(_) => panic!("the budget on {word} is not enforced"),
        }
    }
}

#[test]
fn a_leaf_on_an_unknown_axis_gives_no_bound() {
    let mut g = graph(NO_Z).0;
    for node in &mut g.nodes {
        if let DiffNode::Leaf { axis, .. } = node {
            *axis = 3;
        }
    }
    assert!(filter_bound(&g, "t.pred", AT).is_ok());
    let reason = unavailable(&g);
    assert!(reason.contains("axis 3"), "{reason}");
}

#[test]
fn a_degree_vector_without_the_degree_of_its_node_gives_no_bound() {
    let mut g = graph(INSPHERE).0;
    let global = global_of(&g);
    let root = g.root;
    g.degrees[root] += 1;
    match derive(&g, &global, &Budget::DEFAULT) {
        PartsOutcome::Unavailable(reason) => {
            assert!(
                reason.contains("a node of degree 6 has the degree vector"),
                "{reason}"
            );
        }
        PartsOutcome::Bound(_) => panic!("a node with the wrong degree is accepted"),
    }
    // A leaf of degree 0.
    let mut g = graph(NO_Z).0;
    let global = global_of(&g);
    let leaf = g
        .nodes
        .iter()
        .position(|n| matches!(n, DiffNode::Leaf { .. }))
        .unwrap();
    g.degrees[leaf] = 0;
    assert!(matches!(
        derive(&g, &global, &Budget::DEFAULT),
        PartsOutcome::Unavailable(_)
    ));
}

/// `x^980 + 2^10 * y^980`, with the sum of `y^980` written as a balanced
/// tree: the coefficient of `epsilon` of `x^980` is far smaller than that
/// of the global form.
fn uneven_powers() -> String {
    let mut body = String::from("let x0 = a.x - b.x;\nlet y0 = a.y - b.y;\n");
    for i in 1..=9 {
        body.push_str(&format!("let x{i} = x{} * x{};\n", i - 1, i - 1));
        body.push_str(&format!("let y{i} = y{} * y{};\n", i - 1, i - 1));
    }
    // 980 = 512 + 256 + 128 + 64 + 16 + 4.
    body.push_str("let x = x9 * x8 * x7 * x6 * x4 * x2;\n");
    body.push_str("let t0 = y9 * y8 * y7 * y6 * y4 * y2;\n");
    for i in 1..=10 {
        body.push_str(&format!("let t{i} = t{} + t{};\n", i - 1, i - 1));
    }
    format!("fn p(a: Point3, b: Point3) -> Sign {{ {body} x + t10 }}")
}

#[test]
fn a_form_without_a_range_gives_no_bound_and_no_error() {
    let g = graph(&uneven_powers()).0;
    assert_eq!(g.degrees[g.root], 980);
    assert!(filter_bound(&g, "t.pred", AT).is_ok());
    let reason = unavailable(&g);
    assert!(reason.contains("underflow"), "{reason}");
}

#[test]
fn what_the_global_form_rejects_is_an_error() {
    // Degree 1023 is above the limit of the global form.
    let mut body = String::from("let x0 = a.x - b.x;\n");
    for i in 1..=10 {
        body.push_str(&format!("let x{i} = x{} * x{};\n", i - 1, i - 1));
    }
    let source = format!("fn p(a: Point3, b: Point3) -> Sign {{ {body} x10 }}");
    let g = graph(&source).0;
    assert!(filter_bound(&g, "t.pred", AT).is_err());
    assert!(parts_bound(&g, "t.pred", AT).is_err());
}

// The bound on calls.

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

/// `D^g`, the product of `d[k]^g[k]`.
fn power(d: &[Dyadic; 3], g: [u32; 3]) -> Dyadic {
    (0..3).fold(int(1), |acc, k| (0..g[k]).fold(acc, |acc, _| &acc * &d[k]))
}

fn eval(p: &super::Poly, d: &[Dyadic; 3]) -> Dyadic {
    p.iter()
        .fold(int(0), |acc, (g, v)| &acc + &(v * &power(d, *g)))
}

/// `A(D) + sum of E_k B_k(D)`.
fn error_bound(a: &super::Poly, b: &[super::Poly; 3], d: &[Dyadic; 3], e: &[Dyadic; 3]) -> Dyadic {
    (0..3).fold(eval(a, d), |acc, k| &acc + &(&e[k] * &eval(&b[k], d)))
}

#[test]
fn the_per_node_bounds_hold_on_random_points() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    for source in SOURCES {
        let (g, count) = graph(source);
        let nodes = rules(&g);
        for _ in 0..200 {
            let call = random_call(&mut rng, count);
            let (computed, exact) = evaluate(&g, &call);
            let e = call.e().map(Dyadic::from_f64);
            let delta = delta(&g, &computed).map(Dyadic::from_f64);
            let d = [0, 1, 2].map(|k| &delta[k] + &e[k]);
            for (id, n) in nodes.iter().enumerate() {
                let value = Dyadic::from_f64(computed[id]);
                assert!(abs(&exact[id]) <= eval(&n.m, &d), "true value at {id}");
                assert!(abs(&value) <= eval(&n.c, &d), "computed value at {id}");
                let bound = error_bound(&n.a, &n.b, &d, &e);
                assert!(abs(&(&value - &exact[id])) <= bound, "error at {id}");
            }
        }
    }
}

/// The bound as the filter evaluates it: a sum of chains of products, each
/// starting with its constant.
fn emitted_bound(c: &PartsConstants, d: [f64; 3], e: [f64; 3]) -> f64 {
    let chain = |start: f64, g: [u32; 3], slot: Option<usize>| {
        let mut value = start;
        if let Some(k) = slot {
            value *= e[k];
        }
        for k in 0..3 {
            for _ in 0..g[k] {
                value *= d[k];
            }
        }
        value
    };
    let mut bound = 0.0;
    for t in &c.epsilon {
        bound += chain(t.value, t.degrees, None);
    }
    for t in &c.kappa {
        bound += chain(t.value, t.rest, Some(t.axis));
    }
    bound
}

/// The error of the computed value, the exact bound with
/// `D_k = delta_k + E_k`, the bound the filter computes and the `d_k`.
fn outcome(
    g: &DiffGraph,
    root: &PartsNode,
    c: &PartsConstants,
    call: &Call,
) -> (Dyadic, Dyadic, f64, [f64; 3]) {
    let (computed, exact) = evaluate(g, call);
    let e = call.e();
    let delta = delta(g, &computed);
    let d = [0, 1, 2].map(|k| delta[k] + e[k]);
    let exact_d = [0, 1, 2].map(|k| &Dyadic::from_f64(delta[k]) + &Dyadic::from_f64(e[k]));
    let exact_bound = error_bound(&root.a, &root.b, &exact_d, &e.map(Dyadic::from_f64));
    let error = abs(&(&Dyadic::from_f64(computed[g.root]) - &exact[g.root]));
    (error, exact_bound, emitted_bound(c, d, e), d)
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

/// Points of insphere whose `d_k` are at `scale[k]`, with leaves of every
/// magnitude down to the subnormals, so that products underflow.
fn call_at(rng: &mut Rng, scale: [f64; 3], upper: [bool; 3]) -> Call {
    let mut approx = vec![[0.0f64; 3]; 5];
    let mut error = vec![[0.0f64; 3]; 5];
    for k in 0..3 {
        let top = if upper[k] {
            scale[k] * (1.0 - 2f64.powi(-20))
        } else {
            scale[k]
        };
        let largest = rng.below(4) as usize;
        for (p, point) in approx.iter_mut().enumerate().take(4) {
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
    let (g, _) = graph(INSPHERE);
    let c = parts_of(&g).constants();
    let root = rules(&g).swap_remove(g.root);
    let scales = [c.d_min, 2.0 * c.d_min, c.d_max / 2.0, c.d_max];
    let slack = &int(1) + &Dyadic::pow2(-32);
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    let mut underflowing = 0;
    for _ in 0..4_000 {
        let pick = [0; 3].map(|_| rng.below(4) as usize);
        let upper = pick.map(|i| i >= 2);
        let call = call_at(&mut rng, pick.map(|i| scales[i]), upper);
        let (error, exact_bound, bound, d) = outcome(&g, &root, &c, &call);
        assert!(d.iter().all(|&dk| c.d_min <= dk && dk <= c.d_max), "{d:?}");
        assert!(error <= &slack * &exact_bound, "exact bound at {d:?}");
        assert!(
            bound.is_finite() && error <= Dyadic::from_f64(bound),
            "{d:?}"
        );
        underflowing += usize::from(underflows(&g, &call));
    }
    assert!(underflowing > 1_000, "only {underflowing} calls underflow");
}

#[test]
fn on_random_points_the_emitted_bound_covers_the_error() {
    let mut rng = Rng(0x1234_5678_9abc_def1);
    for source in SOURCES {
        let (g, count) = graph(source);
        let c = parts_of(&g).constants();
        let root = rules(&g).swap_remove(g.root);
        for _ in 0..300 {
            let call = random_call(&mut rng, count);
            let (error, _, bound, d) = outcome(&g, &root, &c, &call);
            let in_range = (0..3)
                .filter(|&k| c.range_axes[k])
                .all(|k| c.d_min <= d[k] && d[k] <= c.d_max);
            if in_range {
                assert!(error <= Dyadic::from_f64(bound), "{source} at {d:?}");
            }
        }
    }
}
