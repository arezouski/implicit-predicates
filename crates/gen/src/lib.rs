//! Code generator for exact geometric predicates.
//!
//! Turns the definition of a predicate or of a point type into specialized
//! Rust code for the `indirect-predicates` crates.
//!
//! **Status: early development.** Today the crate reads predicate definitions:
//! [`parse`] takes a `.pred` file, written in a small subset of Rust, and
//! returns each [`Predicate`] as an expression [`Graph`] in which equal
//! subexpressions share a node. Mistakes in the file are reported as an
//! [`Error`] that carries the position of the offending token.
//!
//! A definition is a function over `Point3` parameters returning `Sign`. Its
//! body holds `let` bindings and a final expression built from `+`, `-` (binary
//! and unary), `*`, parentheses, integer literals, the coordinates `p.x`,
//! `p.y`, `p.z` of a parameter and the names of earlier bindings. A second
//! function with the same name and parameters, marked `#[exact]`, gives the
//! form used for exact evaluation.
//!
//! The only limit is the nesting of brackets, at most 32 deep, which keeps
//! the stack small: sums, products and runs of unary minus are read in loops
//! and have no limit beyond memory, so [`parse`] cannot panic or overflow the
//! stack of the calling thread, even in a debug build with a 1 MiB stack.
//! Comments cost nothing.
//!
//! Two analyses work on the graphs. [`check_same_polynomial`] expands the
//! main and the exact form of a predicate with [`canonical`] and requires
//! them to be the same, nonzero polynomial. [`difference_graph`] checks that
//! a form is homogeneous in differences of point coordinates, that is, built
//! from differences of one coordinate of two points, combined by sums of terms
//! of equal degree and by products, and returns it as a [`DiffGraph`] with the
//! degree of every node.

#![forbid(unsafe_code)]

mod cursor;
mod differences;
mod error;
mod expr;
mod graph;
mod limits;
mod parse;
mod polynomial;

pub use differences::{DiffGraph, DiffNode, difference_graph};
pub use error::Error;
pub use graph::{Graph, Node, NodeId};
pub use parse::{Predicate, parse};
pub use polynomial::{Polynomial, canonical, check_same_polynomial};
