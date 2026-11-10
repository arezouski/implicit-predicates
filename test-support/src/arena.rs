//! Random sequences of operations on a `Scratch` arena.

use indirect_predicates_core::{Scratch, ScratchExhausted, Span, eft};
use proptest::prelude::*;

use crate::{bit_pattern_expansion, expansion_with, leaf};

/// The longest operand an [`ArenaOp::Product`] takes, so that the working
/// space of a sequence of products stays modest.
pub const PRODUCT_OPERAND_LIMIT: usize = 4;

/// One arena operation in a random sequence. Operand indices select, modulo
/// their number, among the spans produced so far.
#[derive(Clone, Debug)]
pub enum ArenaOp {
    /// `Scratch::push` of the expansion.
    Push(Vec<f64>),
    /// `Scratch::sum`.
    Sum(usize, usize),
    /// `Scratch::diff`.
    Diff(usize, usize),
    /// `Scratch::scale` by the factor.
    Scale(usize, f64),
    /// `Scratch::product`.
    Product(usize, usize),
}

/// Arena operations. Pushed expansions come from [`expansion_with`] with
/// leaves in `2^-40 ..= 2^41`, or from [`bit_pattern_expansion`] with bits in
/// `2^-120 ..= 2^120`; scale factors have at most 8 significant bits and
/// magnitude in `2^-20 ..= 2^21`.
pub fn arena_op() -> impl Strategy<Value = ArenaOp> {
    prop_oneof![
        expansion_with(8, leaf(-40, 40, 53)).prop_map(ArenaOp::Push),
        bit_pattern_expansion(12, -120, 120).prop_map(ArenaOp::Push),
        (any::<usize>(), any::<usize>()).prop_map(|(a, b)| ArenaOp::Sum(a, b)),
        (any::<usize>(), any::<usize>()).prop_map(|(a, b)| ArenaOp::Diff(a, b)),
        (any::<usize>(), leaf(-20, 20, 8)).prop_map(|(a, b)| ArenaOp::Scale(a, b)),
        (any::<usize>(), any::<usize>()).prop_map(|(a, b)| ArenaOp::Product(a, b)),
    ]
}

/// An operation that ran: the positions, among the spans it was given, of
/// its operands, and what the arena returned.
#[derive(Debug)]
pub struct Applied {
    /// Indices into the spans passed to [`ArenaOp::apply`].
    pub operands: Vec<usize>,
    /// The result of the arena operation.
    pub result: Result<Span, ScratchExhausted>,
}

impl ArenaOp {
    /// Runs the operation on `scratch` with operands picked among `spans`.
    ///
    /// Returns `None`, without touching the arena, when the operation is
    /// skipped: it needs operands and `spans` is empty, or it multiplies and
    /// an operand has a component outside the product domain of
    /// `eft::in_product_domain` (or, for a product, more than
    /// [`PRODUCT_OPERAND_LIMIT`] components). Results of sums and products
    /// can leave that domain, so the check keeps every sequence inside the
    /// requirements of `scale` and `product`.
    pub fn apply(&self, scratch: &mut Scratch<'_>, spans: &[Span]) -> Option<Applied> {
        let index = |i: usize| i.checked_rem(spans.len());
        let (operands, result) = match *self {
            ArenaOp::Push(ref e) => (vec![], scratch.push(e)),
            ArenaOp::Sum(a, b) => {
                let (a, b) = (index(a)?, index(b)?);
                (vec![a, b], scratch.sum(spans[a], spans[b]))
            }
            ArenaOp::Diff(a, b) => {
                let (a, b) = (index(a)?, index(b)?);
                (vec![a, b], scratch.diff(spans[a], spans[b]))
            }
            ArenaOp::Scale(a, factor) => {
                let a = index(a)?;
                if !(in_product_domain(scratch, spans[a]) && eft::in_product_domain(factor)) {
                    return None;
                }
                (vec![a], scratch.scale(spans[a], factor))
            }
            ArenaOp::Product(a, b) => {
                let (a, b) = (index(a)?, index(b)?);
                let fits =
                    |s: Span| s.len() <= PRODUCT_OPERAND_LIMIT && in_product_domain(scratch, s);
                if !(fits(spans[a]) && fits(spans[b])) {
                    return None;
                }
                (vec![a, b], scratch.product(spans[a], spans[b]))
            }
        };
        Some(Applied { operands, result })
    }
}

/// Whether every component of the expansion at `s` lies in the product
/// domain.
fn in_product_domain(scratch: &Scratch<'_>, s: Span) -> bool {
    scratch.get(s).iter().all(|&x| eft::in_product_domain(x))
}

/// The space `Scratch::product` needs above the top while it works, for
/// operands of `la` and `lb` components, as its documentation states.
pub fn product_working_space(la: usize, lb: usize) -> usize {
    if la == 0 || lb == 0 {
        0
    } else {
        4 * la * lb + 2 * la.max(lb)
    }
}
