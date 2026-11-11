# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.1] - 2026-11-11

First release: early development, published so that the crates can be
tried. The API is incomplete and will change.

### Added

- `Sign`, the exact sign of a predicate: negative, zero or positive, ordered
  `Negative < Zero < Positive`, with conversions to `i8` and from
  `core::cmp::Ordering`.
- Error-free transformations (`two_sum`, `fast_two_sum`, `two_diff`,
  `two_product`, `two_square`) in `implicit-predicates-core`, using a hardware
  fused multiply-add for products when the target has one. Both product
  algorithms are available directly as `two_product_dekker` and
  `two_product_fma`, and `HAS_FMA` tells which one `two_product` uses.
- The product domain of the exact products, `eft::in_product_domain` with
  its bounds `PRODUCT_OPERAND_MIN` and `PRODUCT_OPERAND_MAX` (`2^-480` and
  `2^480`). Debug builds panic on operands outside it.
- Floating-point expansion primitives (`from_f64`, `from_two_sum`,
  `from_two_diff`, `from_two_product`, `grow`, `sum`, `diff`, `scale`,
  `negate`, `sign`, `estimate`) for exact arithmetic on `f64` values. Debug
  builds check their finiteness and domain requirements.
- The `Scratch` arena for expansion results, borrowed from a caller-provided
  buffer, with an optional growable form behind the `alloc` feature, and the
  exact product of two expansions, `Scratch::product`. Results are `Span`s,
  `Mark`s save the top of the arena, and operations that do not fit return
  `ScratchExhausted`. Debug builds check that `Scratch::push` receives a
  valid expansion.
- `Dyadic`, an exact dyadic rational number, in `implicit-predicates-oracle`
  for checking results.
- `implicit-predicates-gen`, a code generator for predicates, with a command
  line tool. A definition is a `.pred` file in a small subset of Rust: `let`
  bindings and a final expression built from `+`, `-`, `*`, integer
  constants, coordinates `p.x`, `p.y`, `p.z` and earlier bindings, with an
  optional second form marked `#[exact]`. The generator checks that both
  forms are the same nonzero polynomial and that the main form is homogeneous
  in differences of coordinates. It derives, in exact arithmetic, the
  constants of the floating-point filter, the ranges of input magnitudes of
  the exact stages and the exponent ranges and buffer sizes they need. It
  emits, for each predicate, a module of stages (filter, exact differences,
  translated expansions, and the `#[exact]` form) and a module of checks for
  tests, formatted with `rustfmt`. `--check` compares the files with what
  would be written without writing them. Errors carry the file, line and
  column.
- `orient3d` for explicit points, in `implicit-predicates`, generated from
  its definition: a floating-point filter, then exact differences of
  coordinates, then expansions on translated coordinates, then expansions on
  the raw coordinates. The sign is that of Shewchuk's `orient3d`. It takes
  `&[f64; 3]` points, does not allocate and works without `std`. The result
  is provisionally an `Option<Sign>`: `None` when a coordinate is not finite
  or, if the earlier stages cannot decide, when a nonzero coordinate is
  outside `2^-188..=2^239`. `implicit_predicates::Sign` re-exports the `Sign`
  of `implicit-predicates-core`.
