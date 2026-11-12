//! The range of the exact stages over arena spans.
//!
//! An exact stage over spans reads each coordinate of each point as an
//! expansion of up to `L` components in a `Scratch` arena and evaluates a
//! form with the operations of the arena: a leaf of the translated form is
//! `diff` of two coordinates, a leaf of the untranslated form is the
//! coordinate itself, a constant is `push` of one component (none for
//! zero), a sum or difference is `sum` or `diff`, a negation is `diff` from
//! the empty expansion, and a product is `product`, the sum of the scalings
//! of the longer operand by each component of the shorter one. This module
//! derives the magnitudes `[m, M]` of the components of the coordinates for
//! which every exact product of that evaluation has both operands in the
//! product domain and no value overflows.
//!
//! [`exact_plan_spans_translated`] gives the rule and why it is sound.
//!
//! The range does not depend on the type of the points: it is a condition
//! on the components of their exact coordinates, whatever computed them.

use crate::differences::DiffGraph;
use crate::dyadic::Dyadic;
use crate::error::Error;
use crate::graph::Graph;
use crate::parse::Position;

use super::shape::{MAX_LENGTH, Op, difference_ops, raw_ops};
use super::{ExactPlan, ExactRange, OVERFLOW_EXPONENT, domain, first_failure};

/// The most components of a coordinate that the exact stages over spans
/// accept, which covers every point type of the library.
pub const MAX_COORD_COMPONENTS: usize = 16;

/// The upper limit on the number of error-free transformations behind one
/// node, `2^50`: with `u = 2^-53` it keeps `6 u T <= 3/4`, so the component
/// bound `B = (1 + 6 u T) S` is below `2 S`, which the cap on the lengths
/// needs.
const MAX_OPERATIONS: u128 = 1 << 50;

