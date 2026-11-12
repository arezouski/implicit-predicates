# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Implicit points. `Point`, in `implicit-predicates-core`, is the contract
  of a point as the predicates read it: an approximation of its
  coordinates, a bound on the error of each one, whether it is explicit,
  and its exact coordinates written as expansions to a `Scratch` arena, in
  homogeneous form (`ExactCoords`, with a `Weight`), or an `ExactError`.
  `[f64; 3]` implements it as an explicit point. The predicates rely on the
  contract without checking it: an implementation that breaks it gives
  wrong signs.
- Point types in `implicit-predicates`, generated from their declarations:
  `Explicit`, a validated explicit point; `Lnc`, the point `q + t (p - q)`
  on the segment between two explicit points, with its coordinates computed
  in `f64` and an error bound for each one that is zero on the coordinates
  that are exact; `AnyPoint`, either of them; and `PointError`, the error of
  their constructors. `new` rejects inputs that are not finite and, for
  `Lnc`, a `t` outside `0..=1`; `new_unchecked` accepts any `f64`.
- `orient3d_in`, `orient3d` with a caller-provided arena for the exact
  coordinates of implicit points. It works without `std` and returns
  `ScratchExhausted` when the arena is too small.
- The `std` feature of `implicit-predicates`, on by default, which provides
  the arena of each thread used by `orient3d`.
- `Scratch::write`, to write an expansion computed by other code into the
  arena, and the `filter` module of `implicit-predicates-core`, which turns
  the errors of the points of a call into the uncertainty of a filter.
- Point declarations in `implicit-predicates-gen`: `point NAME(params) =
  expr;`, with `F64x3` and `f64` parameters and an optional `where` clause
  on the parameters, read by
  `parse_points` into a `PointType`. `generate_points` emits the module of
  the point types of a file, with their constructors, the error bound of
  their coordinates, derived by `error_bound`, and their exact coordinates.
  The command line tool writes it as `points.rs`, next to the predicates and
  their checks; `--check` covers it as well.
- A floating-point filter with one size and one uncertainty per axis, chosen
  by `filter_form` when every term of a predicate has the same degree in
  each axis, as for `orient3d` (`AxisBound`, `AxisConstants`,
  `FilterForm`), and exact stages over spans of an arena for points of any
  type, with their ranges derived by `exact_plan_spans_translated` and
  `exact_plan_spans_untranslated`.

### Changed

- `orient3d` takes any `&impl Point` instead of `&[f64; 3]`, so it accepts
  `[f64; 3]`, `Explicit`, `Lnc`, `AnyPoint` and the types of other crates
  that implement `Point`. This is a breaking change. Calls with `&[f64; 3]`
  arguments keep compiling and return the same signs, and `orient3d` still
  coerces to the function pointer type
  `fn(&[f64; 3], &[f64; 3], &[f64; 3], &[f64; 3]) -> Option<Sign>`. Arguments
  that reached `&[f64; 3]` through a deref coercion, such as `&&[f64; 3]` or
  `&Box<[f64; 3]>`, no longer compile: pass the reference itself (`r`
  rather than `&r`) or dereference the box (`&*b`). So do arguments whose
  type was inferred from the parameter, such as
  `&slice.try_into().unwrap()`: annotate them as `[f64; 3]`. The generic
  parameters of `orient3d` cannot be named, since they are `impl Point`
  arguments.
- `orient3d` needs the `std` feature, on by default. Without it, call
  `orient3d_in`. On explicit points it still does not allocate; with an
  implicit point that the filter cannot decide, the arena of the thread is
  allocated on its first use and grows on the heap when a call needs more.
- With implicit points, `orient3d` also returns `None` when the exact
  coordinates of a point cannot be computed in the range of its exact
  arithmetic, or when, no stage being able to decide, a nonzero component
  of those exact coordinates lies outside `2^-188..=2^235` or a coordinate
  has more than 16 components. For explicit points the conditions are
  unchanged.
- In the generated code, the filter of a predicate with a per-axis bound
  takes the uncertainty of the points as `e: [f64; 3]`, and each predicate
  module also holds `filter_points`, `exact_stages`, `exact_stages_in` and
  `staged_in`.

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
