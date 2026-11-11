//! The per-axis form of the error bound of the filter.
//!
//! A form is multihomogeneous when every sum and difference combines nodes
//! of the same degree in each axis (see [`DiffGraph::axis_degrees`]). For
//! such a form the bound keeps one size and one uncertainty per axis
//! instead of a single one. For a call, on each axis `k` (0, 1 and 2 for x,
//! y and z), let `delta_k` be the largest magnitude of the computed leaves of
//! that axis, `E_k` the sum of the two largest errors of the points on that
//! axis, and `D_k = delta_k + E_k`. A node has a degree vector
//! `g = (g_0, g_1, g_2)`; write `D^g = D_0^g_0 * D_1^g_1 * D_2^g_2` and `e_k`
//! for the unit vector of axis `k`. Every node gets exact coefficients `m`,
//! `c`, `alpha` and one `beta_k` per axis such that
//!
//! ```text
//! |true value|      <= m * D^g
//! |computed value|  <= c * D^g
//! |computed - true| <= alpha * D^g + sum over k of beta_k * E_k * D^(g - e_k)
//! ```
//!
//! with `beta_k = 0` whenever `g_k = 0`. The rules are those of the global
//! form ([`filter_bound`]) with `beta` split by axis:
//!
//! - a leaf of axis `k` has `g = e_k`, `m = 1 + u`, `c = 1`, `alpha = u`,
//!   `beta_k = 1` and the other two `beta` zero: its computed value is at
//!   most `delta_k <= D_k`, and it differs from the true one by the rounding
//!   of the subtraction, at most `u delta_k`, plus the errors of its two
//!   points on axis `k`, at most `E_k`;
//! - a negation is exact and keeps the coefficients of its operand;
//! - a sum or difference of `x` and `y`, of equal degree vectors, has the
//!   `m`, `c` and `alpha` of the global form and `beta_k = beta_x,k +
//!   beta_y,k`;
//! - a product of `x` and `y` adds the degree vectors, has the `m`, `c` and
//!   `alpha` of the global form and `beta_k = c_x beta_y,k + m_y beta_x,k`,
//!   from `x~ y~ - x y = x~ (y~ - y) + y (x~ - x)`: the factor
//!   `D^(g_y - e_k)` of an error term of `y` times the `D^(g_x)` of `x~`
//!   gives `D^(g - e_k)`.
//!
//! The root gives `epsilon = alpha` and `kappa_k = beta_k`. The rules for
//! `m`, `c` and `alpha` are those of the global form, and those for `beta`
//! are linear with the same coefficients, so `epsilon` is the `epsilon` of
//! the global form and the `kappa_k` add up to its `kappa`.
//!
//! # Range of validity
//!
//! The node rules assume that no value overflows and that every rounding
//! error is at most `u` times the computed magnitude. With `G` the degree
//! vector of the root and `K` its total degree (the degree of the global
//! form), let `d_min = 2^j_min` and `d_max = 2^j_max` be the powers of two
//! that [`FilterBound::constants`] gives for the same graph: `j_max` the
//! largest `j >= 1` with `c_x * 2^(j |g_x|) <= 2^1022` at every node `x`,
//! where `|g_x|` is the total degree of `x`, and `j_min` the smallest
//! `j <= -1` with, for `P = 2^j`,
//!
//! 1. `epsilon * P^K * 2^-32 >= N * K * c_max * 2^-1074`, with `N` the number
//!    of products and `c_max` the largest `c`, and
//! 2. `epsilon * P^K >= 2^-1021`.
//!
//! **Claim.** Let the filter compute `d_k = delta_k + E_k` in `f64` on every
//! axis `k` with `G_k >= 1` and accept the bound only when
//! `d_min <= d_k <= d_max` on each of them, with `E_k` finite, nonnegative and
//! not below the true sum of the two largest errors. Let it evaluate the
//! bound in `f64` as a sum, in any order, of the terms
//! `EPSILON * D^G` and, for each axis with `G_k >= 1`,
//! `KAPPA_k * E_k * D^(G - e_k)`, each term a chain of products that starts
//! with its constant and multiplies the factors in any order, with the `d_k`
//! in place of the `D_k` and the emitted constants `EPSILON` and `KAPPA_k`,
//! at least `1 + 2^-30` times the exact ones. Then the computed bound is at
//! least the error of the computed value of the form. The axes with
//! `G_k = 0` take no part: no leaf and no factor uses them.
//!
//! **Proof.** Write `D_k = d_k / (1 - u)`. A sum of nonnegative values rounds
//! down by at most a factor `1 - u` (it is exact when the result is
//! subnormal), so `D_k >= delta_k + E_k`. The node bounds hold for any
//! `D_k >= delta_k + E_k`, since they only grow with `D_k`, so they hold with
//! these `D_k`, and `d_k >= d_min` gives `D_k >= d_min`. Let `B` be the exact
//! bound `epsilon * D^G + sum over k of kappa_k * E_k * D^(G - e_k)` with
//! these `D_k`.
//!
//! - Overflow. Every computed node is at most `c_x * (delta + E)^(g_x)`, and
//!   on each axis `delta_k + E_k <= D_k <= d_max / (1 - u)`, so it is at most
//!   `c_x * 2^(j_max |g_x|) * (1 - u)^-|g_x| <= 2^1022 * (1 - u)^-1022`, below
//!   `2^1023`: no node overflows. An overflow in the evaluation of the bound
//!   gives an infinite bound, or a NaN one when an infinite partial product
//!   is then multiplied by an `E_k` of zero; either only makes the filter
//!   abstain, since it decides on `|value| > bound`, false for both.
//! - Underflow of the form. A sum or difference whose result is subnormal is
//!   exact; a product that underflows errs by at most `2^-1075` in absolute
//!   terms; allow `2^-1074`. As in the global form, that error reaches the
//!   root multiplied by at most `p'(c_x)` times the product of the bounds on
//!   the other factors met on the way, where `p(t)` is `c_root` computed by
//!   the rules with `c_x` replaced by `t`. Degree vectors add along products,
//!   so the other factors on any path from `x` to the root have degree vector
//!   `G - g_x` and their bounds multiply to `D^(G - g_x)`; and as in the
//!   global form `p'(c_x) <= K * c_max`. So the total underflow error is at
//!   most `U = sum over the products x of K * c_max * 2^-1074 * D^(G - g_x)`.
//!   On every axis with `g_x,k >= 1` the root has `G_k >= 1`, so `D_k >= P`
//!   with `P = d_min < 1`: the factors with `D_k >= 1` are at least 1 and the
//!   others at least `P`, so `D^(g_x) >= P^|g_x| >= P^K`, and
//!   `D^(G - g_x) <= D^G / P^K`. Condition 1 then gives
//!   `U <= N * K * c_max * 2^-1074 * D^G / P^K <= 2^-32 * epsilon * D^G
//!   <= 2^-32 * B`. Terms of second order in these errors are smaller by a
//!   factor of about `2^-1000` and are absorbed by the same slack, as in the
//!   global form. The computed value of the form errs by at most
//!   `(1 + 2^-32) * B`.
//! - Rounding of the bound. Each term is a chain of `K` products of
//!   nonnegative values, and its factors `d_k` are `(1 - u) D_k`, exactly.
//!   While no partial product is subnormal, the computed term is at least the
//!   exact term with the `D_k` times `(1 - u)^(2K)`: `K` roundings and at most
//!   `K` uses of a `d_k`. The terms are summed with at most 3 more roundings,
//!   so the computed bound is at least `(1 + 2^-30) (1 - u)^(2K + 3) B`,
//!   which is at least `(1 + 2^-31) B` for `K <= 1022`, as in the global
//!   form, less the losses to underflow.
//! - Underflow of the bound. A partial product of the term of `EPSILON` is
//!   `EPSILON` times at most `K` of the `d_k`, each `d_k >= P` or above 1,
//!   so before rounding it is at least `epsilon * P^K`, at least `2^-1021`
//!   by condition 2, and after at most `K` roundings at least
//!   `(1 - u)^K * 2^-1021 > 2^-1022`: those partial products are normal. A
//!   partial product of the term of `KAPPA_k` may underflow, and loses at
//!   most `2^-1075`, which the remaining factors then multiply. Those factors
//!   are `d` values and perhaps `E_k <= D_k`, so the loss is at most
//!   `2^-1075 * (1 + u)^K * D^G / Q`, where `Q` is the product of the `D_k`
//!   of the factors already multiplied, counting `D_k` for `E_k`. As above,
//!   `Q >= P^K`, and condition 2 gives `1 / P^K <= 2^1021 * epsilon`, so the
//!   loss is at most `2^-54 * (1 + u)^K * epsilon * D^G <= 2^-53 * B`. At
//!   most `3K` partial products can underflow, so the losses are at most
//!   `3 * 1022 * 2^-53 * B < 2^-41 * B`.
//!
//! The computed bound is then at least `(1 + 2^-31 - 2^-41) * B`, more than
//! the error `(1 + 2^-32) * B`. The conditions on `P` are those of the global
//! form with `k = K`, so `d_min` and `d_max` are the same two powers of two.
//!
//! A condition on the product of the `D_k` alone is not enough: with
//! `D_y = D_z = 2^-520` and `D_x = 2^100` the product is far from the limits,
//! but a product of a leaf of y and a leaf of z is subnormal, and its
//! rounding error, multiplied by a leaf of x, exceeds the bound. The proof
//! above needs `D^(g_x) >= P^K` at every product, which bounding each `D_k`
//! separately gives.
//!
//! An axis with `D_k = 0` (every leaf of the axis is zero and `E_k = 0`) is
//! below the range; [`AxisBound::certain_zero`] says when the value of the
//! form is then exactly zero.

