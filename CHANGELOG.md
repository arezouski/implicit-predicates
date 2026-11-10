# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `Sign`, the exact sign of a predicate: negative, zero or positive.
- Error-free transformations (`two_sum`, `fast_two_sum`, `two_diff`,
  `two_product`, `two_square`) in `indirect-predicates-core`, using a hardware
  fused multiply-add for products when the target has one.
- Floating-point expansion primitives (`from_f64`, `from_two_sum`,
  `from_two_diff`, `from_two_product`, `grow`, `sum`, `diff`, `scale`,
  `negate`, `sign`, `estimate`) for exact arithmetic on `f64` values.
- The `Scratch` arena for expansion results, borrowed from a caller-provided
  buffer, with an optional growable form behind the `alloc` feature.
- `Dyadic`, an exact dyadic rational number, in `indirect-predicates-oracle`
  for checking results.
