//! Arithmetic building blocks for exact geometric predicates.
//!
//! This is the `no_std`, dependency-free core of the `indirect-predicates`
//! crates. Most users should depend on `indirect-predicates` instead.
//!
//! Currently it provides:
//!
//! - [`Sign`], the result type of every predicate;
//! - [`eft`], error-free transformations: floating-point sums and products
//!   together with their exact rounding errors.
//!
//! **Status: early development.** The API is incomplete and may change.

#![no_std]

pub mod eft;
pub mod sign;

pub use sign::Sign;
