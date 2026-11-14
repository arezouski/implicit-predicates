//! The error bound of the filter by parts.
//!
//! The per-axis form of the bound ([`AxisBound`](super::AxisBound)) needs a
//! multihomogeneous form, whose terms all have the same degree in each axis.
//! A form such as the in-sphere determinant is not one, but it is a sum of
//! parts that are: its terms have the degree vectors (3, 1, 1), (1, 3, 1) and
//! (1, 1, 3). The bound by parts keeps one size per axis for such a form by
//! giving every node polynomials in the sizes of the axes instead of single
//! coefficients.
//!
//! # The rules
//!
//! For a call, on each axis `k` (0, 1 and 2 for x, y and z), let `delta_k` be
//! the largest magnitude of the computed leaves of that axis, `E_k` the sum
//! of the two largest errors of the points on that axis (or any upper bound
//! on it), and `D_k >= delta_k + E_k`. Write `D^s = D_0^s_0 * D_1^s_1 *
//! D_2^s_2` for a degree vector `s`, `|s|` for its total degree, `e_k` for the
//! unit vector of axis `k`, and `u = 2^-53`. Every node `x` gets four
//! polynomials in `D = (D_0, D_1, D_2)` with exact, nonnegative coefficients,
//! `M_x`, `C_x`, `A_x` and one `B_x,k` per axis, such that
//!
//! ```text
//! |true value|      <= M_x(D)
//! |computed value|  <= C_x(D)
//! |computed - true| <= A_x(D) + sum over k of E_k * B_x,k(D)
//! ```
//!
//! where the true value is the value of the node at the exact points and the
//! computed one its value in `f64` at the approximations, while no operation
//! overflows or underflows. The rules, with all the operations on
//! polynomials exact, are
//!
//! - a leaf of axis `k`: `M = (1 + u) D_k`, `C = D_k`, `A = u D_k`,
//!   `B_k = 1` and the other two `B` zero. Its computed value is at most
//!   `delta_k <= D_k`; it differs from the true one by the rounding of the
//!   subtraction, at most `u delta_k`, and by the errors of its two points on
//!   axis `k`, at most `E_k`; so the true value is at most
//!   `(1 + u) delta_k + E_k <= (1 + u) D_k`;
//! - a negation is exact and keeps the four polynomials of its operand;
//! - a sum or difference of `x` and `y`: `M = M_x + M_y`,
//!   `C = (C_x + C_y)(1 + u)`, `A = A_x + A_y + u (C_x + C_y)` and
//!   `B_k = B_x,k + B_y,k`, since the rounding of the result errs by at most
//!   `u` times the exact sum of the computed operands;
//! - a product of `x` and `y`: `M = M_x M_y`, `C = C_x C_y (1 + u)`,
//!   `A = C_x A_y + M_y A_x + u C_x C_y` and `B_k = C_x B_y,k + M_y B_x,k`,
//!   from `x~ y~ - x y = x~ (y~ - y) + y (x~ - x)`, with `|x~| <= C_x`,
//!   `|y| <= M_y`, plus the rounding of the product, at most `u |x~ y~|`.
//!
//! These are the rules of the global form ([`filter_bound`]) with
//! polynomials in place of the coefficients; they hold because every bound
//! is a polynomial with nonnegative coefficients, so it only grows with the
//! `D_k`. At the root, the bound is `A(D) + sum over k of E_k B_k(D)`: each
//! monomial `epsilon_g D^g` of `A` is a term of epsilon, of degree vector
//! `g`, and each monomial `kappa_k,h D^h` of `B_k` a term of kappa, of axis
//! `k` and degree vector `g = h + e_k` (the rest `h` is `g - e_k`).
//!
//! # Supports
//!
//! The proof below uses four facts about the supports, the sets of degree
//! vectors of the monomials. Let `S_x` be the support of `C_x`, and `V` that
//! of the root, the degree vectors of the root.
//!
//! - (S1) `M_x`, `C_x` and `A_x` all have the support `S_x`: true for a leaf,
//!   and kept by the rules, since no coefficient is negative and none
//!   cancels. Every coefficient of `C_x` and of every `B_x,k` is at least 1
//!   (it is 1 at a leaf; the rules add such coefficients, multiply them, or
//!   multiply them by `1 + u`).
//! - (S2) Every vector of `S_x` has total degree `|g_x|`, the degree of the
//!   node, and for every monomial `D^h` of `B_x,k` the vector `h + e_k` is in
//!   `S_x`: true for a leaf, and kept by the rules (a product term
//!   `C_x B_y,k` has vectors `s + h` with `s` in `S_x` and `h + e_k` in
//!   `S_y`). So every term of kappa has a degree vector in `V`, and there are
//!   at most `4 |V|` terms in all: `|V|` of epsilon and at most `|V|` on each
//!   axis.
//! - (S3) For a node `x` on which the root depends, every vector of `S_x` is
//!   at most (in each component) some vector of `V`: true at the root, and
//!   passed down to the operands, since a sum has the union of the supports
//!   of its operands and a product the sums `s + t` of a vector of each. So
//!   such a node has total degree at most `K`, the degree of the root, and
//!   uses an axis `k` only if some vector of `V` has `g_k >= 1`. These are
//!   the axes of [`PartsBound::range_axes`]; call them `R`.
//! - (S4) Evaluated at `D = (1, 1, 1)`, the polynomials give the global
//!   coefficients `m`, `c`, `alpha` and `beta = sum over k of B_x,k(1, 1, 1)`
//!   of the same node, as polynomials in `u`, since the global rules are these
//!   rules with every `D_k = 1`. In particular the terms of epsilon add up to
//!   the global `epsilon` and those of kappa to the global `kappa`, and
//!   every coefficient of `C_x` is at most the global `c_x`.
//!
//! # Range of validity
//!
//! Let `K` be the degree of the root, `N` the number of products of the
//! graph, `c_max` the largest global `c` of any node, `epsilon_min` the
//! smallest coefficient of a term of epsilon, and `T` the number of terms,
//! at most `4 * 16 = 64` because [`parts_bound`] declines a form with more
//! than 16 degree vectors at the root. Let `d_max = 2^j_max` be the `d_max`
//! of the global form ([`FilterBound::constants`]): `j_max` is the largest
//! `j >= 1` with `c_x * 2^(j |g_x|) <= 2^1022` at every node. Let
//! `d_min = 2^j_min`, with `j_min` the smallest `j <= -1` such that
//! `P = 2^j` satisfies
//!
//! 1. `epsilon_min * P^K * 2^-32 >= N * K * c_max * 2^-1074`,
//! 2. `epsilon_min * P^K >= 2^-1021`, and
//! 3. `P^K >= 2^-1021`.
//!
//! Each condition holds for every larger `j` once it holds for one. The first
//! two are those of the global form with `epsilon_min` for `epsilon`; the
//! third follows from the second whenever `epsilon_min <= 1`, which is the
//! case of every form of use (for in-sphere `epsilon_min` is about
//! `358 u`), and is there to bound the underflow of the form below without
//! a bound on `epsilon_min`. If no `j <= -1` satisfies all three, there is
//! no range and [`parts_bound`] gives no bound.
//!
//! **Claim.** Let the filter compute `d_k = delta_k + E_k` in `f64` on every
//! axis `k` of `R`, and accept the bound only when `d_min <= d_k <= d_max` on
//! each of them, with every `E_k` finite, nonnegative and not below the true
//! sum of the two largest errors on its axis. Let it evaluate the bound in
//! `f64` as a sum, in any order, of the `T` terms, each term a chain of `K`
//! products that starts with its constant and multiplies its factors in any
//! order: `EPSILON_g` times `d_k` repeated `g_k` times on each axis for a
//! term of epsilon, and `KAPPA_k,h` times `E_k` once and `d_j` repeated
//! `h_j` times on each axis for a term of kappa, with the emitted constants
//! at least `1 + 2^-30` times the exact ones. Then the computed bound, when
//! it is not infinite or NaN, is greater than the error of the computed
//! value of the form, so `|value| > bound` gives the sign of the true value.
//! The axes outside `R` take no part: by (S3) the root depends on no leaf of
//! them, and no term has a factor of them.
//!
//! **Proof.** Write `D_k = d_k / (1 - u)` for `k` in `R`. A sum of
//! nonnegative values rounds down by at most a factor `1 - u` (it is exact
//! when the result is subnormal), so `D_k >= delta_k + E_k`, and the node
//! bounds hold with these `D_k`. On every axis of `R`,
//! `P <= d_k <= D_k <= d_max / (1 - u)`. Let
//! `B = A(D) + sum over k of E_k B_k(D)` be the exact bound with these
//! `D_k`. The four parts below follow the nodes on which the root depends;
//! the others do not change the value of the form.
//!
//! - Lower bound of the nodes. By (S1) and (S3), `C_x(D)` has a monomial of
//!   coefficient at least 1 on axes of `R` only, of total degree
//!   `|g_x| <= K`, and every factor `D_k >= P` with `P < 1`, so
//!   `C_x(D) >= P^(|g_x|) >= P^K`.
//! - Underflow of the form. Model every operation of the computation as
//!   `fl(z) = z (1 + theta) + eta`, where `z` is the exact result on the
//!   computed operands: when `|z| >= 2^-1022`, `|theta| <= u` and
//!   `eta = 0`; for a sum or difference with `|z| < 2^-1022`, the result is
//!   exact (`theta = eta = 0`); for a product with `|z| < 2^-1022`,
//!   `theta = 0` and `|eta| <= 2^-1075`. Let `v_x` be the computed values and `w_x` the
//!   values of the same computation, with the same `theta`, but every `eta`
//!   set to zero. The `w_x` follow the model of the node rules, so
//!   `|w_x| <= C_x(D)` and `|w_root - true| <= B`. Let
//!   `Delta_x = |v_x - w_x|`. A leaf has `Delta = 0`; a negation keeps it; a
//!   sum or difference has `Delta <= (1 + u)(Delta_a + Delta_b)`; and a
//!   product of `a` and `b` has
//!   `Delta <= (1 + u) ((C_a + Delta_a) Delta_b + C_b Delta_a) + 2^-1075`.
//!   With `rho = 2^-1075 / P^K`, the lower bound above gives
//!   `2^-1075 <= rho * C_x(D)` at every product, and induction on the nodes
//!   gives `Delta_x <= tau_x C_x(D)` with `1 + tau_x <= (1 + rho)^(|g_x| - 1)`:
//!   a leaf has `tau = 0`; a sum takes the larger `tau` of its operands,
//!   which have its degree; and a product has
//!   `1 + tau <= (1 + tau_a)(1 + tau_b)(1 + rho)`, whose exponent is
//!   `(|g_a| - 1) + (|g_b| - 1) + 1 = |g_x| - 1`. Condition 3 gives
//!   `rho <= 2^-54`, so with `K <= 1022` the root has
//!   `tau <= (1 + rho)^(K - 1) - 1 <= K rho < 2^-44`, and
//!   `|v_root - w_root| <= K * 2^-1075 * C_root(D) / P^K`. By (S1) and (S2)
//!   `C_root` has the support `V` of `A`; by (S4) its coefficients are at
//!   most `c_root <= c_max`; and every coefficient of `A` is at least
//!   `epsilon_min`. So condition 1 (with `N >= 1`; a form without products
//!   has `tau = 0`) gives, monomial by monomial,
//!   `|v_root - w_root| <= 2^-32 * A(D) <= 2^-32 * B`, and the computed
//!   value of the form errs by at most `(1 + 2^-32) * B`. Nothing in this
//!   argument is of second order: the recursion bounds `Delta` exactly.
//! - Overflow. A leaf is at most `delta_k <= d_k`. The exact result `z` of
//!   the operation of any other node `x` on its computed operands is at most
//!   `(1 + tau_x) C_x(D)`: for a product,
//!   `|z| <= (1 + tau_a)(1 + tau_b) C_a(D) C_b(D)`, and
//!   `C_x = (1 + u) C_a C_b`; a sum is alike. By (S4) the coefficients of
//!   `C_x` add up to the global `c_x`, and every `D_k` that `C_x` involves is
//!   at most `d_max / (1 - u)`, above 1, so
//!   `|z| <= (1 + 2^-44) c_x (d_max / (1 - u))^(|g_x|)`, which by the
//!   definition of `j_max` is at most `(1 + 2^-44) * 2^1022 * (1 - u)^-1022`,
//!   below `2^1023`. No operation overflows, so the model of the previous
//!   part holds at `x`: the two parts are one induction on the nodes, in the
//!   order of the graph. A leaf that overflows, the difference of two
//!   coordinates of opposite signs near `f64::MAX`, gives an infinite
//!   `delta_k`, so `d_k > d_max`, and the filter rejects the call by its
//!   range. An overflow in the evaluation of the bound gives an infinite
//!   bound, or a NaN one when an infinite partial product is then multiplied
//!   by an `E_k` of zero; either only makes the filter abstain, since it
//!   decides on `|value| > bound`, false for both.
//! - Rounding of the bound. Every partial product of a chain obeys
//!   `fl(z) >= z (1 - u) - 2^-1075` (the second term only when `z` is below
//!   `2^-1022`), so a chain `c_0 = CONSTANT`, `c_i = fl(c_(i-1) f_i)` of `K`
//!   factors `f_i >= 0` ends with
//!   `c_K >= (1 - u)^K CONSTANT * f_1 ... f_K - L`, where
//!   `L = sum over i of 2^-1075 * f_(i+1) ... f_K` counts only the products
//!   that underflow. The factors `d_k` are exactly `(1 - u) D_k`, at most `K`
//!   of them per term, so the chain without `L` is at least `(1 - u)^(2K)`
//!   times the exact term with the `D_k` and the emitted constant. The `T`
//!   terms are nonnegative and are summed with `T - 1` additions, each of
//!   which rounds down by at most a factor `1 - u`, in any order, so no term
//!   meets more than `T - 1` of them. The exact bound with the emitted
//!   constants is at least `(1 + 2^-30) B`. With `n = 2K + T - 1 <= 2107`,
//!   `(1 - u)^n >= 1 - 2^-41`, and the computed bound is at least
//!   `(1 + 2^-30)(1 - 2^-41) B - (sum of the L) >= (1 + 2^-31) B - (sum of
//!   the L)`.
//! - Underflow of the bound. A partial product of a term of epsilon is
//!   `EPSILON_g` times at most `K` factors `d_k`, each at least `P` (or above
//!   1), so before rounding it is at least `epsilon_g P^K >= epsilon_min
//!   P^K >= 2^-1021` by condition 2, and after at most `K` roundings at
//!   least `(1 - u)^K * 2^-1021 > 2^-1022`: a term of epsilon never
//!   underflows and has `L = 0`. A partial product of a term of kappa, of
//!   degree vector `g`, may underflow. Its later factors are `d` values and
//!   perhaps `E_k`, each at most its `D_k` (`E_k <= D_k`), so the loss is at
//!   most `2^-1075 * D^g / Q`, where `Q` is the product of the `D_k` of the
//!   factors already multiplied, counting `D_k` for `E_k`: each is at least
//!   `P`, so `Q >= P^K`. Condition 2 gives `1 / P^K <= 2^1021 * epsilon_min`,
//!   and `g` is in `V` by (S2), so `epsilon_min <= epsilon_g` and the loss
//!   is at most `2^-54 * epsilon_g D^g <= 2^-54 * B`. There are at most
//!   `3 * 16` terms of kappa with `K <= 1022` partial products each, fewer
//!   than `2^16` in all, so the losses add up to less than `2^-38 * B`.
//!
//! The computed bound is then at least `(1 + 2^-31 - 2^-38) B`, more than
//! the error `(1 + 2^-32) B` of the computed value.
//!
//! The range is required on every axis of `R` because the proof needs
//! `D_k >= P` on every axis that a node uses: as in the per-axis form, a
//! condition on a product of sizes alone lets a product of two small leaves
//! underflow and its error be multiplied by a large one. On an axis outside
//! `R`, no node on which the root depends has a leaf and no term has a
//! factor, so neither the value nor the bound depends on it.
//!
//! # Certain zero
//!
//! If every vector of `V` has degree at least 1 in every axis
//! ([`PartsBound::certain_zero`]) and some axis `k` has `d_k = 0`, with
//! `E_k` finite and non-negative, then `delta_k = 0` and `E_k = 0` (a sum of
//! two non-negative `f64` is zero only when both are), so every leaf of axis
//! `k`, computed or true, is exactly zero, and the true value of the form is
//! exactly zero. By induction, a node all of whose vectors have `s_k >= 1`
//! has a true value of zero: a leaf of that kind is on axis `k`; a sum has
//! operands of that kind; and if every `s + t` of a product has a positive
//! component `k`, one of the two operands has only vectors with a positive
//! component `k`, otherwise a vector of each with `s_k = t_k = 0` would give
//! `(s + t)_k = 0`.
//!
//! The computed value follows the same induction with one exception: a
//! product of a zero and a factor that is not finite, as when a leaf of
//! another axis is infinite or a product of other axes overflows, is NaN,
//! not zero. So the computed value is zero or NaN, and a NaN leaf, which the
//! largest magnitude can skip, makes it NaN as well. The filter decides zero
//! only under these guards: some `d_k == 0` on an axis of the range, every
//! `E_k` finite and non-negative, every `d_k` finite, and the computed value
//! exactly `0.0`; a NaN value decides nothing.

