# implicit-predicates

Exact geometric predicates in Rust for explicit points and for *implicit*
points: points defined by an unevaluated construction, such as the point at a
given parameter on a segment.

> **Status: early development.** Three predicates exist so far, in
> `implicit-predicates`, over explicit points and points on a segment:
> `orient3d`, `insphere` and `insphere_sos`, which is `insphere` with its
> ties broken by the order of the indices of the points. They are generated
> by `implicit-predicates-gen` from short definitions and evaluated in
> stages: a floating-point filter and a last exact stage on big integers
> (`orient3d` has exact stages on floating-point expansions in between), so
> each returns the exact sign for every finite input. Other kinds of
> implicit points (on a triangle, at the intersection of a line and a
> plane, or of three planes) are not implemented. Versions 0.0.x are
> development releases: each of them may change the API.

Add it to a project with `cargo add implicit-predicates`. The API is
documented at <https://docs.rs/implicit-predicates>, and the changes of each
release are in the [changelog](CHANGELOG.md).

## Example

```rust
use implicit_predicates::{Lnc, Sign, orient3d};

let (a, b, c) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
// a, b and c appear counter-clockwise from (1, 1, 1), above the plane.
assert_eq!(orient3d(&a, &b, &c, &[1.0, 1.0, 1.0]), Sign::Negative);
assert_eq!(orient3d(&a, &b, &c, &[0.0, 0.0, 0.0]), Sign::Positive);
// A point on the plane.
assert_eq!(orient3d(&a, &b, &c, &[0.5, 0.25, 0.25]), Sign::Zero);
// A point on the edge between b and c, c + 0.3 (b - c): on the plane,
// exactly, although its coordinates are not representable as `f64`.
let m = Lnc::new(b, c, 0.3).unwrap();
assert_eq!(orient3d(&a, &b, &c, &m), Sign::Zero);
```

The `orient3d` sign follows Shewchuk's convention: positive when `d` lies
below the plane through `a`, `b` and `c`, taking "above" to be the side from
which they appear counter-clockwise. `insphere` follows it too: for four
points that `orient3d` finds positively oriented, its sign is positive when
the fifth is inside their sphere. `insphere_sos` takes five distinct indices
and never returns zero unless the five points are coplanar. The predicates
are checked against an exact reference, against the `robust` crate on
explicit points, and against predicate calls recorded from a real mesher,
with explicit points and with points on a segment.

## Points

The predicates take points of any type that implements the `Point` trait:

- `[f64; 3]`, an explicit point, not validated;
- `Explicit`, an explicit point whose constructor rejects coordinates that
  are not finite;
- `Lnc`, built from two explicit points `p` and `q` and a parameter `t`:
  the point `q + t (p - q)` on the segment between them. `Lnc::new` checks
  that the inputs are finite and that `0 <= t <= 1`; `Lnc::new_unchecked`
  accepts any `f64`;
- `AnyPoint`, either an `Explicit` or an `Lnc`, for collections that mix
  them.

A point gives a predicate an approximation of its coordinates in `f64`, a
bound on the error of each one, and, only when the filter cannot decide, its
exact coordinates. Most calls are decided by the filter, without heap
allocation. When it cannot decide, `orient3d` on explicit points goes
through exact stages whose temporaries are on the stack, so in release
builds no call on explicit points whose nonzero coordinates have magnitudes
in `2^-188..=2^238` allocates; debug builds allocate an arena for the check
of the contract of the points. With a point on a segment, and for `insphere`
and `insphere_sos` whatever the points, the exact coordinates are written to
an arena of the calling thread, which is allocated on the first such call
and grows when a call needs more; explicit points outside that range use the
same arena, for the last stage described below. `orient3d_in`,
`insphere_in` and `insphere_sos_in` take a caller-provided arena (`Scratch`)
instead and report `ScratchExhausted` if it is too small.

The predicates without the `_in` suffix need the `std` feature, on by
default. Without it the crate is `no_std` and the `_in` functions are the way
to call the predicates.

