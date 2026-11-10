//! Arithmetic building blocks for exact geometric predicates.
//!
//! This is the `no_std`, dependency-free core of the `indirect-predicates`
//! crates. Most users should depend on `indirect-predicates` instead.
//!
//! Currently it provides [`Sign`], the result type of every predicate.
//!
//! **Status: early development.** The API is incomplete and may change.

#![no_std]

pub mod sign;

pub use sign::Sign;
