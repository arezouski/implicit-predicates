//! The error bound of the floating-point filter of a predicate.
//!
//! The filter evaluates the difference form in `f64` and accepts the sign of
//! the result when its magnitude exceeds a bound on the rounding error. The
//! bound is derived here, node by node, in exact arithmetic.

use crate::differences::{DiffGraph, DiffNode};
use crate::dyadic::Dyadic;
use crate::graph::NodeId;

/// The bounds of one node of degree `g`, in terms of `D` and `E`:
/// `|true value| <= m * D^g`, `|computed value| <= c * D^g` and
/// `|computed - true| <= (alpha * D + beta * E) * D^(g - 1)`.
#[derive(Clone, Debug)]
pub(crate) struct NodeBound {
    pub(crate) degree: u32,
    pub(crate) m: Dyadic,
    pub(crate) c: Dyadic,
    pub(crate) alpha: Dyadic,
    pub(crate) beta: Dyadic,
}

/// The exact error bound of the `f64` evaluation of a difference form.
///
/// For a call, let `delta` be the largest magnitude of the computed leaves
/// (the differences of the coordinates of the approximations of the points),
/// `E` the sum of the two largest errors of the approximations, and
/// `D = delta + E`. The `f64` value of the form differs from the exact value
/// at the exact points by at most `(epsilon * D + kappa * E) * D^(k - 1)`,
/// where `k` is the degree, as long as no product underflows; see
/// [`FilterBound::constants`] for the range in which underflow and overflow
/// are accounted for.
#[derive(Clone, Debug)]
pub struct FilterBound {
    /// The degree `k` of the form in the differences.
    pub degree: u32,
    /// `epsilon / u`, with `u = 2^-53`, as an exact fraction.
    pub epsilon_in_u: String,
    /// `kappa` as an exact fraction.
    pub kappa_exact: String,
    /// The number of products `N` in the form; a product used several times
    /// is counted once.
    pub products: u32,
    nodes: Vec<NodeBound>,
    root: NodeId,
}

/// The constants of the filter of a predicate, as `f64`.
///
/// The filter decides when `d_min <= D <= d_max` and
/// `|value| > (epsilon * D + kappa * E) * D^(degree - 1)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FilterConstants {
    /// The degree `k` of the form.
    pub degree: u32,
    /// The coefficient of `D^k`, rounded up.
    pub epsilon: f64,
    /// The coefficient of `E * D^(k - 1)`, rounded up.
    pub kappa: f64,
    /// The smallest `D` for which the bound covers underflow: a power of two
    /// below 1.
    pub d_min: f64,
    /// The largest `D` for which no node overflows: a power of two above 1.
    pub d_max: f64,
}

/// Derives the error bound of the `f64` evaluation of `graph`.
///
/// With `u = 2^-53`, every node of degree `g` gets coefficients `m`, `c`,
/// `alpha` and `beta` such that `|true value| <= m * D^g`,
/// `|computed value| <= c * D^g` and
/// `|computed - true| <= (alpha * D + beta * E) * D^(g - 1)`:
///
/// - a leaf has `g = 1`, `m = 1 + u`, `c = 1`, `alpha = u`, `beta = 1`;
/// - a negation is exact and keeps the coefficients of its operand;
/// - the sum or difference of `x` and `y`, of equal degree, has
///   `m = m_x + m_y`, `c = (c_x + c_y)(1 + u)`,
///   `alpha = alpha_x + alpha_y + u (c_x + c_y)` and `beta = beta_x + beta_y`;
/// - the product of `x` and `y` has `g = g_x + g_y`, `m = m_x m_y`,
///   `c = c_x c_y (1 + u)`, `alpha = c_x alpha_y + m_y alpha_x + u c_x c_y`
///   and `beta = c_x beta_y + m_y beta_x`.
///
/// The root gives `epsilon = alpha` and `kappa = beta`. Every node is
/// analysed once, in the order of the graph, so a shared subexpression counts
/// as the single computation it is.
pub fn filter_bound(graph: &DiffGraph) -> FilterBound {
    let u = Dyadic::pow2(-53);
    let one = Dyadic::integer(1);
    let one_plus_u = &one + &u;
    let mut nodes: Vec<NodeBound> = Vec::with_capacity(graph.nodes.len());
    for (id, node) in graph.nodes.iter().enumerate() {
        let bound = match *node {
            DiffNode::Leaf { .. } => NodeBound {
                degree: 1,
                m: one_plus_u.clone(),
                c: one.clone(),
                alpha: u.clone(),
                beta: one.clone(),
            },
            DiffNode::Neg(x) => nodes[x].clone(),
            DiffNode::Add(x, y) | DiffNode::Sub(x, y) => {
                let (x, y) = (&nodes[x], &nodes[y]);
                let c_sum = &x.c + &y.c;
                NodeBound {
                    degree: x.degree,
                    m: &x.m + &y.m,
                    c: &c_sum * &one_plus_u,
                    alpha: &(&x.alpha + &y.alpha) + &(&u * &c_sum),
                    beta: &x.beta + &y.beta,
                }
            }
            DiffNode::Mul(x, y) => {
                let (x, y) = (&nodes[x], &nodes[y]);
                let c_product = &x.c * &y.c;
                NodeBound {
                    degree: x.degree + y.degree,
                    m: &x.m * &y.m,
                    c: &c_product * &one_plus_u,
                    alpha: &(&(&x.c * &y.alpha) + &(&y.m * &x.alpha)) + &(&u * &c_product),
                    beta: &(&x.c * &y.beta) + &(&y.m * &x.beta),
                }
            }
        };
        debug_assert_eq!(bound.degree, graph.degrees[id]);
        nodes.push(bound);
    }
    let products = graph
        .nodes
        .iter()
        .filter(|n| matches!(n, DiffNode::Mul(..)))
        .count();
    let root = &nodes[graph.root];
    FilterBound {
        degree: root.degree,
        epsilon_in_u: (&root.alpha * &Dyadic::pow2(53)).fraction(),
        kappa_exact: root.beta.fraction(),
        products: u32::try_from(products).expect("fewer than 2^32 products"),
        nodes,
        root: graph.root,
    }
}

