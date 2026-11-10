# indirect-predicates-core

The `no_std`, dependency-free core of
[`indirect-predicates`](https://crates.io/crates/indirect-predicates): the
arithmetic building blocks shared by the predicates and by generated code.

> **Status: early development.** The API is incomplete and may change.

Currently it provides:

- `Sign`, the result type of every predicate;
- `eft`, error-free transformations: floating-point sums and products
  together with their exact rounding errors;
- `expansion`, exact values represented as sums of `f64` components, with
  exact addition, subtraction, scaling and sign, all without allocation;
- `Scratch`, a bump arena over a borrowed buffer that holds the expansions
  of a computation and runs those operations on them.

The optional `alloc` feature adds `Scratch::growable`, an arena that grows on
the heap so the exact arithmetic can always complete. Without it the crate
never allocates.

Most users should depend on `indirect-predicates` instead.

## License

Licensed under either of Apache License, Version 2.0 or MIT license, at your
option.
