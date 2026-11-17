# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `Bpt`, in `implicit-predicates`: a point on a triangle, `u p + v q +
  (1 - u - v) r` for three explicit points `p`, `q` and `r` and parameters
  `u` and `v`, evaluated as `r + u (p - r) + v (q - r)`. `Bpt::new` rejects
  inputs that are not finite and parameters that do not satisfy `0 <= u`,
  `0 <= v` and `u + v <= 1`, the last one exactly, whatever the rounding of
  the sum; `Bpt::new_unchecked` accepts any `f64`. It has an accessor per
  parameter, keeps the error bound of each coordinate, which is zero on the
  coordinates that the three corners share, and implements `Point`, so every
  predicate takes it. `AnyPoint` has a variant `Bpt` and `From<Bpt>`.
- `dot_sign_3d` and `dot_sign_3d_in`, in `implicit-predicates`: the exact
  sign of `(p - q) . (r - q)`, over points of any type that implements
  `Point`. The apex of the angle is the middle argument; the sign is
  positive when the angle at `q` is acute, negative when it is obtuse, and
  zero when it is right or when `p` or `r` is `q`. A floating-point filter
  and a second test of it decide most calls; for explicit points, exact
  stages on the stack come next, and they decide when every nonzero
  coordinate has a magnitude in `2^-428..=2^478`, without using the arena of
  the thread; the others go to stages on the exact coordinates and, last, to
  a stage on big integers, so every call with finite inputs returns its
  exact sign. `dot_sign_3d` needs the `std` feature and uses the arena of
  the thread; `dot_sign_3d_in` takes the arena and works without `std`: 568
  `f64` always suffice for three explicit points, and it returns
  `ScratchExhausted` when the arena is too small. They panic on an input
  that is not finite and on a point whose exact coordinates have a weight
  other than `Weight::One`, like `orient3d`.
- `dihedral_sign` and `dihedral_sign_in`: the exact sign of
  `[(b - d) x (c - d)] . [(a - d) x (c - d)]`, the dot product of the normals
  of the triangles `a, c, d` and `b, c, d` that share the edge `c, d`. It is
  positive when the dihedral angle at that edge is acute, negative when it is
  obtuse, and zero when it is right or when a triangle is degenerate. For
  explicit points, a stage on the stack decides when every difference of
  coordinates it takes is exact in `f64` and every nonzero one has a
  magnitude in `2^-188..=2^239`, without using the arena of the thread;
  otherwise the call goes through the arena, to stages on the exact
  coordinates and, last, to a stage on big integers, so every call with
  finite inputs returns its exact sign. `dihedral_sign` needs `std` and uses
  the arena of the thread; `dihedral_sign_in` takes the arena and works
  without `std`: 2,233 `f64` always suffice for the stage on big integers on
  four explicit points, and points on a segment or a triangle need more. They
  panic in the same cases as `dot_sign_3d`.
- A `where` comparison on the sum of two parameters, `a + b <= k`, in the
  point declarations of `implicit-predicates-gen`, for two distinct
  parameters with coefficient one and an integer `k` of magnitude at most
  `2^53`. The emitted `new` checks it exactly, with the rounding error of the
  floating-point sum, instead of the generator reporting an error.
- Two targets of the fuzzing package, `fuzz`, outside the workspace and not
  published, for `dot_sign_3d` and `dihedral_sign`; its decoder also builds
  `Bpt` points, and `point_check` covers them.

### Changed

- **Breaking:** `AnyPoint`, in `implicit-predicates`, has a new variant,
  `Bpt`, and is `#[non_exhaustive]`: a `match` on it needs a wildcard arm.
  The `AnyPoint` that `implicit-predicates-gen` emits for any list of point
  declarations is `#[non_exhaustive]` too, so the same holds for the
  modules that other crates generate, whether or not they declare a new
  type: a `match` on their `AnyPoint` needs a wildcard arm.
- `implicit-predicates-gen` accepts the sum of two parameters in a `where`
  clause where it reported an error before; any other comparison that is not
  one parameter against an integer is still an error, and its message now
  names the accepted forms.

## [0.0.3] - 2026-11-15

### Added