/// Derives the exact stage over spans of a difference form, each of whose
/// leaves is the difference of two coordinates of up to `components`
/// components, for the product domain `[operand_min, operand_max]`.
///
/// The range bounds the nonzero components of the coordinates, so it does
/// not depend on the type of the points. The evaluation is that of the
/// generated stage: a leaf is `diff` of two coordinates in the arena, a sum
/// or difference `sum` or `diff`, a negation `diff` from the empty
/// expansion, a product `product`, the sum of the scalings of the longer
/// operand by each component of the shorter one. The lengths of the plan
/// are the worst-case lengths of the expansion of each node for inputs in
/// the range.
///
/// # Why the range is sufficient
///
/// The argument is that of [`ExactRange`], with leaves that are
/// expansions; write `u = 2^-53`. Every coordinate is a valid expansion, as
/// every operation of the arena requires of its inputs, of at most `L`
/// components, each zero or of magnitude in `[m, M]`, with `m` and `M`
/// powers of two in the product domain.
///
/// **Grid.** A component of magnitude at least `m` is a multiple of
/// `q = m 2^-52`, so every component of every coordinate is. The grid
/// argument of [`ExactRange`] then holds unchanged: the rounded value
/// and the error of each error-free transformation of multiples of `g` are
/// multiples of `g`, so every component of every expansion the evaluation
/// handles, however small it is after cancellation or as the error of a
/// sum or a product, is a multiple of the grid of its node: `q` for a leaf,
/// the smaller grid of the operands for a sum, their product for a product.
/// A nonzero component is at least its grid. The rule asks the grid of
/// every product operand to be at least the bottom of the product domain,
/// so no component of an operand of `product` is nonzero and below it.
///
/// **Magnitude.** The bound `S` on the sum of the magnitudes of the
/// components of a node follows the structure as before, starting from
/// `L M` for a coordinate and `2 L M` for the difference of two, which
/// holds whatever the components are. Every value handled on the way to a
/// node, the running sums inside `product` included, is at most
/// `(1 + 3 u)^T S`, where `T <= 2^50` counts, with multiplicity, the
/// error-free transformations the node depends on. Since `3 u T <= 3/8`,
/// `(1 + 3 u)^T <= e^(3 u T) <= 1 + 6 u T` (as `e^x <= 1 + 2 x` for
/// `0 <= x <= 1`), so every such value is at most `B = (1 + 6 u T) S`, and
/// `6 u T <= 6 * 2^-53 * 2^50 = 3/4` gives `B <= (7/4) S < 2 S`.
///
/// **Lengths.** The counts `T` depend on the lengths of the expansions, and
/// the lengths of the stack model, `2 l_x l_y` for a product, grow too fast
/// with leaves of 16 components. Every result of the arena is a valid
/// expansion: its components are nonzero, of strictly increasing magnitude
/// and nonoverlapping, so each occupies bit positions that no other one
/// does. They are multiples of `2^g`, `g` the grid of the node, and at
/// most `B < 2 S <= 2^(c + 1)` in magnitude, with `c` the smallest integer
/// such that `S <= 2^c`; so their bits lie in positions `g..=c`, and there
/// are at most `c - g + 1` of them. The length `l` of a
/// node is the smaller of that and of its structural length: `L` or `2 L`
/// for a leaf, 1 for a nonzero constant, `l_x` for a negation, `l_x + l_y`
/// for a sum or difference and `2 l_x l_y` for a product. The cap holds for
/// the running sums of a product too, whose values are partial sums of the
/// product, below its `B` and on its grid.
///
/// **Counts.** With those lengths: a translated leaf merges two coordinates,
/// at most `4 L` transformations; a sum or difference at most
/// `2 (l_x + l_y)`, a negation `2 l_x`; a product with `s` the smaller and
/// `t` the larger length of its operands scales the longer operand `s`
/// times, at most `1 + 3 (t - 1) <= 3 t` transformations each, and merges each scaling, of at
/// most `2 t` components, into a running sum of at most `l` components, at
/// most `2 (l + 2 t)` each: `s (7 t + 2 l)` in all. A count is computed from
/// the lengths, and the cap on the length of a node relies on the count of
/// that same node being at most `2^50`. That is not circular: the rule
/// rejects a count above `2^50`, and by induction over the transformations
/// of the node, as long as the count so far is within the computed one every
/// value is below `B < 2 S`, so every expansion held is within the cap, so
/// the next merge stays within the count.
///
/// **Rule.** The range is the widest pair of powers of two within the
/// product domain such that every product operand has a grid of at least
/// its bottom and a `B` of at most its top, every node a `T` of at most
/// `2^50` and a `B` below `2^1020`. The grid condition depends only on `m`;
/// the others depend on `M` and, through the caps, on `m`, and grow with
/// `M` and shrink with `m`. So `m` is the smallest value that passes the
/// grid condition, and `M` is found for that `m`, which gives the longest
/// caps: inputs whose nonzero components are all larger only have shorter
/// expansions.
///
/// # Errors
///
/// Returns an error, at `position` in `file`, when no range satisfies the
/// rule, or when the temporaries would need `2^32` components or more in
/// all.
///
/// # Panics
///
/// Panics if `components` is zero, or if `operand_min` and `operand_max`
/// are not normal powers of two with `operand_min <= operand_max`.
pub fn exact_plan_spans_translated(
    graph: &DiffGraph,
    components: usize,
    file: &str,
    position: Position,
    operand_min: f64,
    operand_max: f64,
) -> Result<ExactPlan, Error> {
    let arena = Arena::new(difference_ops(graph), components, true);
    let (low, high) = domain(operand_min, operand_max);
    arena
        .plan(low, high)
        .map_err(|message| Error::at(file, position, message))
}

/// Derives the exact stage over spans of a raw form, whose leaves are
/// coordinates of up to `components` components, for the product domain
/// `[operand_min, operand_max]`.
///
/// The rule and its proof are those of [`exact_plan_spans_translated`],
/// with the coordinates themselves as leaves, a constant written with
/// `push` of one component (of none for zero) and a negation `0 - x` as
/// `diff` from that empty expansion.
///
/// # Errors
///
/// As [`exact_plan_spans_translated`], and when a constant is not exact in
/// `f64`.
///
/// # Panics
///
/// As [`exact_plan_spans_translated`].
pub fn exact_plan_spans_untranslated(
    graph: &Graph,
    components: usize,
    file: &str,
    position: Position,
    operand_min: f64,
    operand_max: f64,
) -> Result<ExactPlan, Error> {
    let (low, high) = domain(operand_min, operand_max);
    raw_ops(graph)
        .and_then(|ops| Arena::new(ops, components, false).plan(low, high))
        .map_err(|message| Error::at(file, position, message))
}