use std::collections::{BTreeMap, BTreeSet};

use super::{FilterBound, close_above, d_min_exponent, filter_bound, node_coefficient_bits};
use crate::differences::{DiffGraph, DiffNode};
use crate::dyadic::Dyadic;
use crate::error::Error;
use crate::parse::Position;

/// A polynomial in `D_0`, `D_1` and `D_2`: the coefficient of each degree
/// vector that has one, all positive.
type Poly = BTreeMap<[u32; 3], Dyadic>;

/// The bounds of one node, as polynomials in the `D_k`:
/// `|true value| <= M(D)`, `|computed value| <= C(D)` and
/// `|computed - true| <= A(D) + sum of E_k B_k(D)`.
#[derive(Clone, Debug)]
pub(crate) struct PartsNode {
    pub(crate) m: Poly,
    pub(crate) c: Poly,
    pub(crate) a: Poly,
    pub(crate) b: [Poly; 3],
}

/// The limits of the derivation of the bound by parts.
#[derive(Clone, Copy, Debug)]
struct Budget {
    /// The most monomials of any polynomial of any node.
    monomials: usize,
    /// The most degree vectors at the root.
    root_vectors: usize,
    /// The most products of two monomials, with additions counted as one
    /// operation per monomial.
    operations: u64,
    /// The most bits that the exact coefficients of all the nodes may take
    /// together.
    bits: u64,
}

