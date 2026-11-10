//! Arithmetic building blocks for exact geometric predicates.
//!
//! This is the `no_std`, dependency-free core of the `indirect-predicates`
//! crates. Most users should depend on `indirect-predicates` instead.
//!
//! Currently it provides:
//!
//! - [`Sign`], the result type of every predicate;
//! - [`eft`], error-free transformations: floating-point sums and products
//!   together with their exact rounding errors;
//! - [`expansion`], exact values represented as sums of `f64` components,
//!   with exact addition, subtraction, scaling and sign;
//! - [`scratch`], [`Scratch`], a bump arena over a borrowed buffer that holds
//!   the expansions of a computation and runs those operations on them, as
//!   well as the exact product of two expansions.
//!
//! The optional `alloc` feature adds an owned arena that grows on the heap,
//! `Scratch::growable`, so the exact arithmetic can always complete. Without
//! it nothing allocates.
//!
//! **Status: early development.** The API is incomplete and may change.

#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod eft;
pub mod expansion;
pub mod scratch;
mod sign;

pub use scratch::{Mark, Scratch, ScratchExhausted, Span};
pub use sign::Sign;
