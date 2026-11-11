//! The error bound of the approximate coordinates of a point type: the
//! program that computes it and its slack factor, derived and justified in
//! the documentation of [`error_bound`].

use crate::dyadic::Dyadic;
use crate::error::Error;

use super::{PointNode, PointType};

/// The largest rounding count [`error_bound`] accepts: it keeps the exact
/// derivation of the slack factor small.
pub const MAX_ROUNDS: u32 = 1024;

/// The error bound of the coordinates of a point type, as derived by
/// [`error_bound`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ErrorBound {
    /// The number of roundings of the computation of the error of each
    /// coordinate, on its longest chain of operations.
    pub rounds: [u32; 3],
    /// The slack factor `S`: the smallest `f64` with `S·(1 - 2^-53)^H >= 1`,
    /// where `H` is the largest of [`rounds`](Self::rounds).
    pub slack: f64,
}

/// A term of the error of a node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Term {
    /// The error of a node, as computed.
    Error(usize),
    /// The covered product of `|value|` and the error of `error`.
    ValueTimesError {
        /// The node whose value is taken.
        value: usize,
        /// The node whose error is taken.
        error: usize,
    },
    /// The covered product of the errors of two nodes.
    ErrorTimesError(usize, usize),
    /// The rounding of a sum or difference of the nodes `x` and `y`.
    SumRounding(usize, usize),
    /// The rounding of a product of the nodes `x` and `y`.
    ProductRounding(usize, usize),
}

/// The program that computes the error of every node of a point type: the
/// terms added, in order, and the rounding count of each node. A node with
/// no terms has an error of zero for every input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ErrorProgram {
    pub(crate) terms: Vec<Vec<Term>>,
    pub(crate) rounds: Vec<u32>,
}

impl ErrorProgram {
    /// Builds the program of `point`, whose integers must be representable.
    pub(crate) fn of(point: &PointType) -> ErrorProgram {
        let nodes = &point.graph.nodes;
        let is_zero = |id: usize| nodes[id] == PointNode::Int(0);
        let mut terms: Vec<Vec<Term>> = Vec::with_capacity(nodes.len());
        let mut rounds: Vec<u32> = Vec::with_capacity(nodes.len());
        for node in nodes {
            let has_error = |id: usize, terms: &[Vec<Term>]| !terms[id].is_empty();
            let mut node_terms = Vec::new();
            match *node {
                PointNode::Coord { .. } | PointNode::Scalar(_) | PointNode::Int(_) => {}
                PointNode::Neg(x) => {
                    if has_error(x, &terms) {
                        node_terms.push(Term::Error(x));
                    }
                }
                PointNode::Add(x, y) | PointNode::Sub(x, y) => {
                    for operand in [x, y] {
                        if has_error(operand, &terms) {
                            node_terms.push(Term::Error(operand));
                        }
                    }
                    if !is_zero(x) && !is_zero(y) {
                        node_terms.push(Term::SumRounding(x, y));
                    }
                }
                PointNode::Mul(x, y) => {
                    let (ex, ey) = (has_error(x, &terms), has_error(y, &terms));
                    if ey {
                        node_terms.push(Term::ValueTimesError { value: x, error: y });
                    }
                    if ex {
                        node_terms.push(Term::ValueTimesError { value: y, error: x });
                    }
                    if ex && ey {
                        node_terms.push(Term::ErrorTimesError(x, y));
                    }
                    if !is_zero(x) && !is_zero(y) {
                        node_terms.push(Term::ProductRounding(x, y));
                    }
                }
            }
            let count = node_terms
                .iter()
                .map(|&term| term_rounds(term, &rounds))
                .reduce(|total, term| total.max(term).saturating_add(1))
                .unwrap_or(0);
            terms.push(node_terms);
            rounds.push(count);
        }
        ErrorProgram { terms, rounds }
    }
}

