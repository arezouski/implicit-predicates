# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `Sign`, the exact sign of a predicate: negative, zero or positive, ordered
  `Negative < Zero < Positive`, with conversions to `i8` and from
  `core::cmp::Ordering`.
- Error-free transformations (`two_sum`, `fast_two_sum`, `two_diff`,
  `two_product`, `two_square`) in `indirect-predicates-core`, using a hardware
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
- `Dyadic`, an exact dyadic rational number, in `indirect-predicates-oracle`
  for checking results.