- `insphere` and `insphere_in`, in `implicit-predicates`: the exact sign of
  the position of a point with respect to the sphere through four others,
  over points of any type that implements `Point`, with the convention of
  Shewchuk's `insphere`. A floating-point filter and a second test of it
  that scales its bound by the extent of each axis decide most calls; the
  others go to an exact stage on big integers, so every call with finite
  inputs returns its exact sign. `insphere` needs the `std` feature and uses
  the arena of the thread; `insphere_in` takes the arena and works without
  `std`: 6,835 `f64` always suffice for five explicit points, and it returns
  `ScratchExhausted` when the arena is too small. They panic on an input
  that is not finite and on a point whose exact coordinates have a weight
  other than `Weight::One`, like `orient3d`.
- `insphere_sos` and `insphere_sos_in`: `insphere` with its ties broken by a
  symbolic perturbation ordered by the indices of the five points, which
  must be distinct. The result is `Sign::Zero` exactly when the five points
  are coplanar. `insphere_sos` needs `std`; `insphere_sos_in` takes the
  arena.
- `check`, `CheckError` and `Violation`, in `implicit-predicates-core` and
  re-exported by `implicit-predicates`: `check` tests that a point type keeps
  the contract of `Point`, and reports the first obligation it breaks. It
  sees only what the views of a point show of it.
- A `cargo-fuzz` package, `fuzz`, outside the workspace and not published,
  with targets that compare the public predicates with the exact reference.
- Tests of `implicit-predicates-core` and `implicit-predicates` on
  `wasm32-wasip1`, run under wasmtime in CI, which exercise the guards for a
  32-bit `usize`.
- `ExactError::Weighted`, in `implicit-predicates-core`: the exact
  coordinates of a point have a weight other than one, which the big-integer
  coordinates of `Point::exact_big` cannot carry. `Point::exact_big` must
  return it for such a point, and the predicates turn it into the panic
  about the weight.
- `Big`, in `implicit-predicates-core`: exact integers of arbitrary size
  stored in a `Scratch` arena, built from `f64` values with
  `Scratch::big_from_f64`, with exact `big_add`, `big_sub` and `big_mul`;
  `Scratch::big_limb` reads one of its 64-bit words.
- `implicit-predicates` re-exports `Span` and `Mark`, so that a point type
  written by hand needs no other crate to name the types of the `Point`
  contract.
- `Generated::warnings`, in `implicit-predicates-gen`: what the user of the
  generated code should know and the code cannot enforce. The command-line
  tool prints them to standard error. There are three: no input at
  the edge of the error bound of a filter was found, so the table of edge
  cases of its checks is empty; a point type has an exact coordinate that
  can have more than 16 components: the stages on expansions decline it,
  and the stage on big integers, which is slower, decides it; and the error
  bound by parts of a filter was not derived, so its predicate has no
  `filter_axes`.
- `parts_bound`, in `implicit-predicates-gen`: the error bound by parts of
  the floating-point filter, with one size per axis for a form that is not
  multihomogeneous, such as the in-sphere determinant. It gives a
  `PartsOutcome`: a `PartsBound`, whose terms of epsilon (`EpsilonTerm`) and
  of kappa (`KappaTerm`) add up to the global bound when every size is the
  same, with its constants as `f64` (`PartsConstants`, `EpsilonConstant`,
  `KappaConstant`), or the reason it is not derived.
- A second test of the filter in the code generated by
  `implicit-predicates-gen` for a predicate whose main form is not
  multihomogeneous, such as the in-sphere determinant: `filter_axes`, with
  the error bound by parts, one size and one uncertainty per axis, and its
  constants `AXES_EPSILON`, `AXES_KAPPA`, `AXES_D_MIN` and `AXES_D_MAX`;
  and `filter_axes_points`, the same on points of any type. It runs right
  after `filter`, only when `filter` declines, and before every exact stage:
  in `staged`, and at the start of `exact_stages` (or of
  `exact_stages_explicit_in`, when there is no `exact_stages`) and of
  `exact_stages_in`. When the bound by parts is not derived, the test is not
  emitted and `Generated::warnings` says why.
