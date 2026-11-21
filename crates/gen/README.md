# implicit-predicates-gen

[![crates.io](https://img.shields.io/crates/v/implicit-predicates-gen.svg)](https://crates.io/crates/implicit-predicates-gen)
[![docs.rs](https://img.shields.io/docsrs/implicit-predicates-gen)](https://docs.rs/implicit-predicates-gen)
[![CI](https://github.com/arezouski/implicit-predicates/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/arezouski/implicit-predicates/actions/workflows/ci.yml)

Code generator for
[`implicit-predicates`](https://crates.io/crates/implicit-predicates): turns
the definition of a predicate into specialized Rust code.

> **Status: early development.** The generator handles predicates over
> points of any type that implements the `Point` contract of
> `implicit-predicates-core`, and emits the point types declared next to
> them; its API may change.

## Definitions

A predicate is defined in a `.pred` file, in a small subset of Rust:

```text
// Orientation of d with respect to the plane through a, b and c.
fn orient3d(a: Point3, b: Point3, c: Point3, d: Point3) -> Sign {
    let ax = a.x - d.x;  let ay = a.y - d.y;  let az = a.z - d.z;
    let bx = b.x - d.x;  let by = b.y - d.y;  let bz = b.z - d.z;
    let cx = c.x - d.x;  let cy = c.y - d.y;  let cz = c.z - d.z;
    ax*(by*cz - bz*cy) - ay*(bx*cz - bz*cx) + az*(bx*cy - by*cx)
}

#[exact]
fn orient3d(a: Point3, b: Point3, c: Point3, d: Point3) -> Sign {
    // The same polynomial on the raw coordinates.
    ...
}
```

The body holds `let` bindings and a final expression built from `+`, `-`
(binary and unary), `*`, parentheses, integer literals, the coordinates
`p.x`, `p.y`, `p.z` of a parameter and the names of earlier bindings. The
predicate is the sign of that polynomial.

The main form must be homogeneous in differences of coordinates: built from
differences of one coordinate of two points, combined by sums of terms of
equal degree and by products. An optional second form, marked `#[exact]`,
writes the same polynomial in another way; it may use the raw coordinates
and integer constants. It is a check at generation time only: the generator
requires it to be the same polynomial as the main form, and no emitted
stage evaluates it, so every stage, exact ones included, works on the main
form.

## Point types

The same files declare the types of points a predicate can take:

```text
point Explicit(p: F64x3) = p;

point Lnc(p: F64x3, q: F64x3, t: f64) = q + t*(p - q)
    where 0 <= t, t <= 1;
```

A declaration reads:

```text
declaration := 'point' NAME '(' params ')' '=' expr ('where' bounds)? ';'
params      := (param (',' param)* ','?)?
param       := NAME ':' ('F64x3' | 'f64')
expr        := term (('+' | '-') term)*
term        := unary ('*' unary)*
unary       := '-'* atom
atom        := INTEGER | NAME | '(' expr ')'
bounds      := bound (',' bound)*
bound       := sum '<=' sum
sum         := (NAME | INTEGER) ('+' (NAME | INTEGER))*
```

A parameter is an `F64x3` (three `f64` coordinates) or an `f64`. In the
expression, a `NAME` is a parameter, used whole: the coordinates of an
`F64x3` cannot be read one by one. `INTEGER` is a plain decimal integer
literal that fits in an `i64` and is exactly an `f64`. Operations on an
`F64x3` are per coordinate: a sum or a difference combines two values of the
same type, a product combines an `f64` with an `F64x3` (in either order) or
two `f64`, and the value of the whole expression must be an `F64x3`. Every
parameter must be used by the expression. Brackets nest at most 32 deep.

The optional `where` clause lists what the parameters must satisfy for the
point to exist. Its comparisons are `<=` only, between sums of `f64`
parameters and integers; `-`, `*` and `F64x3` parameters are refused there,
and so is a comparison that names no parameter. A comparison must also
reduce, once the parameters on both sides cancel, to one of two forms, so
that it is checked exactly, and `generate_points` reports any other as an
error:

- one parameter against an integer of magnitude at most `2^53`, such as
  `0 <= t` or `t <= 1`;
- the sum of two distinct parameters, each with coefficient one, at most an
  integer of magnitude at most `2^53`, such as `u + v <= 1`. The sum may
  name them in either order. A sum at least an integer (`1 <= u + v`), a
  sum of three parameters, a parameter against another (`u <= v`) and a
  parameter counted twice (`u + u <= 1`) are errors.

The emitted `new` checks a sum with the exact rounding error of the
floating-point sum of the two parameters, so that no rounding admits a pair
whose exact sum is above the integer.

Names:

- the names of points and parameters cannot be Rust keywords, `_` or raw
  identifiers; two points cannot share a name, nor two parameters of one
  point;
- the name of a point type must start with an uppercase ASCII letter, and
  cannot be a name that the generated modules use: `AnyPoint`, `Big`,
  `BigCoords`, `PointError`,
  `Point`, `Scratch`, `ScratchExhausted`, `ExactCoords`, `ExactError`,
  `Weight`, `Mark`, `Span`, `Dyadic`, `UNIT`, `TINY`, `SUM_OPERAND_LIMIT`,
  `Result`, `Ok`, `Err`, `Option`, `Some`, `None` and `From`; two point
  types cannot have names whose snake case is the same, since each is
  checked by `exact_<snake case of the name>`;
- the name of a parameter must start with a lowercase ASCII letter, so that
  it cannot be a type, a constant or a variant that the generated code uses
  (such as `Ok`, `None`, `UNIT` or `ERR_SLACK`); it must be in snake case,
  lowercase ASCII letters, digits and underscores without two underscores in
  a row before the end (`t_a` and `t1`, not `tA` or `t__a`), so that the
  generated code compiles without warnings; and it cannot be named like a
  field or a function of the generated type: `approx`, `err`, `exact`,
  `is_explicit`, `new`, `new_unchecked`, `evaluate` and `write_exact`.

A point type is explicit when it has one `F64x3` parameter and returns it as
is, like `Explicit` above; every other type is implicit. `generate_points`
also refuses an implicit type whose error bound needs more than
`MAX_ROUNDS` (1,024) roundings on a chain of operations, or whose exact
coordinates need temporaries of more than 4,096 `f64`, and any type whose
coordinates take more than `MAX_POINT_NODES` (1,024) distinct operations
and inputs, counted once over the three coordinates, which bounds the size
of the emitted code. Every error is reported as `file:line:column:
message`.

A type with an exact coordinate that can have more than
`MAX_COORD_COMPONENTS` (16) components is generated, with a warning: the
exact stages on expansions of a predicate decline a call with a coordinate
that has more.

`parse_points` reads the declarations of a file; `parse` reads its
predicates and checks the declarations without returning them (a file of
only points gives no predicates, not an error). Each coordinate of a point
is held in one graph of `f64` operations in which equal subexpressions share
a node, and the clause as a list of comparisons, so that they can be
evaluated and emitted operation by operation.

`generate_points` emits the module `points` of the point types of a file:

- `PointError`, the error of the constructors;
- for each type, a struct with `new`, which rejects inputs that are not
  finite and parameters outside the `where` clause, `new_unchecked`, which
  accepts anything, and an implementation of the `Point` trait of
  `implicit-predicates-core`. An explicit type (one `F64x3` returned as is)
  wraps `[f64; 3]`. Every other type also has one accessor per parameter and
  stores the coordinates computed in `f64` with a bound on the error of
  each one; `Point::exact` writes its exact coordinates as expansions;
- `AnyPoint`, an enum with one variant per type, marked `#[non_exhaustive]`,
  which implements `Point` too.

The error bound is computed operation by operation, by a rule that
`error_bound` documents together with the proof that the result is an upper
bound: a sum adds the errors of its operands and `2^-53` times its value, a
product propagates the errors of its factors and adds `2^-53` times its
value plus `2^-1074`, each rounding term only when no operand is zero, and
a slack factor derived from the number of roundings of the computation
covers the rounding of the computation itself. A coordinate that is exact,
such as one shared by the parents of a point on a line, gets an error of
exactly zero.

The checks module of the point types holds `exact_<name>`, the exact
coordinates of each implicit type, in snake case, evaluated with the exact
numbers of `implicit-predicates-oracle`.

## What the generator checks

Before writing anything, the generator expands both forms, when there is an
`#[exact]` one, and requires them to be the same nonzero polynomial, checks
that the main form is homogeneous in the differences, and derives, in exact
arithmetic:

- the error bound of the floating-point filter and the range of scales in
  which overflow and underflow cannot invalidate it, and, for a main form
  that is not multihomogeneous, the error bound by parts of its second
  test, with one size per axis, and its range. That bound has a budget of
  its own, on the number of its terms and on the exact work: beyond it, the
  second test is not emitted and the generation carries a warning, not an
  error;
- for each exact stage on expansions, the range of input magnitudes for
  which every exact product of the arithmetic core stays in its domain and
  no value overflows, and the worst-case length of every temporary. A main
  form of total degree 5 or more (`BIG_INTEGERS_ONLY_DEGREE`) has no stage
  on expansions, so nothing of this is derived for it: its only exact
  stage is the one on big integers, which has no range.

A definition that fails any of these is reported as `file:line:column:
message`.

## Generated code

For each predicate the generator writes one module with:

- the constants of the filter and the ranges of the exact stages
  (`DIFFERENCES_RANGE`, `TRANSLATED_RANGE` and `SPANS_TRANSLATED_RANGE`);
- `filter`, the main form evaluated in `f64` with its error bound. When
  every term of the main form has the same degree in the differences of
  each axis, as for an orientation, the bound has one size and one
  uncertainty per axis: the constants are `DEGREES`, `EPSILON`, `KAPPA` (one
  per axis), `D_MIN` and `D_MAX`, the filter takes `e: [f64; 3]`, and it
  returns `Some(Sign::Zero)` when some axis is flat and every term has
  positive degree in it. Otherwise the bound is global: the constants are
  `DEGREE`, `EPSILON`, `KAPPA`, `D_MIN` and `D_MAX`, and the filter takes
  `e: f64`;
- for a filter with a global bound, `filter_axes`, its second test, which
  runs only when `filter` declines: the main form evaluated in `f64` again,
  with the error bound by parts, one size and one uncertainty per axis, a
  sum of one term of epsilon per degree vector of the main form and of
  terms of kappa, each a chain of products that starts with its constant.
  It decides calls whose points spread much less on one axis than on
  another, which the global bound, sized by the largest axis, declines. Its
  constants are `AXES_EPSILON` (degree vector and coefficient of each term),
  `AXES_KAPPA` (axis, degree vector of the rest and coefficient),
  `AXES_D_MIN` and `AXES_D_MAX`; it takes `e: [f64; 3]`, and returns
  `Some(Sign::Zero)` when some axis is flat and every degree vector has
  positive degree in every axis;
- `exact_differences` and `expansions_translated`: exact evaluations of
  the main form with floating-point expansions on stack buffers, on the
  differences of coordinates computed in `f64`, which must be exact, and
  on each difference as the exact expansion of two components. A stage
  whose buffers would take more than 4,096 `f64`, or too many to plan, is
  not emitted, and the stage over arena spans takes its place;
- `staged`, which runs the filter, then `filter_axes` if there is one, and
  then the exact stages in the order: differences that are exact, then the
  translated expansion form. The order is the same for every predicate that
  has them;
- for a main form of total degree 5 or more, none of the stages on
  expansions above, nor their ranges, `MAX_COORD_COMPONENTS`,
  `exact_stages` or `expansions_translated_in`: its `staged` is the filter
  alone (with `filter_axes`, if it has one), without an arena, and its
  `exact_stages_explicit_in` and `exact_stages_in` below run
  `big_integers_in` alone, after `filter_axes` or `filter_axes_points` when
  it has them, so its chain never asks a point for its exact coordinates
  as expansions;
- for points of any type: `filter_points`, the filter on the approximations
  of the points with the uncertainty of their errors (zero when every point
  is explicit); `filter_axes_points`, the same for `filter_axes`, with the
  uncertainty per axis; `exact_stages`, the stages of `staged` after
  `filter`; `expansions_translated_in`, the expansion form over spans of an
  arena, on the exact coordinates of the points; `exact_stages_in`, which
  runs `expansions_translated_in`, when the predicate has it, and then
  `big_integers_in`; `exact_stages_explicit_in`, which runs `exact_stages`,
  when the predicate has it, and then `big_integers_in`, for points that
  are all explicit; and `staged_in`, which runs `filter_points`, then
  `exact_stages_explicit_in` when every point is explicit and
  `exact_stages_in` otherwise. When there is `filter_axes`, `exact_stages`
  (or `exact_stages_explicit_in`, when there is no `exact_stages`) starts
  with it and `exact_stages_in` with `filter_axes_points`, so that the
  uncertainty per axis is computed only when `filter` declines. So, for a
  main form of a lower degree than `BIG_INTEGERS_ONLY_DEGREE`, the chain of
  `staged_in` is `filter_points`, `filter_axes_points` if there is one,
  `expansions_translated_in` and `big_integers_in`, and on explicit points
  it is `filter`, `filter_axes` if there is one, the stages of
  `exact_stages` and `big_integers_in`; for the others, it is the two tests
  of the filter, or the first alone, and `big_integers_in`. `staged_in` and
  `exact_stages_in` return `Result<Sign, ScratchExhausted>`: they decide
  every input, and fail only when the arena runs out of room. They panic on
  a point whose exact coordinates have a weight other than one, and on a
  point that cannot give its coordinates as big integers because an input
  is not finite or its weight is other than one. In debug builds the
  chain also asserts the obligations of `Point` that need no room in the
  arena: an error bound that is not NaN or negative, no error on a point
  that says it is explicit, and exact coordinates that are valid expansions
  or big integers in their unique representation, below the top of the
  arena. A point that breaks one panics with a message that begins with
  ``breaks the contract of `Point`:``; release builds check none of it;
- `big_integers_in`, the main form evaluated exactly with the big-integer
  operations of an arena, on the coordinates of the points as big
  integers; it always decides, and fails only when the arena runs out of
  room. `BIG_INTEGERS_EXPLICIT_ARENA` is a number of `f64` slots of the
  arena, one 64-bit word each, that suffices for it on explicit points,
  their coordinates as big integers included: a conservative bound, not a
  tight one. Both chains end with this stage.

Each stage returns `None` when it cannot decide, except `big_integers_in`.

For the tests of each predicate it writes a second module, of checks, with:

- `exact_at` and `exact`, the main form evaluated with the exact dyadic
  numbers of `implicit-predicates-oracle` at exact or at `f64` coordinates,
  the reference the stages are tested against;
- `bound_edge_cases`, pairs of inputs on either side of the error bound of
  the filter for explicit points, one representable value apart in one
  coordinate, built with exact arithmetic from points on a grid at scales
  from just above `D_MIN` to just below `D_MAX`, and written as bit
  patterns;
- for a filter with one uncertainty per axis, `uncertainty_edge_cases`,
  pairs built in the same way on either side of its bound with an
  uncertainty that is not zero on one axis, for each axis of positive
  degree, or on all of the axes of positive degree, given with each pair;
- for a predicate with `filter_axes`, `axes_edge_cases`, pairs built in the
  same way on either side of its bound by parts, each with its uncertainty,
  at scales from just above `AXES_D_MIN` to just below `AXES_D_MAX`: with no
  uncertainty on grids of one scale, then on grids with a scale of their
  own on each axis, one axis `2^-20` or `2^-40` times as short or as long as
  the other two, with no uncertainty and with one on each axis of the range
  or on all of them. In a pair with an uncertainty the terms of kappa are
  more than `2^10` times the terms of epsilon;
- `check_bound_edge_cases`, `check_axes_edge_cases` and
  `check_uncertainty_edge_cases`, which run a
  stage, given as a closure of the coordinates of the points (and of their
  errors, or of the uncertainty for `check_axes_edge_cases`), on those
  cases and check its signs, so that the tests can run them on every stage
  and on points of any type. `check_axes_edge_cases` also requires the
  stage to decline the first input of each pair with an uncertainty when
  the uncertainty is doubled, and to decide the second when it is halved,
  so that a term of kappa that is much too small or much too large fails
  even when no sign is wrong. With an uncertainty, the
  sign is checked at every corner of the boxes of the errors for a
  predicate of at most four points; above that the corners are too many,
  and only a fixed sample of `2 * (1 + 3 * points)` of them is checked, as
  the documentation of the emitted function says. The corners show that the
  sign holds in the whole boxes only when the predicate is affine in each
  coordinate, like a determinant of differences; the documentation of the
  emitted function says which is the case.

The search can find no edge case: for a product of one difference per axis,
the filter decides every input in its range. The table is then empty, its
documentation says so, and the result of the generation carries a warning
(`Generated::warnings`), which the command-line tool prints to standard
error.

Generated files start with a header saying that they must not be edited
and giving the command that regenerates them.

## Command line

```sh
implicit-predicates-gen <pred-dir> --stages <dir> --checks <dir> [--check]
```

reads every `.pred` file in `<pred-dir>` and writes `<name>.rs` for each
predicate to the `--stages` directory, and the module of its checks to the
`--checks` directory; the point types, which only one file of the directory
may declare, are written as `points.rs` to both. The output is formatted with `rustfmt --edition 2024`,
which must be installed; the text can depend on the rustfmt version, and the
files committed in this repository are produced with the stable toolchain's
rustfmt. The header of each file gives the command that wrote it, with its
arguments as they were given; the paths must be relative to the directory
the command is run from, so that the header is the same wherever the files
are, and absolute paths are refused. With `--check` nothing is written: the
tool compares the files, header included, with what it would write, names
each one that differs or is missing, and exits with a nonzero status if any
does, so it must be run with the arguments the files were written with.

The header shows the tool's own name, `implicit-predicates-gen`, followed by
those arguments. Inside the `implicit-predicates` repository the tool is not
installed: it is run through `cargo run -p implicit-predicates-gen --`
followed by the same arguments, from the root of the repository. The
generated code is regenerated with:

```sh
cargo run -p implicit-predicates-gen -- crates/implicit-predicates/predicates --stages crates/implicit-predicates/src/generated --checks crates/implicit-predicates/tests/generated
```

and checked, stages and checks alike, with the same command followed by
`--check`.

## License

Licensed under either of Apache License, Version 2.0 or MIT license, at your
option.
