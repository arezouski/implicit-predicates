# indirect-predicates

Exact geometric predicates in Rust for explicit points and for *implicit*
points: points defined by an unevaluated construction, such as a point on an
edge or inside a triangle.

> **Status: early development.** Today the crate provides `orient3d` for
> explicit points; the API may change. Implicit points and `insphere` do not
> exist yet.

## Example

```rust
use indirect_predicates::{Sign, orient3d};

let (a, b, c) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
// a, b and c appear counter-clockwise from (1, 1, 1), above the plane.
assert_eq!(orient3d(&a, &b, &c, &[1.0, 1.0, 1.0]), Some(Sign::Negative));
assert_eq!(orient3d(&a, &b, &c, &[0.0, 0.0, 0.0]), Some(Sign::Positive));
// A point on the plane.
assert_eq!(orient3d(&a, &b, &c, &[0.5, 0.25, 0.25]), Some(Sign::Zero));
```

`orient3d` takes `&[f64; 3]` points and returns the exact sign of the
determinant of `a - d`, `b - d`, `c - d` (Shewchuk's convention). A
floating-point filter decides most calls; the rest go to exact stages on
floating-point expansions held on the stack, so no call allocates. The crate
is `no_std` without its default `std` feature.

The result is an `Option<Sign>`, provisionally: it is `None` when a
coordinate is not finite or, only if the earlier stages cannot decide either,
when a nonzero coordinate has a magnitude outside `2^-188..=2^239` (about
`2.5e-57` to `8.8e71`). A later version will decide every finite input.

## Goals

- Exact sign for every finite input (not yet reached by `orient3d`: see
  above).
- Speed close to classical filtered predicates in the common case.
- Explicit points and implicit points through one API.
- New predicates and point types through a code generator.

Planned for the first release: `orient3d`, `insphere` and a symbolically
perturbed `insphere`, over explicit points and points on a segment. Only
`orient3d` over explicit points exists today.

## Minimum supported Rust version

Rust 1.86.

## License

Licensed under either of Apache License, Version 2.0 or MIT license, at your
option.