use super::{
    FilterBound, check_budget, close_above, d_max_exponent, d_min_exponent, filter_bound,
    underflow_message,
};
use crate::differences::{DiffGraph, DiffNode};
use crate::dyadic::Dyadic;
use crate::error::Error;
use crate::graph::NodeId;
use crate::parse::Position;

/// The form of the error bound of the filter of a predicate.
#[derive(Clone, Debug)]
pub enum FilterForm {
    /// One size and one uncertainty per axis, for a multihomogeneous form.
    PerAxis(AxisBound),
    /// One size and one uncertainty for all the axes.
    Global(FilterBound),
}

/// Derives the error bound of the `f64` evaluation of `graph`: the per-axis
/// form when every term has the same degree in each axis, and the global
/// form of [`filter_bound`] otherwise.
///
/// The per-axis bound follows the rules of the global form with one `beta`
/// per axis; the documentation of [`AxisBound`] gives them, and its range of
/// validity.
///
/// # Errors
///
/// The errors of [`filter_bound`], with the same limits, for either form:
/// the per-axis form has the same degrees, the same `m`, `c` and `alpha`,
/// and the same range as the global one.
pub fn filter_form(graph: &DiffGraph, file: &str, position: Position) -> Result<FilterForm, Error> {
    match graph.node_axis_degrees() {
        Some(degrees) => axis_bound(graph, &degrees, file, position).map(FilterForm::PerAxis),
        None => filter_bound(graph, file, position).map(FilterForm::Global),
    }
}

