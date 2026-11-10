//! Exact geometric predicates over explicit and implicit points.
//!
//! An implicit point is defined by an unevaluated construction, such as a
//! point on an edge or inside a triangle. Predicates receive the construction
//! itself and return the exact sign, without rounding the point's
//! coordinates.
//!
//! **Status: early development.** This crate has no usable API yet.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