impl FilterBound {
    /// Whether `n u <= epsilon < n u (1 + 2^-40)`, exactly.
    ///
    /// The exact `epsilon` is slightly above an integer multiple of `u`,
    /// because of the terms in `u^2` and beyond.
    pub fn epsilon_is_u_times(&self, n: u64) -> bool {
        let target = &Dyadic::integer(n) * &Dyadic::pow2(-53);
        close_above(&self.nodes[self.root].alpha, &target)
    }

    /// Whether `n <= kappa < n (1 + 2^-40)`, exactly.
    pub fn kappa_is(&self, n: u64) -> bool {
        close_above(&self.nodes[self.root].beta, &Dyadic::integer(n))
    }

    /// The bound on the magnitude of the computed value of node `id`, as the
    /// coefficient `c` of `D^g`.
    pub(crate) fn magnitude(&self, id: NodeId) -> &Dyadic {
        &self.nodes[id].c
    }

    /// The constants of the filter.
    ///
    /// `epsilon` and `kappa` are the exact values times `1 + 2^-30`, rounded
    /// up; the inflation covers the rounding of the evaluation of the bound
    /// itself and the underflow allowance described at `d_min`.
    ///
    /// # Panics
    ///
    /// Panics if the degree is `2^20` or more, or if the form is so large
    /// that no `D > 1` avoids overflow or no `D < 1` covers underflow.
    pub fn constants(&self) -> FilterConstants {
        assert!(
            self.degree < 1 << 20,
            "the degree is too large for the bound"
        );
        let inflate = &Dyadic::integer(1) + &Dyadic::pow2(-30);
        let root = &self.nodes[self.root];
        FilterConstants {
            degree: self.degree,
            epsilon: (&root.alpha * &inflate).round_up(),
            kappa: (&root.beta * &inflate).round_up(),
            d_min: Dyadic::pow2(self.d_min_exponent()).round_up(),
            d_max: Dyadic::pow2(self.d_max_exponent()).round_up(),
        }
    }

    /// The exponent of `d_max`: the largest `j` such that
    /// `c_x * 2^(j g_x) <= 2^1022` at every node `x`.
    ///
    /// For `D <= d_max` every computed node then has magnitude at most
    /// `c_x * D^(g_x) <= 2^1022` (for `D < 1`, `D^(g_x) <= 1 < d_max^(g_x)`),
    /// half the overflow threshold, so no intermediate value and not the
    /// result overflows. An overflow in the evaluation of the bound itself
    /// gives an infinite bound, which only makes the filter abstain.
    fn d_max_exponent(&self) -> i64 {
        // `c_x * 2^(j g) <= 2^1022` iff `j g <= 1022 - s` with `s` the
        // smallest integer such that `c_x <= 2^s`.
        let j = (0..self.nodes.len())
            .map(|id| {
                let degree = i64::from(self.nodes[id].degree);
                (1022 - self.magnitude(id).ceil_log2()).div_euclid(degree)
            })
            .min()
            .expect("a graph has a root");
        assert!(j >= 1, "the form overflows for every D above 1");
        j
    }