/// The bounds of one node of degree vector `degrees`, in terms of the `D_k`
/// and `E_k`: `|true value| <= m * D^g`, `|computed value| <= c * D^g` and
/// `|computed - true| <= alpha * D^g + sum of beta_k * E_k * D^(g - e_k)`.
#[derive(Clone, Debug)]
pub(crate) struct AxisNode {
    pub(crate) degrees: [u32; 3],
    pub(crate) m: Dyadic,
    pub(crate) c: Dyadic,
    pub(crate) alpha: Dyadic,
    pub(crate) beta: [Dyadic; 3],
}

/// The exact per-axis error bound of the `f64` evaluation of a
/// multihomogeneous difference form.
///
/// For a call, with `D_k = delta_k + E_k` on each axis (see the
/// documentation of this module, in the source, for the definitions and the
/// proof), the `f64` value of the form differs from the exact value at the
/// exact points by at most
/// `epsilon * D^G + sum over k of kappa_k * E_k * D^(G - e_k)`, where `G` is
/// [`AxisBound::degrees`], while on every axis of positive degree the size
/// computed in `f64`, `d_k = delta_k + E_k`, is in the range of
/// [`AxisBound::constants`].
#[derive(Clone, Debug)]
pub struct AxisBound {
    /// The degree `G_k` of the form in the differences of each axis.
    pub degrees: [u32; 3],
    /// `epsilon / u`, with `u = 2^-53`, as an exact fraction.
    pub epsilon_in_u: String,
    /// `kappa_k` of each axis as an exact fraction; zero on an axis of
    /// degree zero.
    pub kappa_exact: [String; 3],
    nodes: Vec<AxisNode>,
    root: NodeId,
    d_min_exponent: i64,
    d_max_exponent: i64,
}

