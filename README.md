# indirect-predicates

Exact geometric predicates in Rust for explicit points and for *implicit*
points: points defined by an unevaluated construction, such as a point on an
edge or inside a triangle.

> **Status: early development.** There are no predicates yet. The exact
> arithmetic they will be built on is in place: `indirect-predicates-core`
> provides error-free transformations, floating-point expansions and a
> scratch arena for exact computations, and `indirect-predicates-oracle`
> provides the exact reference used to test them. `indirect-predicates` and
> `indirect-predicates-gen` are still placeholders, and every API is subject
> to change.

## Why

A geometric predicate answers a yes/no/degenerate question, for example "on
which side of the plane *abc* does *d* lie?". Evaluated naively in floating
point, the answer can be wrong near degenerate configurations, which breaks
the algorithms built on top of it.

Robust predicates for points with `f64` coordinates are well covered in Rust.
Meshing algorithms, however, also create new points (the intersection of an
edge with a plane, a point at a given parameter along a segment) whose exact
coordinates are not representable as `f64`. Rounding those coordinates
reintroduces the errors the predicates were meant to remove.

*Indirect predicates* avoid this by keeping such points implicit: a predicate
receives the construction itself and returns the exact sign, without ever
computing the point's coordinates.

## Goals

- Exact sign for every finite input.
- Speed close to classical filtered predicates in the common case: most calls
  are decided by a floating-point filter, without heap allocation.
- Explicit points and implicit points through one API.
- A code generator, so new predicates and new point types can be defined
  once and get the same staged evaluation as the built-in ones.
- `no_std` core with no dependencies.

Planned for the first release: `orient3d`, `insphere` and a symbolically
perturbed `insphere`, over explicit points and points on a segment.

## Workspace

| Crate | Purpose |
| --- | --- |
| [`indirect-predicates`](crates/indirect-predicates) | Point types and predicates. The crate most users want |
| [`indirect-predicates-core`](crates/core) | Shared arithmetic building blocks. `no_std` |
| [`indirect-predicates-gen`](crates/gen) | Code generator for predicates and point types |
| [`indirect-predicates-oracle`](crates/oracle) | Slow exact reference implementation, used for testing |

Four more crates are not published:

- `benches`: the benchmarks;
- `test-support`: the test infrastructure shared by the crates above;
- `corpus`: reads, checks and summarizes predicate calls recorded from a
  reference mesher; its tools record them from a sample of the Thingi10K
  models;
- `ffi-ref`: outside the workspace, binds two third-party C++ reference
  predicate libraries for comparison.

The libraries `ffi-ref` binds are licensed under the LGPL and are linked
only for local tests and benchmarks: they are downloaded at pinned commits
into a git-ignored directory, and no part of them is included in this
repository or in the published crates.

## Minimum supported Rust version

Rust 1.86.

## Background

- J. R. Shewchuk, *Adaptive Precision Floating-Point Arithmetic and Fast
  Robust Geometric Predicates*, 1997.
- M. Attene, *Indirect Predicates for Geometric Constructions*, 2020.
- L. Diazzi, D. Panozzo, A. Vaxman, M. Attene, *Constrained Delaunay
  Tetrahedrization: A Robust and Practical Approach*, 2023.

This project is an independent implementation written from the published
papers.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or
  <https://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