impl Budget {
    /// The limits of [`parts_bound`]. The proof of the range needs at most
    /// 16 degree vectors at the root (so at most 64 terms); 64 monomials per
    /// polynomial and `2^22` operations keep the derivation far below a
    /// second, and the bits are those of the global form.
    const DEFAULT: Budget = Budget {
        monomials: 64,
        root_vectors: 16,
        operations: 1 << 22,
        bits: super::MAX_COEFFICIENT_BITS,
    };
}

/// A term of epsilon of a [`PartsBound`]: `epsilon_g * D^g`.
#[derive(Clone, Debug)]
pub struct EpsilonTerm {
    /// The degree vector `g`: the degree of the term in the differences of
    /// each axis.
    pub degrees: [u32; 3],
    /// `epsilon_g / u`, with `u = 2^-53`, as an exact fraction.
    pub coefficient_in_u: String,
    exact: Dyadic,
}

/// A term of kappa of a [`PartsBound`]: `kappa_k,h * E_k * D^h`.
#[derive(Clone, Debug)]
pub struct KappaTerm {
    /// The axis `k` of the uncertainty `E_k`: 0, 1 and 2 for x, y and z.
    pub axis: usize,
    /// The degree vector `h` of the rest of the term, `g - e_k` for the
    /// degree vector `g` of the term.
    pub rest: [u32; 3],
    /// `kappa_k,h` as an exact fraction.
    pub coefficient: String,
    exact: Dyadic,
}