/// The constants of the per-axis filter of a predicate, as `f64`.
///
/// The filter decides when `d_min <= d_k <= d_max` on every axis `k` with
/// `degrees[k] >= 1`, where `d_k = delta_k + E_k` is computed in `f64`, and
/// `|value| > epsilon * d^G + sum over k of kappa[k] * E_k * d^(G - e_k)`,
/// evaluated as a sum of chains of products, each starting with its
/// constant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisConstants {
    /// The degree `G_k` of the form in each axis.
    pub degrees: [u32; 3],
    /// The coefficient of `D^G`, rounded up.
    pub epsilon: f64,
    /// The coefficient of `E_k * D^(G - e_k)` of each axis, rounded up; zero
    /// on an axis of degree zero.
    pub kappa: [f64; 3],
    /// The smallest `d_k`, computed in `f64` as `delta_k + E_k`, on each axis
    /// of positive degree, for which the bound covers underflow: a power of
    /// two below 1.
    pub d_min: f64,
    /// The largest `d_k`, computed in `f64` as `delta_k + E_k`, on each axis
    /// of positive degree, for which no node overflows: a power of two above
    /// 1.
    pub d_max: f64,
}

/// Applies the per-axis rules to `graph`, whose nodes have the degree
/// vectors `degrees`.
fn axis_bound(
    graph: &DiffGraph,
    degrees: &[[u32; 3]],
    file: &str,
    position: Position,
) -> Result<AxisBound, Error> {
    // The per-axis coefficients are those of the global form, with `beta`
    // split in three parts that are each at most the global `beta`: the
    // same estimate bounds their size.
    check_budget(graph, file, position)?;
    let u = Dyadic::pow2(-53);
    let zero = Dyadic::integer(0);
    let one = Dyadic::integer(1);
    let one_plus_u = &one + &u;
    let mut nodes: Vec<AxisNode> = Vec::with_capacity(graph.nodes.len());
    for (node, &g) in graph.nodes.iter().zip(degrees) {
        let bound = match *node {
            DiffNode::Leaf { axis, .. } => AxisNode {
                degrees: g,
                m: one_plus_u.clone(),
                c: one.clone(),
                alpha: u.clone(),
                beta: [0, 1, 2].map(|k| {
                    if k == usize::from(axis) {
                        one.clone()
                    } else {
                        zero.clone()
                    }
                }),
            },
            DiffNode::Neg(x) => nodes[x].clone(),
            DiffNode::Add(x, y) | DiffNode::Sub(x, y) => {
                let (x, y) = (&nodes[x], &nodes[y]);
                let c_sum = &x.c + &y.c;
                AxisNode {
                    degrees: g,
                    m: &x.m + &y.m,
                    c: &c_sum * &one_plus_u,
                    alpha: &(&x.alpha + &y.alpha) + &(&u * &c_sum),
                    beta: [0, 1, 2].map(|k| &x.beta[k] + &y.beta[k]),
                }
            }
            DiffNode::Mul(x, y) => {
                let (x, y) = (&nodes[x], &nodes[y]);
                let c_product = &x.c * &y.c;
                AxisNode {
                    degrees: g,
                    m: &x.m * &y.m,
                    c: &c_product * &one_plus_u,
                    alpha: &(&(&x.c * &y.alpha) + &(&y.m * &x.alpha)) + &(&u * &c_product),
                    beta: [0, 1, 2].map(|k| &(&x.c * &y.beta[k]) + &(&y.m * &x.beta[k])),
                }
            }
        };
        nodes.push(bound);
    }
    let products = graph
        .nodes
        .iter()
        .filter(|n| matches!(n, DiffNode::Mul(..)))
        .count();
    // As in the global form, the budget allows fewer than 2^19 nodes.
    let products = u32::try_from(products).expect("fewer than 2^32 products");
    let root = &nodes[graph.root];
    let total = graph.degrees[graph.root];
    let d_max_exponent = d_max_exponent(nodes.iter().zip(&graph.degrees).map(|(n, &g)| (&n.c, g)))
        .map_err(|degree| Error::at(file, position, super::overflow_message(degree)))?;
    let c_max = nodes
        .iter()
        .map(|n| &n.c)
        .max()
        .expect("a graph has a root");
    let d_min_exponent = d_min_exponent(&root.alpha, c_max, total, products)
        .ok_or_else(|| Error::at(file, position, underflow_message(total)))?;
    Ok(AxisBound {
        degrees: root.degrees,
        epsilon_in_u: (&root.alpha * &Dyadic::pow2(53)).fraction(),
        kappa_exact: [0, 1, 2].map(|k| root.beta[k].fraction()),
        nodes,
        root: graph.root,
        d_min_exponent,
        d_max_exponent,
    })
}