/// The rounding count of `term`, given the counts of the earlier nodes.
fn term_rounds(term: Term, rounds: &[u32]) -> u32 {
    match term {
        Term::Error(x) => rounds[x],
        Term::ValueTimesError { error, .. } => rounds[error].saturating_add(2),
        Term::ErrorTimesError(x, y) => rounds[x].saturating_add(rounds[y]).saturating_add(2),
        Term::SumRounding(..) => 2,
        Term::ProductRounding(..) => 3,
    }
}

/// Derives the error bound of the approximate coordinates of `point`.
///
/// The constructor of a point type evaluates its coordinates in `f64`, one
/// node of its [`PointGraph`](super::PointGraph) at a time, and with each
/// value `v` an error `e`, a bound on `|v - V|` where `V` is the exact value
/// of the node. This function derives the program that computes `e` and the
/// factor that makes the result an upper bound in spite of the rounding of
/// that program itself.
///
/// # The rule
///
/// With `u = 2^-53`, the rule gives each node a real bound `R`:
///
/// - an input (a coordinate, an `f64` parameter or an integer that `f64`
///   represents exactly) has `R = 0`;
/// - a negation keeps the bound of its operand;
/// - a sum or difference `s = x ± y` has `R_s = R_x + R_y`, plus `u·|s|`
///   unless an operand is exactly zero;
/// - a product `p = x·y` has `R_p = |x|·R_y + |y|·R_x + R_x·R_y`, plus
///   `u·|p| + 2^-1074` unless an operand is exactly zero.
///
/// Here `x`, `y`, `s` and `p` are the computed `f64` values. `R` bounds the
/// error: rounding to nearest makes `|fl(x ± y) - (x ± y)|` at most
/// `u·|fl(x ± y)|` (a sum whose result is below `2^-1021` is exact), and
/// `|fl(x·y) - x·y|` at most `u·|fl(x·y)|` or, when `x·y` is below `2^-1022`,
/// `2^-1075`; an operation with an operand that is exactly zero is exact.
/// The errors of the operands then propagate as written, since
/// `|x·y - X·Y| <= |x|·|y - Y| + (|y| + |y - Y|)·|x - X|`. The condition
/// "exactly zero" is on the operands, never on the result: a product that
/// underflows gives zero without being exact.
///
/// # The program
///
/// The program computes, for each node, the terms of its rule in the order
/// above and adds them from left to right. Terms that are zero for every
/// input (the error of an input) are left out; the others are computed even
/// when they are zero at run time, which costs nothing in the bound because
/// an operation with a zero operand is exact. Every product of the program
/// is a *covered product*: `fl(fl(a·b) + 2^-1074)`, or `0` when `a` or `b`
/// is zero. The rounding terms are the covered product of `u` and `|s|` for
/// a sum, and the covered product of `u` and `|p|` plus `2^-1074` for a
/// product, both `0` when an operand of the node is zero.
///
/// # The rounding of the program
///
/// All the values of the program are non-negative. For non-negative `a` and
/// `b`, `fl(a + b) >= (1 - u)·(a + b)`: above `2^-1022` by the bound on the
/// relative error, and below it because the sum is exact. A covered product
/// satisfies `c >= (1 - u)^2·a·b` whatever the magnitudes, which is why the
/// program adds `2^-1074` after each product: if `fl(a·b) >= 2^-1022`,
/// `fl(a·b) >= (1 - u)·a·b` and the addition loses at most another factor
/// `1 - u`; if `fl(a·b) < 2^-1022`, possibly zero after underflow, then
/// `fl(a·b) >= a·b - 2^-1075`, and adding `2^-1074` to a value below
/// `2^-1022` is exact, so `c >= a·b`. That covers the underflow of the
/// program itself, including a product rounded to zero, and the term is
/// added only when both operands are nonzero, so a coordinate whose terms
/// are all zero still gets an error of exactly zero.
///
/// Each node is given a count `h` of roundings: one per addition and per
/// multiplication of the program on the longest chain that leads to its
/// error, so a covered product counts two. An error `e(x)` with `h(x)` such
/// roundings satisfies `e(x) >= (1 - u)^h(x)·R(x)`, by induction over the
/// operations: an input has `h = 0`; the error of an operand has its own
/// count; a covered product of a value and an error `e(y)` has `h(y) + 2`,
/// and of two errors `h(x) + h(y) + 2`; the rounding term of a sum has 2 and
/// that of a product 3; and adding a term to the sum of the previous ones
/// gives one more than the larger of the two counts.
///
/// # The factor and the final rounding
///
/// With `H` the largest count of the three coordinates, the slack factor `S`
/// is the smallest `f64` with `S·(1 - u)^H >= 1`, derived here in exact
/// arithmetic. The constructor then stores `next_up(fl(e·S))`, which is at
/// least `e·S >= R`, since the value after the nearest `f64` is above the
/// exact product; and `0` when `e` is `0`, which happens only when every
/// term is zero, so only when `R` is zero and the coordinate is exact. If
/// the coordinate or `fl(e·S)` is not finite, it stores `+∞` instead.
///
/// # Errors
///
/// An error at the name of the type if an integer of its expression is not
/// exactly an `f64`, or if the computation of the error of a coordinate
/// takes more than [`MAX_ROUNDS`] roundings.
pub fn error_bound(point: &PointType, file: &str) -> Result<ErrorBound, Error> {
    check_integers(point, file)?;
    let program = ErrorProgram::of(point);
    let rounds = point.graph.roots.map(|root| program.rounds[root]);
    let most = rounds.into_iter().max().unwrap_or(0);
    if most > MAX_ROUNDS {
        return Err(Error::at(
            file,
            point.position,
            format!(
                "the error bound of `{}` takes {most} roundings, more than the {MAX_ROUNDS} \
                 for which its slack factor is derived",
                point.name
            ),
        ));
    }
    Ok(ErrorBound {
        rounds,
        slack: slack_factor(most),
    })
}

