# indirect-predicates

Exact geometric predicates in Rust for explicit points and for *implicit*
points: points defined by an unevaluated construction, such as a point on an
edge or inside a triangle.

> **Status: early development.** This crate is a placeholder and has no usable
> API yet.

## Goals

- Exact sign for every finite input.
- Speed close to classical filtered predicates in the common case.
- Explicit points and implicit points through one API.
- New predicates and point types through a code generator.

Planned for the first release: `orient3d`, `insphere` and a symbolically
perturbed `insphere`, over explicit points and points on a segment.

## Minimum supported Rust version

Rust 1.86.

## License

Licensed under either of Apache License, Version 2.0 or MIT license, at your
option.
