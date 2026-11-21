# implicit-predicates-core

[![crates.io](https://img.shields.io/crates/v/implicit-predicates-core.svg)](https://crates.io/crates/implicit-predicates-core)
[![docs.rs](https://img.shields.io/docsrs/implicit-predicates-core)](https://docs.rs/implicit-predicates-core)
[![CI](https://github.com/arezouski/implicit-predicates/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/arezouski/implicit-predicates/actions/workflows/ci.yml)

The `no_std`, dependency-free core of
[`implicit-predicates`](https://crates.io/crates/implicit-predicates): the
arithmetic building blocks shared by the predicates and by generated code.

> **Status: early development.** The API is incomplete and may change.

Currently it provides:

- `Sign`, the result type of every predicate;
- `eft`, error-free transformations: floating-point sums and products
  together with their exact rounding errors;
- `expansion`, exact values represented as sums of `f64` components, with
  exact addition, subtraction, scaling and sign, all without allocation;
- `Scratch`, a bump arena over a borrowed buffer that holds the expansions
  of a computation and runs those operations on them, as well as the exact
  product of two expansions; `Scratch::write` lets other code write
  expansions of its own into it;
- `Point`, the contract every point type implements: an approximation of its
  coordinates, a bound on their error, and its exact coordinates, as
  expansions and as big integers, in an arena. `[f64; 3]` implements it as
  an explicit point;
- `check`, with `CheckError` and `Violation`, which tests that a point keeps
  the contract of `Point`, for the tests of a point type. It sees only what
  the views of a point show of it. In debug builds, the predicates of
  `implicit-predicates` that take no arena run it on each point when a call
  reaches their exact stages; release builds never do;
- `Big`, exact integers of arbitrary size in an arena, built from `f64`
  values, with exact addition, subtraction and multiplication;
- `filter`, the helpers that turn the errors of the points of a call into
  the uncertainty of the floating-point filter of a predicate.

The exact products require operands in a documented range, the product
domain (`eft::in_product_domain`); debug builds check it.

The optional `alloc` feature adds `Scratch::growable`, an arena that grows on
the heap so the exact arithmetic can always complete. Without it the crate
never allocates.

Most users should depend on `implicit-predicates` instead.

## License

Licensed under either of Apache License, Version 2.0 or MIT license, at your
option.