    /// The exponent of `d_min`: the smallest `j` such that `P = 2^j`
    /// satisfies
    ///
    /// 1. `epsilon * P^k * 2^-32 >= N * k * c_max * 2^-1074`, with `N` the
    ///    number of products and `c_max` the largest `c` of any node, and
    /// 2. `epsilon * P^k >= 2^-1021`.
    ///
    /// Both then hold for every larger `j`.
    ///
    /// Why this is sound, for `D >= d_min` and with `B` the rounding-error
    /// bound `(epsilon * D + kappa * E) * D^(k - 1)`:
    ///
    /// - The node rules assume that every rounding error is at most `u` times
    ///   the computed magnitude. A sum or difference whose result is
    ///   subnormal is exact, but a product that underflows can err by up to
    ///   `2^-1075` in absolute terms; allow `2^-1074` per product.
    /// - An absolute error at a product `x` reaches the root multiplied by
    ///   the derivative of the computation with respect to `x`: a sum over
    ///   the paths from `x` to the root of the products of the other factors
    ///   met on the way, each with its `(1 + u)` rounding. Every factor is
    ///   bounded by its `c * D^g`, so the derivative is at most
    ///   `p'(c_x) * D^(k - g_x)`, where `p(t)` is `c_root` computed by the
    ///   rules with `c_x` replaced by `t`. That polynomial has nonnegative
    ///   coefficients and degree at most `k / g_x`, since degrees add along
    ///   products, so `c_x p'(c_x) <= (k / g_x) p(c_x)`; with `c_x >= 1` and
    ///   `g_x >= 2` the derivative is at most `k * c_max * D^(k - g_x)`.
    ///   Terms of second order in these errors are below `2^-2000` and are
    ///   absorbed by the slack below.
    /// - The total underflow error is therefore
    ///   `U <= N * k * c_max * 2^-1074 * max(1, D^k)`. For `d_min <= D <= 1`,
    ///   condition 1 gives `U <= 2^-32 * epsilon * d_min^k
    ///   <= 2^-32 * epsilon * D^k`; for `D > 1` it gives the same, because
    ///   condition 1 holds with `d_min^k < 1`. So `U <= 2^-32 * B`, and the
    ///   value of the form errs by at most `(1 + 2^-32) * B`.
    /// - The emitted constants are at least `(1 + 2^-30)` times the exact
    ///   ones, so the exact value of the emitted bound is at least
    ///   `(1 + 2^-30) * B`. Condition 2 keeps its `f64` evaluation out of the
    ///   subnormal range: every partial result built from `epsilon * D` is at
    ///   least `epsilon * min(1, D)^k >= 2^-1021`, so each of its about
    ///   `k + 2` operations errs by at most a factor `1 + u`, and the term
    ///   `kappa * E`, which may underflow, loses at most `2^-1075`, a
    ///   fraction `2^-54` of the sum it joins. For `k < 2^20` the computed
    ///   bound is at least `(1 + 2^-30)(1 - u)^(k + 4) * B >= (1 + 2^-31) * B`,
    ///   which exceeds the error `(1 + 2^-32) * B`.
    fn d_min_exponent(&self) -> i64 {
        let epsilon = &self.nodes[self.root].alpha;
        let c_max = (0..self.nodes.len())
            .map(|id| self.magnitude(id))
            .max()
            .expect("a graph has a root");
        let factor = Dyadic::integer(u64::from(self.products) * u64::from(self.degree));
        let underflow = &(&factor * c_max) * &Dyadic::pow2(-1074 + 32);
        let normal = Dyadic::pow2(-1021);
        let k = i64::from(self.degree);
        let covers = |j: i64| {
            let scaled = epsilon * &Dyadic::pow2(j * k);
            scaled >= underflow && scaled >= normal
        };
        assert!(covers(-1), "the form underflows for every D below 1");
        let mut j = -1;
        while covers(j - 1) {
            j -= 1;
        }
        j
    }
}

/// Whether `target <= value < target * (1 + 2^-40)`.
fn close_above(value: &Dyadic, target: &Dyadic) -> bool {
    let limit = target * &(&Dyadic::integer(1) + &Dyadic::pow2(-40));
    target <= value && *value < limit
}

#[cfg(test)]
mod tests;