- In the checks generated for a predicate with `filter_axes`,
  `axes_edge_cases`, pairs of inputs on either side of its bound by parts,
  each pair with its uncertainty: on grids with one scale for every axis,
  and on grids with one axis far shorter or longer than the others, with no
  uncertainty and with one on one axis or on all of them; and
  `check_axes_edge_cases`, which takes the uncertainty as a second argument
  of the stage, checks the sign at a sample of corners of the boxes of the
  errors, requires the stage to decline the first input of each pair with
  an uncertainty when the uncertainty is doubled (`2 e`) and to decide the
  second when it is halved (`e / 2`), and returns its counts with no
  uncertainty and with one.
- `Polynomial::is_affine_in_each_coordinate`.
- `Dyadic::from_words`, in `implicit-predicates-oracle`: builds a number
  from its sign, the 64-bit words of its magnitude and a binary exponent.

### Changed

- **Breaking:** `orient3d` returns `Sign` instead of `Option<Sign>`, and
  `orient3d_in` returns `Result<Sign, ScratchExhausted>` instead of
  `Result<Option<Sign>, ScratchExhausted>`: no finite input gives `None`
  any more, and an input that is not finite, which gave `None`, panics. So
  does a point whose exact coordinates have a weight other than
  `Weight::One`, which also gave `None`.
- **Breaking:** the generated `staged_in` and `exact_stages_in` return
  `Result<Sign, ScratchExhausted>` instead of
  `Result<Option<Sign>, ScratchExhausted>`, and generated modules use `Big`
  and `BigCoords` of `implicit-predicates-core`. Anyone who regenerates with
  `implicit-predicates-gen` is affected.
- **Breaking:** `Point`, in `implicit-predicates-core`, has a new required
  method, `exact_big`, which writes the exact coordinates as big integers
  (`BigCoords`) to a `Scratch` arena. `[f64; 3]` and the generated point
  types implement it; a point type of your own must too. `implicit-predicates`
  re-exports `Big` and `BigCoords`.
- **Breaking:** `ExactError` and `Weight`, in `implicit-predicates-core`,
  and `PointError` are `#[non_exhaustive]`: a `match` on them needs an arm
  for the variants that may be added.
- The exact stage over arena spans declines a coordinate with a zero
  component, which no valid expansion has, instead of evaluating it.
- A predicate whose exact stages on the stack would be too large to plan is
  generated with the stage over arena spans alone, instead of being
  rejected.
- The generated `check_uncertainty_edge_cases` says that the sign at the
  corners of the boxes of the errors is the sign in the whole boxes only
  for a predicate that is affine in each coordinate; for any other it says
  that the corners do not show it.
- The exact stages evaluate the main form only. For a predicate of degree
  4 or less, the chain for explicit points is the filter, the exact
  differences, the expansions on the differences and the stage on big
  integers; for points of any type it is the filter, the expansions on the
  differences over arena spans and the stage on big integers; in both,
  `filter_axes` follows the filter when the predicate has it. A call that
  the expansions on the differences decline goes to the stage on big
  integers.
- So, in release builds, no call of `orient3d` on explicit points whose
  nonzero coordinates have magnitudes in `2^-188..=2^238` allocates,
  instead of `2^-188..=2^239`: with a coordinate above `2^238`, such a call
  goes to the stage on big integers, in the arena of the thread.
- The `#[exact]` form of a definition is only checked to be the same
  polynomial as the main form; no stage evaluates it, and no plan of an
  exact stage is derived from it. So `generate` no longer rejects an
  `#[exact]` form for what such a plan required of it, such as a constant
  that is not exact in `f64`.
- **Breaking:** a predicate whose main form has a total degree of 5 or more
  (`BIG_INTEGERS_ONLY_DEGREE`, new in `implicit-predicates-gen`) uses the
  stage on big integers as its only exact stage: its chain is the filter
  (with `filter_axes`, when the predicate has one) and the stage on big
  integers, for points of any type. The generator plans no stage on
  expansions for it and emits none: no `exact_differences`,
  `expansions_translated` or `expansions_translated_in`, no
  `exact_stages`, no `DIFFERENCES_RANGE`, `TRANSLATED_RANGE`,
  `SPANS_TRANSLATED_RANGE` or `MAX_COORD_COMPONENTS`, and no helpers that
  only they use. Its `staged` is the filter alone (`filter`, then
  `filter_axes` when it has one) and takes no arena, and
  `exact_stages_explicit_in` and `exact_stages_in` run `filter_axes` or
  `filter_axes_points`, when it has them, and then `big_integers_in`; the
  chain never calls `Point::exact`, only `Point::exact_big`. Anyone who
  regenerates such a predicate loses those emitted items, and its `staged`
  no longer takes an arena. A definition of such a degree that the plan of
  the stage over spans rejected, such as one with a product operand of
  degree 17, is now generated. Predicates of degree 4 or less keep their
  chain.