/// The exact error bound by parts of the `f64` evaluation of a difference
/// form.
///
/// For a call, with `D_k = delta_k + E_k` on each axis, as for an
/// [`AxisBound`](super::AxisBound), the `f64` value of the form differs
/// from the exact value at the exact points by at most the sum of the terms
/// of [`PartsBound::epsilon`] and [`PartsBound::kappa`], while on every axis
/// of [`PartsBound::range_axes`] the size computed in `f64`,
/// `d_k = delta_k + E_k`, is in the range of [`PartsBound::constants`]. The
/// documentation of this module, in the source, gives the rules and the
/// proof of the range, and says how the bound must be evaluated.
#[derive(Clone, Debug)]
pub struct PartsBound {
    /// The total degree `K` of the form in the differences.
    pub degree: u32,
    /// The terms of epsilon, one per degree vector of the form, in
    /// increasing order of the vector.
    pub epsilon: Vec<EpsilonTerm>,
    /// The terms of kappa, in increasing order of the axis and then of the
    /// rest.
    pub kappa: Vec<KappaTerm>,
    d_min_exponent: i64,
    d_max_exponent: i64,
}

/// The outcome of [`parts_bound`].
#[derive(Clone, Debug)]
pub enum PartsOutcome {
    /// The bound by parts.
    Bound(PartsBound),
    /// The bound is not derived, for the reason given, in English, as a
    /// sentence ready for a warning.
    Unavailable(String),
}

