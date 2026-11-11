//! Slow, exact reference implementation used to test `implicit-predicates`.
//!
//! Meant for tests only, not for production use.
//!
//! Currently provides [`Dyadic`], an exact dyadic rational number that every
//! finite `f64` converts into without loss.

#![forbid(unsafe_code)]

mod dyadic;

pub use dyadic::Dyadic;