impl AxisBound {
    /// Whether `n u <= epsilon < n u (1 + 2^-40)`, exactly.
    pub fn epsilon_is_u_times(&self, n: u64) -> bool {
        let target = &Dyadic::integer(n) * &Dyadic::pow2(-53);
        close_above(&self.nodes[self.root].alpha, &target)
    }

    /// Whether `n <= kappa_k < n (1 + 2^-40)` on the axis `axis`, exactly;
    /// false for an axis above 2.
    pub fn kappa_is(&self, axis: usize, n: u64) -> bool {
        self.nodes[self.root]
            .beta
            .get(axis)
            .is_some_and(|beta| close_above(beta, &Dyadic::integer(n)))
    }

    /// Whether every term has degree at least 1 in every axis, so that the
    /// form is exactly zero when every leaf of one axis is exactly zero: a
    /// sum of nodes of positive degree in that axis is zero when both are,
    /// and a product has a factor of positive degree in it.
    pub fn certain_zero(&self) -> bool {
        self.degrees.iter().all(|&g| g >= 1)
    }

    /// The constants of the filter.
    ///
    /// `epsilon` and each `kappa` are the exact values times `1 + 2^-30`,
    /// rounded up. `d_min` and `d_max` are the powers of two of the range of
    /// validity, derived in the documentation of this module (in the source):
    /// those of the global form of the same graph.
    pub fn constants(&self) -> AxisConstants {
        let inflate = &Dyadic::integer(1) + &Dyadic::pow2(-30);
        let root = &self.nodes[self.root];
        AxisConstants {
            degrees: self.degrees,
            epsilon: (&root.alpha * &inflate).round_up(),
            kappa: [0, 1, 2].map(|k| (&root.beta[k] * &inflate).round_up()),
            d_min: Dyadic::pow2(self.d_min_exponent).round_up(),
            d_max: Dyadic::pow2(self.d_max_exponent).round_up(),
        }
    }
}

#[cfg(test)]
#[path = "axis_tests.rs"]
mod tests;
