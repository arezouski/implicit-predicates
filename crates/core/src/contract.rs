//! The obligations of the contract of [`Point`](crate::Point) that need no
//! room in an arena, for the debug assertions of the predicates and for
//! [`point::check`](crate::point::check). Not part of the public API.

use crate::bigint::Big;
use crate::expansion;
use crate::scratch::{Scratch, Span};

/// Whether every error bound is valid: not NaN and not negative. `+∞` is
/// valid.
#[must_use]
pub fn err_is_valid(err: &[f64; 3]) -> bool {
    err.iter().all(|&e| axis_err_is_valid(e))
}

/// Whether the error bound of one axis is valid: not NaN and not negative.
pub(crate) fn axis_err_is_valid(e: f64) -> bool {
    e >= 0.0
}

/// Whether `span` holds a valid expansion of `scratch` (see
/// [`expansion`]). A span that reaches past the top of `scratch` does not.
#[must_use]
pub fn expansion_is_valid(scratch: &Scratch<'_>, span: Span) -> bool {
    scratch.holds(span) && expansion::is_valid(scratch.get(span))
}

/// Whether `b`, whose words are in `scratch`, is in its unique
/// representation (see [`bigint`](crate::bigint)). A value whose words
/// reach past the top of `scratch` is not.
#[must_use]
pub fn big_is_canonical(scratch: &Scratch<'_>, b: Big) -> bool {
    b.is_canonical(scratch)
}