/// The constant of a term of epsilon, as `f64`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EpsilonConstant {
    /// The degree vector `g` of the term.
    pub degrees: [u32; 3],
    /// The coefficient of `D^g`, times `1 + 2^-30`, rounded up.
    pub value: f64,
}

/// The constant of a term of kappa, as `f64`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KappaConstant {
    /// The axis `k` of the factor `E_k`.
    pub axis: usize,
    /// The degree vector `h` of the factors `D`, `g - e_k`.
    pub rest: [u32; 3],
    /// The coefficient of `E_k * D^h`, times `1 + 2^-30`, rounded up.
    pub value: f64,
}

/// The constants of the filter by parts of a predicate, as `f64`.
///
/// The filter decides when `d_min <= d_k <= d_max` on every axis `k` of
/// `range_axes`, where `d_k = delta_k + E_k` is computed in `f64`, and
/// `|value|` exceeds the sum, in any order, of the terms, each evaluated as
/// a chain of products that starts with its constant: `value * d^degrees`
/// for a term of epsilon and `value * E_axis * d^rest` for a term of kappa.
#[derive(Clone, Debug, PartialEq)]
pub struct PartsConstants {
    /// The total degree `K` of the form.
    pub degree: u32,
    /// The constants of the terms of epsilon.
    pub epsilon: Vec<EpsilonConstant>,
    /// The constants of the terms of kappa.
    pub kappa: Vec<KappaConstant>,
    /// The axes on which the range is required: those of positive degree in
    /// some degree vector of the form.
    pub range_axes: [bool; 3],
    /// The smallest `d_k` on each axis of `range_axes` for which the bound
    /// covers underflow: a power of two below 1.
    pub d_min: f64,
    /// The largest `d_k` on each axis of `range_axes` for which no node
    /// overflows: a power of two above 1, the `d_max` of the global form.
    pub d_max: f64,
}