/// An evaluation over spans.
struct Arena {
    ops: Vec<Op>,
    /// `L`, the most components of a coordinate.
    components: u128,
    /// Whether a leaf is a difference of two coordinates.
    translated: bool,
}

impl Arena {
    fn new(ops: Vec<Op>, components: usize, translated: bool) -> Arena {
        assert!(components > 0, "a coordinate has at least one component");
        let components = u128::try_from(components).expect("a usize fits in u128");
        Arena {
            ops,
            components,
            translated,
        }
    }

    /// The plan for the product domain `[2^low, 2^high]`, or the message to
    /// report.
    fn plan(&self, low: i64, high: i64) -> Result<ExactPlan, String> {
        let min = first_failure(low, high, |m| !self.grids_fit(m, low));
        let at = min.min(high);
        let max = first_failure(at, high, |big_m| self.fit(at, big_m, high).is_some()) - 1;
        if min > high || max < min {
            let degree = self.operand_degree();
            return Err(format!(
                "no input range keeps every exact product inside the operand domain \
                 [2^{low}, 2^{high}] of the exact arithmetic for coordinates of up to {} \
                 components: a product operand of degree {degree} needs nonzero components \
                 of magnitude at least 2^{min}, and keeping every operand within the domain \
                 and every value below 2^{OVERFLOW_EXPONENT} needs components of at most \
                 2^{max}",
                self.components
            ));
        }
        let lengths = self.fit(min, max, high).expect("the range fits");
        let too_large = || {
            "the expression is too large for exact evaluation: its temporaries \
             would need 2^32 components or more in all"
                .to_string()
        };
        let total = lengths
            .iter()
            .try_fold(0u128, |sum, &n| sum.checked_add(n))
            .filter(|&total| total < u128::from(MAX_LENGTH))
            .ok_or_else(too_large)?;
        Ok(ExactPlan {
            range: ExactRange {
                min: Dyadic::pow2(min).round_up(),
                max: Dyadic::pow2(max).round_up(),
            },
            lengths: lengths
                .iter()
                .map(|&n| usize::try_from(n).expect("below 2^32"))
                .collect(),
            total: usize::try_from(total).expect("below 2^32"),
        })
    }

    /// The highest degree of a product operand, saturated.
    fn operand_degree(&self) -> u64 {
        let mut degrees: Vec<u64> = Vec::with_capacity(self.ops.len());
        let mut highest = 0;
        for op in &self.ops {
            let degree = match *op {
                Op::Leaf => 1,
                Op::Constant(_) => 0,
                Op::Neg(x) => degrees[x],
                Op::Sum(x, y) => degrees[x].max(degrees[y]),
                Op::Product(x, y) => {
                    highest = highest.max(degrees[x]).max(degrees[y]);
                    degrees[x].saturating_add(degrees[y])
                }
            };
            degrees.push(degree);
        }
        highest
    }

    /// For components of magnitude at least `2^m`, the exponent of a power
    /// of two dividing every component of each node, or `None` for a node
    /// that is identically zero; saturated at `i64::MIN`, which is below
    /// every domain.
    fn grids(&self, m: i64) -> Vec<Option<i64>> {
        let mut grids: Vec<Option<i64>> = Vec::with_capacity(self.ops.len());
        for op in &self.ops {
            let grid = match *op {
                Op::Leaf => Some(m - 52),
                Op::Constant(n) => (n != 0).then(|| i64::from(n.trailing_zeros())),
                Op::Neg(x) => grids[x],
                Op::Sum(x, y) => match (grids[x], grids[y]) {
                    (Some(gx), Some(gy)) => Some(gx.min(gy)),
                    (grid, None) | (None, grid) => grid,
                },
                Op::Product(x, y) => grids[x].zip(grids[y]).map(|(gx, gy)| gx.saturating_add(gy)),
            };
            grids.push(grid);
        }
        grids
    }

    /// Whether, with nonzero components of magnitude at least `2^m`, every
    /// nonzero component of every product operand is at least `2^low`.
    fn grids_fit(&self, m: i64, low: i64) -> bool {
        let grids = self.grids(m);
        self.ops.iter().all(|op| match *op {
            Op::Product(x, y) => [x, y]
                .into_iter()
                .all(|n| grids[n].is_none_or(|g| g >= low)),
            _ => true,
        })
    }