The stages on expansions of `orient3d` decide every call whose coordinates
are of moderate magnitude. The others, with a coordinate or a component of
an exact coordinate too large or too small for them, go to a last stage that
evaluates the determinant on big integers, in the same arena; it is slower,
and decides every input, and it is the only exact stage of `insphere`. So
the result is the exact sign for every finite input. An input that is not
finite (NaN or infinite) makes the predicates panic.

### Point types of your own

`Point` is a public trait, so a type of your own can be passed to the
predicates. Its implementation carries obligations: `err` must bound the
error of each coordinate of `approx`; `is_explicit` may return `true` only
when `approx` holds the exact coordinates, with zero error; `exact` must
write valid expansions; and `exact_big` must give the same coordinates as
`exact` whenever `exact` succeeds, fail with `OutOfRange` only for inputs
that are not finite, and fail with `Weighted` for exact coordinates with a
weight other than one. The trait documentation states them in full.

Release builds of the predicates do not check them, and an implementation
that breaks them makes the predicates return wrong signs, silently. `check`
tests that a point keeps the contract, and is meant for the tests of a type
of your own. Debug builds catch part of the violations with a panic
(``breaks the contract of `Point`: ...``), and the predicates without the
`_in` suffix run `check` on each point when a call reaches the exact stages;
`check` sees only what the views of a point show, so it cannot know that the
exact coordinates are those the point is meant to have. The point types of
this crate are not written by hand: `implicit-predicates-gen` emits them from
short declarations, with error bounds it derives operation by operation.

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

## Workspace

| Crate | Purpose |
| --- | --- |
| [`implicit-predicates`](crates/implicit-predicates) | Predicates, with the code generated for them. The crate most users want |
| [`implicit-predicates-core`](crates/core) | Shared arithmetic building blocks. `no_std` |
| [`implicit-predicates-gen`](crates/gen) | Code generator for predicates and point types |
| [`implicit-predicates-oracle`](crates/oracle) | Slow exact reference implementation, used for testing |

Four more crates are not published:

- `benches`: the benchmarks;
- `test-support`: the test infrastructure shared by the crates above (the
  tests of `implicit-predicates` use it, together with
  `implicit-predicates-oracle` and `corpus`, and are not part of its
  published package);
- `corpus`: reads, checks and summarizes predicate calls recorded from a
  reference mesher; its tools record them from a sample of the Thingi10K
  models;
- `ffi-ref`: outside the workspace, binds two third-party C++ reference
  predicate libraries for comparison.

A fuzzing package, `fuzz`, is also outside the workspace and not published:
`cargo-fuzz` targets that compare the public predicates with the exact
reference (see its README). The tests also run on `wasm32-wasip1`, under
wasmtime, to cover a 32-bit `usize`.

The libraries `ffi-ref` binds are licensed under the LGPL and are linked
only for local tests and benchmarks: they are downloaded at pinned commits
into a git-ignored directory, and no part of them is included in this
repository or in the published crates.

## Development

```sh
cargo test --workspace --all-features
```

runs the tests, which compare every stage of the predicates with the exact
reference of `implicit-predicates-oracle`, on random, degenerate and recorded
inputs. The code of the predicates and of the point types is generated and
versioned; the command that regenerates it, and checks that it is up to
date, is in the README of [`implicit-predicates-gen`](crates/gen). The
READMEs of `fuzz`, `corpus/tools` and `ffi-ref` say how to run the fuzzing
targets, record a corpus and build the bindings of the reference libraries.

## Minimum supported Rust version

Rust 1.86.

## Background

- J. R. Shewchuk, *Adaptive Precision Floating-Point Arithmetic and Fast
  Robust Geometric Predicates*, 1997.
- M. Attene, *Indirect Predicates for Geometric Constructions*, 2020.
- L. Diazzi, D. Panozzo, A. Vaxman, M. Attene, *Constrained Delaunay
  Tetrahedrization: A Robust and Practical Approach*, 2023.

This project is an independent implementation written from the published
papers. It is not affiliated with the authors' C++ libraries and shares no
code with them.

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