/// Derives the error bound by parts of the `f64` evaluation of `graph`.
///
/// The rules are those of [`filter_bound`] with polynomials in the sizes
/// `D_k` of the three axes in place of the coefficients; the documentation
/// of [`PartsBound`] says what the bound states. Any graph is accepted,
/// including a multihomogeneous one, for which the bound is the per-axis
/// one of [`filter_form`](super::filter_form).
///
/// Returns [`PartsOutcome::Unavailable`] when a limit of the derivation is
/// reached: more than 64 monomials in a polynomial of a node, more than 16
/// degree vectors at the root, more than `2^22` operations on exact
/// coefficients, or more than `2^26` bits of exact coefficients in all,
/// each checked before the work it limits; when a leaf is on an axis above
/// 2, or a degree vector of a node does not have the degree of the node,
/// which no graph of [`difference_graph`](crate::difference_graph) has; and
/// when no range of sizes keeps the bound clear of underflow.
///
/// # Errors
///
/// The errors of [`filter_bound`], which runs first: the bound by parts
/// uses its range.
pub fn parts_bound(
    graph: &DiffGraph,
    file: &str,
    position: Position,
) -> Result<PartsOutcome, Error> {
    let global = filter_bound(graph, file, position)?;
    Ok(derive(graph, &global, &Budget::DEFAULT))
}

/// The start of every reason of [`PartsOutcome::Unavailable`].
const NOT_DERIVED: &str = "the error bound by parts of the filter is not derived";

/// Checks the budget on the supports alone, before any coefficient is
/// computed.
fn check_budget(graph: &DiffGraph, budget: &Budget) -> Result<(), String> {
    let bits = node_coefficient_bits(graph);
    let mut supports: Vec<BTreeSet<[u32; 3]>> = Vec::with_capacity(graph.nodes.len());
    let mut operations: u64 = 0;
    let mut total_bits: u64 = 0;
    let too_many_operations = || {
        format!(
            "{NOT_DERIVED}: it would take more than {} operations on exact coefficients",
            budget.operations
        )
    };
    for (id, node) in graph.nodes.iter().enumerate() {
        let support: BTreeSet<[u32; 3]> = match *node {
            DiffNode::Leaf { axis, .. } => {
                if axis > 2 {
                    return Err(format!("{NOT_DERIVED}: a leaf is on the axis {axis}"));
                }
                operations = operations.saturating_add(7);
                let mut g = [0; 3];
                g[usize::from(axis)] = 1;
                BTreeSet::from([g])
            }
            DiffNode::Neg(x) => {
                operations = operations.saturating_add(7 * supports[x].len() as u64);
                supports[x].clone()
            }
            DiffNode::Add(x, y) | DiffNode::Sub(x, y) => {
                let size = (supports[x].len() + supports[y].len()) as u64;
                operations = operations.saturating_add(7 * size);
                supports[x].union(&supports[y]).copied().collect()
            }
            DiffNode::Mul(x, y) => {
                // Four products of polynomials for `M`, `C` and `A`, and two
                // per axis for the `B_k`, plus the support itself.
                let size = supports[x].len() as u64 * supports[y].len() as u64;
                operations = operations.saturating_add(11 * size);
                if operations > budget.operations {
                    return Err(too_many_operations());
                }
                let mut support = BTreeSet::new();
                for s in &supports[x] {
                    for t in &supports[y] {
                        support.insert([0, 1, 2].map(|k| s[k].saturating_add(t[k])));
                    }
                }
                support
            }
        };
        if operations > budget.operations {
            return Err(too_many_operations());
        }
        // (S2), on which the proof relies: every vector of the support has
        // the degree of the node. `difference_graph` builds only such graphs;
        // a graph built otherwise has no bound, like a leaf on an axis
        // above 2.
        let degree = u64::from(graph.degrees[id]);
        if let Some(g) = support
            .iter()
            .find(|g| g.iter().map(|&x| u64::from(x)).sum::<u64>() != degree)
        {
            return Err(format!(
                "{NOT_DERIVED}: a node of degree {degree} has the degree vector {g:?}"
            ));
        }
        if support.len() > budget.monomials {
            return Err(format!(
                "{NOT_DERIVED}: a polynomial of its node bounds has more than {} terms",
                budget.monomials
            ));
        }
        // `M`, `C`, `A` and the three `B_k` have at most `|S|` monomials
        // each, and each coefficient at most the bits of the global estimate.
        total_bits = total_bits.saturating_add(bits[id].saturating_mul(6 * support.len() as u64));
        if total_bits > budget.bits {
            return Err(format!(
                "{NOT_DERIVED}: its exact coefficients would take more than {} bits in all",
                budget.bits
            ));
        }
        supports.push(support);
    }
    let vectors = supports[graph.root].len();
    if vectors > budget.root_vectors {
        return Err(format!(
            "{NOT_DERIVED}: the form has {vectors} degree vectors, more than {}",
            budget.root_vectors
        ));
    }
    Ok(())
}