    /// For nonzero components of magnitude in `[2^m, 2^big_m]`, the capped
    /// length of every node when every product operand has a `B` of at
    /// most `2^high`, every node a `B` below the overflow limit and a count
    /// of at most [`MAX_OPERATIONS`], `2^50`; `None` otherwise. Stops at the
    /// first node that fails, so the work stays small however large the
    /// definition.
    fn fit(&self, m: i64, big_m: i64, high: i64) -> Option<Vec<u128>> {
        self.fit_with_counts(m, big_m, high)
            .map(|(lengths, _)| lengths)
    }

    /// [`Arena::fit`], with the count `T` of every node.
    fn fit_with_counts(&self, m: i64, big_m: i64, high: i64) -> Option<(Vec<u128>, Vec<u128>)> {
        let grids = self.grids(m);
        // A leaf has as many components as the factor of `M` in its `S`.
        let (leaf_length, leaf_operations) = if self.translated {
            (2 * self.components, 4 * self.components)
        } else {
            (self.components, 0)
        };
        let leaf = &Dyadic::integer(leaf_length) * &Dyadic::pow2(big_m);
        let zero = Dyadic::integer(0);
        let overflow = Dyadic::pow2(OVERFLOW_EXPONENT);
        let operand_max = Dyadic::pow2(high);
        let six_u = Dyadic::new(6, -53);
        let one = Dyadic::integer(1);
        let n = self.ops.len();
        let mut magnitudes: Vec<Dyadic> = Vec::with_capacity(n);
        let mut bounds: Vec<Dyadic> = Vec::with_capacity(n);
        let mut lengths: Vec<u128> = Vec::with_capacity(n);
        let mut operations: Vec<u128> = Vec::with_capacity(n);
        for (node, op) in self.ops.iter().enumerate() {
            let magnitude = match *op {
                Op::Leaf => leaf.clone(),
                Op::Constant(c) => Dyadic::integer(c.unsigned_abs()),
                Op::Neg(x) => magnitudes[x].clone(),
                Op::Sum(x, y) => &magnitudes[x] + &magnitudes[y],
                Op::Product(x, y) => {
                    if !(bounds[x] <= operand_max && bounds[y] <= operand_max) {
                        return None;
                    }
                    &magnitudes[x] * &magnitudes[y]
                }
            };
            if magnitude >= overflow {
                return None;
            }
            let structural = match *op {
                Op::Leaf => leaf_length,
                Op::Constant(c) => u128::from(c != 0),
                Op::Neg(x) => lengths[x],
                Op::Sum(x, y) => lengths[x].saturating_add(lengths[y]),
                Op::Product(x, y) => lengths[x].saturating_mul(lengths[y]).saturating_mul(2),
            };
            let length = match grids[node] {
                None => 0,
                Some(_) if magnitude == zero => 0,
                Some(grid) => {
                    let bits = magnitude.ceil_log2().saturating_sub(grid).saturating_add(1);
                    structural.min(u128::try_from(bits).unwrap_or(0))
                }
            };
            let own = match *op {
                Op::Leaf => leaf_operations,
                Op::Constant(_) => 0,
                Op::Neg(x) => lengths[x].saturating_mul(2),
                Op::Sum(x, y) => lengths[x].saturating_add(lengths[y]).saturating_mul(2),
                Op::Product(x, y) => {
                    let (s, t) = (lengths[x].min(lengths[y]), lengths[x].max(lengths[y]));
                    s.saturating_mul(t.saturating_mul(7).saturating_add(length.saturating_mul(2)))
                }
            };
            let below = match *op {
                Op::Leaf | Op::Constant(_) => 0,
                Op::Neg(x) => operations[x],
                Op::Sum(x, y) | Op::Product(x, y) => operations[x].saturating_add(operations[y]),
            };
            let count = below.saturating_add(own);
            if count > MAX_OPERATIONS {
                return None;
            }
            let bound = &magnitude * &(&one + &(&six_u * &Dyadic::integer(count)));
            if bound >= overflow {
                return None;
            }
            magnitudes.push(magnitude);
            bounds.push(bound);
            lengths.push(length);
            operations.push(count);
        }
        Some((lengths, operations))
    }
}

#[cfg(test)]
mod tests;
