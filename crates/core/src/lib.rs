//! Arithmetic building blocks for exact geometric predicates.
//!
//! This is the `no_std`, dependency-free core of the `implicit-predicates`
//! crates. Most users should depend on `implicit-predicates` instead.
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
//!   well as the exact product of two expansions; [`Scratch::write`] lets
//!   other code write expansions of its own into it;
//! - [`point`], [`Point`], the contract every point type implements: an
//!   approximation of its coordinates, a bound on their error, and its exact
//!   coordinates as expansions in an arena. `[f64; 3]` implements it as an
//!   explicit point;
//! - [`filter`], the helpers that turn the errors of the points of a call
//!   into the uncertainty of the floating-point filter of a predicate.
//!
//! The optional `alloc` feature adds an owned arena that grows on the heap,
//! `Scratch::growable`, so the exact arithmetic can always complete. Without
//! it nothing allocates.
//!
//! **Status: early development.** The API is incomplete and may change.

#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod bigint;
pub mod eft;
pub mod expansion;
pub mod filter;
pub mod point;
pub mod scratch;
mod sign;

pub use bigint::Big;
pub use point::{ExactCoords, ExactError, Point, Weight};
pub use scratch::{Mark, Scratch, ScratchExhausted, Span};
pub use sign::Sign;