fn add(x: &Poly, y: &Poly) -> Poly {
    let mut sum = x.clone();
    for (g, v) in y {
        sum.entry(*g)
            .and_modify(|w| *w = &*w + v)
            .or_insert_with(|| v.clone());
    }
    sum
}

fn scale(x: &Poly, s: &Dyadic) -> Poly {
    x.iter().map(|(g, v)| (*g, v * s)).collect()
}

fn mul(x: &Poly, y: &Poly) -> Poly {
    let mut product = Poly::new();
    for (gx, vx) in x {
        for (gy, vy) in y {
            let g = [0, 1, 2].map(|k| gx[k].saturating_add(gy[k]));
            let v = vx * vy;
            product.entry(g).and_modify(|w| *w = &*w + &v).or_insert(v);
        }
    }
    product
}

/// Applies the rules to every node of `graph`, whose leaves are all on the
/// axes 0 to 2.
fn rules(graph: &DiffGraph) -> Vec<PartsNode> {
    let u = Dyadic::pow2(-53);
    let one = Dyadic::integer(1);
    let one_plus_u = &one + &u;
    let mut nodes: Vec<PartsNode> = Vec::with_capacity(graph.nodes.len());
    for node in &graph.nodes {
        let bound = match *node {
            DiffNode::Leaf { axis, .. } => {
                let k = usize::from(axis);
                let mut g = [0; 3];
                g[k] = 1;
                let mut b: [Poly; 3] = Default::default();
                b[k].insert([0; 3], one.clone());
                PartsNode {
                    m: Poly::from([(g, one_plus_u.clone())]),
                    c: Poly::from([(g, one.clone())]),
                    a: Poly::from([(g, u.clone())]),
                    b,
                }
            }
            DiffNode::Neg(x) => nodes[x].clone(),
            DiffNode::Add(x, y) | DiffNode::Sub(x, y) => {
                let (x, y) = (&nodes[x], &nodes[y]);
                let c_sum = add(&x.c, &y.c);
                PartsNode {
                    m: add(&x.m, &y.m),
                    c: scale(&c_sum, &one_plus_u),
                    a: add(&add(&x.a, &y.a), &scale(&c_sum, &u)),
                    b: [0, 1, 2].map(|k| add(&x.b[k], &y.b[k])),
                }
            }
            DiffNode::Mul(x, y) => {
                let (x, y) = (&nodes[x], &nodes[y]);
                let c_product = mul(&x.c, &y.c);
                PartsNode {
                    m: mul(&x.m, &y.m),
                    c: scale(&c_product, &one_plus_u),
                    a: add(
                        &add(&mul(&x.c, &y.a), &mul(&y.m, &x.a)),
                        &scale(&c_product, &u),
                    ),
                    b: [0, 1, 2].map(|k| add(&mul(&x.c, &y.b[k]), &mul(&y.m, &x.b[k]))),
                }
            }
        };
        nodes.push(bound);
    }
    nodes
}

/// The exponent of `d_min`: the smallest `j <= -1` that satisfies the three
/// conditions of the documentation of this module, if any.
fn parts_d_min_exponent(
    epsilon_min: &Dyadic,
    c_max: &Dyadic,
    degree: u32,
    products: u32,
) -> Option<i64> {
    // Conditions 1 and 2, as in the global form with `epsilon_min`.
    let j = d_min_exponent(epsilon_min, c_max, degree, products)?;
    // Condition 3: `j K >= -1021`, so `j >= -floor(1021 / K)`.
    let j_normal = -(1021 / i64::from(degree.max(1)));
    (j_normal <= -1).then_some(j.max(j_normal))
}

