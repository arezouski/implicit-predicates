# implicit-predicates

Exact geometric predicates in Rust for explicit points and for *implicit*
points: points defined by an unevaluated construction, such as a point on an
edge or inside a triangle.

> **Status: early development.** Today the crate provides `orient3d` for
> explicit points and points on a line; the API may change. `insphere` does
> not exist yet.

## Example

```rust
use implicit_predicates::{Lnc, Sign, orient3d};

let (a, b, c) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
// a, b and c appear counter-clockwise from (1, 1, 1), above the plane.
assert_eq!(orient3d(&a, &b, &c, &[1.0, 1.0, 1.0]), Some(Sign::Negative));
assert_eq!(orient3d(&a, &b, &c, &[0.0, 0.0, 0.0]), Some(Sign::Positive));
// A point on the plane.
assert_eq!(orient3d(&a, &b, &c, &[0.5, 0.25, 0.25]), Some(Sign::Zero));
// A point on the edge between b and c, b + 0.3 (c - b): on the plane,
// exactly, although its coordinates are not representable.
let m = Lnc::new(c, b, 0.3).unwrap();
assert_eq!(orient3d(&a, &b, &c, &m), Some(Sign::Zero));
```

`orient3d` takes four points of any type that implements `Point`, and
returns the exact sign of the determinant of `a - d`, `b - d`, `c - d`
(Shewchuk's convention) at their exact coordinates. The point types are
`[f64; 3]` (an explicit point, not validated), `Explicit` (a validated
explicit point), `Lnc { p, q, t }`, the point `q + t (p - q)` on the line
through two explicit points, and `AnyPoint`, either of the last two.

A floating-point filter on the approximations of the points decides most
calls. When it cannot, explicit points go to exact stages on floating-point
expansions held on the stack, so no call on explicit points allocates. With
a point on a line, the exact coordinates of the points go to an arena of the
calling thread, 16,384 `f64` to start with, that grows on the heap when a
call needs more. `orient3d_in` takes the arena instead. Without its default
`std` feature the crate is `no_std`, and `orient3d_in` takes the place of
`orient3d`.

The result is an `Option<Sign>`, provisionally. It is `None` when an input of
a point is not finite. Otherwise, only if the earlier stages cannot decide
either: for explicit points, when a nonzero coordinate has a magnitude
outside `2^-188..=2^239` (about `2.5e-57` to `8.8e71`); with a point on a
line, when the inputs of a point are beyond the range of its own exact
arithmetic, or a nonzero component of the exact coordinates has a magnitude
outside `2^-188..=2^235`. A later version will decide every finite input.

## Goals

- Exact sign for every finite input (not yet reached by `orient3d`: see
  above).
- Speed close to classical filtered predicates in the common case.
- Explicit points and implicit points through one API.
- New predicates and point types through a code generator.

Planned for the first release: `orient3d`, `insphere` and a symbolically
perturbed `insphere`, over explicit points and points on a segment. Only
`orient3d` exists today.

## Minimum supported Rust version

Rust 1.86.

## License

Licensed under either of Apache License, Version 2.0 or MIT license, at your
option.