- In debug builds, a point that breaks the contract of `Point` now panics
  with a message that begins with ``breaks the contract of `Point`: ``, in
  `orient3d`, `insphere`, `insphere_sos` and their `_in` variants, and in the
  generated chains. Every call checks that no error bound of a point is NaN
  or negative and that a point that says it is explicit has no error; when
  the filter does not decide, the chains also check that the exact
  coordinates a point writes to the arena are valid, without using room in
  it; and `orient3d`, `insphere` and `insphere_sos`, which take no arena,
  also run `check` on each point when a call reaches the exact stages, in an
  arena of their own, which they allocate: in debug builds, a call of
  `orient3d` on explicit points that the filter does not decide allocates.
  An input that is not finite and a weight other than one keep the panics
  of the predicates. Release builds are unchanged: they
  check nothing, and a point that breaks the contract can still give a wrong
  sign.
- The feature `std` of `implicit-predicates` provides `insphere` and
  `insphere_sos` as well as `orient3d`: without it, `insphere_in`,
  `insphere_sos_in` and `orient3d_in` are the entry points.
- `orient3d` and `orient3d_in` end with an exact stage on big integers, so
  they decide every finite input, whatever the magnitude of the
  coordinates. With explicit points outside the range of the stages on the
  stack, `orient3d` uses the arena of the thread and `orient3d_in` the arena
  it is given, 1,456 `f64` of which always suffice.
- Documented panics of `orient3d` and `orient3d_in`: on an input that is not
  finite, and on a point whose exact coordinates have a weight other than
  `Weight::One`, which the predicates do not support.
- The generated chain for points of any type, `staged_in`, runs the stage on
  big integers last, through `exact_stages_explicit_in` for explicit points
  and `exact_stages_in` for the others.

### Removed

- **Breaking:** the generated stages on the `#[exact]` form,
  `expansions_untranslated` and `expansions_untranslated_in`, and their
  ranges `UNTRANSLATED_RANGE` and `SPANS_UNTRANSLATED_RANGE`. They took
  longer than the stages on the differences, and decided no recorded call
  that those decline.
- **Breaking:** `exact_plan_raw` and `exact_plan_spans_untranslated`, in
  `implicit-predicates-gen`, which derived the ranges of those stages.

### Fixed

- `implicit-predicates-gen` rejects a point type named `Big` or `BigCoords`,
  which the generated modules import from `implicit-predicates-core`, instead
  of emitting code that does not compile.
- The stages generated for a predicate without stages on the stack no
  longer define a helper that nothing calls.

## [0.0.2] - 2026-11-11

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
- `orient3d` needs the `std` feature, on by default. This is a breaking
  change for crates that depend on `implicit-predicates` with
  `default-features = false`: `orient3d` is gone for them, and
  `orient3d_in`, which takes the arena, is the entry point without `std`.
  On explicit points it still does not allocate; with an
  implicit point that the filter cannot decide, the arena of the thread is
  allocated on its first use and grows on the heap when a call needs more.
- With implicit points, `orient3d` also returns `None` when the exact
  coordinates of a point cannot be computed in the range of its exact
  arithmetic, or when, no stage being able to decide, a nonzero component
  of those exact coordinates lies outside `2^-188..=2^235` or a coordinate
  has more than 16 components, or the exact coordinates of a point of
  another crate have a weight other than `Weight::One`. The documentation
  of `orient3d` gives a condition on the inputs of an `Lnc` under which the
  call is decided. For explicit points the conditions are unchanged.
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

[Unreleased]: https://github.com/arezouski/implicit-predicates/compare/v0.0.3...HEAD
[0.0.3]: https://github.com/arezouski/implicit-predicates/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.com/arezouski/implicit-predicates/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/arezouski/implicit-predicates/releases/tag/v0.0.1