/// Derives the bound by parts of `graph`, whose global form is `global`,
/// within `budget`.
fn derive(graph: &DiffGraph, global: &FilterBound, budget: &Budget) -> PartsOutcome {
    if let Err(reason) = check_budget(graph, budget) {
        return PartsOutcome::Unavailable(reason);
    }
    let nodes = rules(graph);
    let root = &nodes[graph.root];
    let epsilon: Vec<EpsilonTerm> = root
        .a
        .iter()
        .map(|(g, v)| EpsilonTerm {
            degrees: *g,
            coefficient_in_u: (v * &Dyadic::pow2(53)).fraction(),
            exact: v.clone(),
        })
        .collect();
    let kappa: Vec<KappaTerm> = (0..3)
        .flat_map(|k| {
            root.b[k].iter().map(move |(h, v)| KappaTerm {
                axis: k,
                rest: *h,
                coefficient: v.fraction(),
                exact: v.clone(),
            })
        })
        .collect();
    let epsilon_min = epsilon
        .iter()
        .map(|t| &t.exact)
        .min()
        .expect("a node has at least one monomial");
    let c_max = global
        .nodes
        .iter()
        .map(|n| &n.c)
        .max()
        .expect("a graph has a root");
    let Some(d_min_exponent) =
        parts_d_min_exponent(epsilon_min, c_max, global.degree, global.products)
    else {
        return PartsOutcome::Unavailable(format!(
            "{NOT_DERIVED}: no size of the inputs below 1 keeps it clear of underflow, \
             because its smallest coefficient of epsilon is too small for the degree {}",
            global.degree
        ));
    };
    PartsOutcome::Bound(PartsBound {
        degree: global.degree,
        epsilon,
        kappa,
        d_min_exponent,
        d_max_exponent: global.d_max_exponent,
    })
}

impl PartsBound {
    /// Whether the term of epsilon of degree vector `degrees` exists and
    /// `n u <= epsilon_g < n u (1 + 2^-40)`, exactly.
    pub fn epsilon_is_u_times(&self, degrees: [u32; 3], n: u64) -> bool {
        let target = &Dyadic::integer(n) * &Dyadic::pow2(-53);
        self.epsilon
            .iter()
            .find(|t| t.degrees == degrees)
            .is_some_and(|t| close_above(&t.exact, &target))
    }

    /// Whether the term of kappa of axis `axis` and rest `rest` exists and
    /// `n <= kappa_k,h < n (1 + 2^-40)`, exactly.
    pub fn kappa_is(&self, axis: usize, rest: [u32; 3], n: u64) -> bool {
        self.kappa
            .iter()
            .find(|t| t.axis == axis && t.rest == rest)
            .is_some_and(|t| close_above(&t.exact, &Dyadic::integer(n)))
    }

    /// Whether every degree vector of the form has degree at least 1 in
    /// every axis, so that the true value of the form is exactly zero when
    /// on some axis every leaf is exactly zero and the uncertainty is zero.
    /// The computed value is then zero or NaN (a zero times a factor that is
    /// not finite), so a filter decides zero only when some `d_k` is zero,
    /// every `E_k` is finite and non-negative, every `d_k` is finite and the
    /// computed value is exactly `0.0` (see the documentation of this
    /// module, in the source).
    pub fn certain_zero(&self) -> bool {
        self.epsilon
            .iter()
            .all(|t| t.degrees.iter().all(|&g| g >= 1))
    }

    /// The axes on which the range of [`PartsBound::constants`] is required:
    /// those of positive degree in some degree vector of the form. The value
    /// of the form and the bound depend on no other axis.
    pub fn range_axes(&self) -> [bool; 3] {
        [0, 1, 2].map(|k| self.epsilon.iter().any(|t| t.degrees[k] >= 1))
    }

    /// The constants of the filter.
    ///
    /// Every coefficient is the exact value times `1 + 2^-30`, rounded up.
    /// `d_min` and `d_max` are the powers of two of the range of validity,
    /// derived in the documentation of this module (in the source); `d_max`
    /// is that of the global form of the same graph.
    pub fn constants(&self) -> PartsConstants {
        let inflate = &Dyadic::integer(1) + &Dyadic::pow2(-30);
        PartsConstants {
            degree: self.degree,
            epsilon: self
                .epsilon
                .iter()
                .map(|t| EpsilonConstant {
                    degrees: t.degrees,
                    value: (&t.exact * &inflate).round_up(),
                })
                .collect(),
            kappa: self
                .kappa
                .iter()
                .map(|t| KappaConstant {
                    axis: t.axis,
                    rest: t.rest,
                    value: (&t.exact * &inflate).round_up(),
                })
                .collect(),
            range_axes: self.range_axes(),
            d_min: Dyadic::pow2(self.d_min_exponent).round_up(),
            d_max: Dyadic::pow2(self.d_max_exponent).round_up(),
        }
    }
}

#[cfg(test)]
#[path = "parts_tests.rs"]
mod tests;