/// Checks that every integer of the expression of `point` is exactly an
/// `f64`, so that it is an input without error.
fn check_integers(point: &PointType, file: &str) -> Result<(), Error> {
    for node in &point.graph.nodes {
        if let PointNode::Int(n) = *node {
            if !is_exact_f64(n) {
                return Err(Error::at(
                    file,
                    point.position,
                    format!(
                        "the integer {n} in `{}` is not exactly an `f64`: integers of a point \
                         must have a magnitude of at most 2^53 or be exactly representable",
                        point.name
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// Whether `n` converts to `f64` without rounding.
pub(crate) fn is_exact_f64(n: i64) -> bool {
    // `as` rounds to nearest, and the conversion back is exact in i128.
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    let back = (n as f64) as i128;
    back == i128::from(n)
}

/// The smallest `f64` `S` with `S·(1 - 2^-53)^rounds >= 1`.
pub(crate) fn slack_factor(rounds: u32) -> f64 {
    let one_minus_u = &Dyadic::integer(1) - &Dyadic::pow2(-53);
    let mut power = Dyadic::integer(1);
    for _ in 0..rounds {
        power = &power * &one_minus_u;
    }
    let one = Dyadic::integer(1);
    let mut slack = 1.0f64;
    while &Dyadic::from_f64(slack) * &power < one {
        slack = slack.next_up();
    }
    slack
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_slack_factor_covers_its_rounds_and_no_smaller_factor_does() {
        assert_eq!(slack_factor(0), 1.0);
        for rounds in [1, 2, 3, 6, 100, MAX_ROUNDS] {
            let s = slack_factor(rounds);
            let one_minus_u = &Dyadic::integer(1) - &Dyadic::pow2(-53);
            let mut power = Dyadic::integer(1);
            for _ in 0..rounds {
                power = &power * &one_minus_u;
            }
            assert!(&Dyadic::from_f64(s) * &power >= Dyadic::integer(1));
            assert!(&Dyadic::from_f64(s.next_down()) * &power < Dyadic::integer(1));
        }
    }

    #[test]
    fn integers_beyond_2_to_the_53_are_exact_only_when_representable() {
        assert!(is_exact_f64(1 << 53));
        assert!(!is_exact_f64((1 << 53) + 1));
        assert!(is_exact_f64(i64::MIN));
        assert!(!is_exact_f64(i64::MAX));
    }
}
