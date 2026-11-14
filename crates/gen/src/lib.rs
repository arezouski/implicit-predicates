//! Code generator for exact geometric predicates.
//!
//! Turns the definition of a predicate or of a point type into specialized
//! Rust code for the `implicit-predicates` crates.
//!
//! **Status: early development.** [`generate`] takes the definitions of a
//! `.pred` file and returns, for each predicate, the Rust module of its
//! stages: a floating-point filter, exact stages on floating-point
//! expansions, and the chain that runs them (see [`generate`] for the items
//! of the module), and the module of its checks for tests: an exact
//! evaluator and inputs at the edge of the error bound of the filter. The
//! `implicit-predicates-gen` command line tool writes those modules for a
//! directory of definitions, formatted with `rustfmt`, or checks with
//! `--check` that the files are up to date; its README gives the command
//! that regenerates the code of the `implicit-predicates` crate.
//!
//! The steps of the generation are public as well. [`parse`] takes a `.pred`
//! file, written in a small subset of Rust, and returns each [`Predicate`] as
//! an expression [`Graph`] in which equal subexpressions share a node.
//! Mistakes in the file are reported as an [`Error`] that carries the position
//! of the offending token.
//!
//! A definition is a function over `Point3` parameters returning `Sign`. Its
//! body holds `let` bindings and a final expression built from `+`, `-` (binary
//! and unary), `*`, parentheses, integer literals, the coordinates `p.x`,
//! `p.y`, `p.z` of a parameter and the names of earlier bindings. A second
//! function with the same name and parameters, marked `#[exact]`, may give
//! the same polynomial in another form, such as on the raw coordinates:
//! [`generate`] only checks that it is the same polynomial as the main form,
//! and no emitted stage evaluates it. Comments cost nothing; doc comments
//! (`///`, `//!`, `/** */`, `/*! */`) are rejected, and so are predicate
//! names that cannot name a module, such as keywords.
//!
//! A `.pred` file may also declare point types, which [`parse_points`] reads
//! and [`parse`] skips: the word `point`, a name, parameters of type `F64x3`
//! or `f64`, `=`, an expression and `;`, as in
//! `point Lnc(p: F64x3, q: F64x3, t: f64) = q + t*(p - q) where 0 <= t, t <= 1;`.
//! The expression uses `+`, `-`, `*`, parentheses, integer literals and the
//! parameters; operations on an `F64x3` are per coordinate, a product has at
//! least one `f64` factor, and the value is an `F64x3`. The optional `where`
//! clause is a list of comparisons `<=` between sums of `f64` parameters and
//! integers. A [`PointType`] holds the three coordinates as one
//! [`PointGraph`] of `f64` operations in which equal subexpressions share a
//! node, and the clause as a list of [`Constraint`]s. The limit on brackets
//! and the guarantee of never panicking are those of [`parse`].
//! [`generate_points`] emits the module of the point types of a file, with
//! their constructors, their error bounds and their exact coordinates;
//! [`error_bound`] derives the rule of the error bound of each coordinate,
//! and documents why it is an upper bound. [`generate_points`] has budgets
//! too: [`MAX_ROUNDS`] roundings on a chain of operations, and
//! [`MAX_POINT_NODES`] distinct operations and inputs in the coordinates of a
//! point, which bounds the size of the emitted code.
//!
//! The only limit of [`parse`] is the nesting of brackets, at most 32 deep,
//! which keeps the stack small: sums, products and runs of unary minus are
//! read in loops, so [`parse`] cannot panic or overflow the stack of the
//! calling thread, even in a debug build with a 1 MiB stack, and it accepts
//! a sum of any length. The later steps have budgets instead: a sum written
//! as one chain of more than about 1,560 terms is rejected by
//! [`filter_bound`], because the exact coefficients of its error bound grow
//! with the length of the chain, an expression too large to expand is
//! rejected by [`canonical`], and [`generate`] rejects main forms of more
//! than 256 distinct subexpressions, for which the search of the edge cases
//! of the checks would be slow. Each reports the limit it hit, before doing
//! the work; the work section of [`generate`] lists every budget.
//!
//! Two analyses work on the graphs. [`check_same_polynomial`] expands the
//! main and the exact form of a predicate with [`canonical`] and requires
//! them to be the same, nonzero polynomial. [`difference_graph`] checks that
//! a form is homogeneous in differences of point coordinates, that is, built
//! from differences of one coordinate of two points, combined by sums of
//! terms of equal degree and by products, and returns it as a [`DiffGraph`]
//! with the degree of every node.
//!
//! [`filter_bound`] derives, in exact arithmetic, the error bound of
//! evaluating a [`DiffGraph`] in `f64`: with `D` the largest computed
//! difference plus the two largest errors of the points, and `E` the sum of
//! those two errors, the computed value differs from the exact one by at most
//! `(epsilon * D + kappa * E) * D^(k - 1)`, where `k` is the degree.
//! [`FilterBound::constants`] gives `epsilon` and `kappa` rounded up to `f64`,
//! and the range of `D`, between two powers of two, in which overflow and
//! underflow cannot invalidate the bound. [`filter_form`] chooses the
//! per-axis form instead when every term has the same degree in each axis
//! ([`DiffGraph::axis_degrees`]): an [`AxisBound`] keeps one size and one
//! uncertainty per axis, with the same `epsilon`, one `kappa` per axis that
//! add up to the global `kappa`, and the same range for each axis.
//!
//! [`exact_plan_differences`] derives, for each exact stage on the stack,
//! the range of input magnitudes for which every exact product of the
//! arithmetic core stays in its domain and no value overflows, and the
//! worst-case length of every temporary; [`generate`] sizes the stack
//! buffers of the emitted stages from them. [`exact_plan_spans_translated`]
//! derives the same for the stage over arena spans, whose leaves are expansions of up to
//! [`MAX_COORD_COMPONENTS`] components: the range bounds every nonzero
//! component, so it does not depend on the type of the points.

#![forbid(unsafe_code)]

mod bound;
mod cursor;
mod differences;
mod dyadic;
mod emit;
mod error;
mod expr;
mod graph;
mod limits;
mod parse;
mod point;
mod polynomial;
mod range;

pub use bound::{
    AxisBound, AxisConstants, FilterBound, FilterConstants, FilterForm, filter_bound, filter_form,
};
pub use differences::{DiffGraph, DiffNode, difference_graph};
pub use emit::{Generated, generate, generate_points};
pub use error::Error;
pub use graph::{Graph, Node, NodeId};
pub use parse::{Position, Predicate, parse};
pub use point::{
    Constraint, ErrorBound, LinearSum, MAX_POINT_NODES, MAX_ROUNDS, ParamType, PointGraph,
    PointNode, PointParam, PointType, error_bound, parse_points,
};
pub use polynomial::{Polynomial, canonical, canonical_of_differences, check_same_polynomial};
pub use range::{
    ExactPlan, ExactRange, Leaves, MAX_COORD_COMPONENTS, exact_plan_differences,
    exact_plan_spans_translated,
};
