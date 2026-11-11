# implicit-predicates-gen

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
gives the polynomial to use for exact evaluation; it may use the raw
coordinates and integer constants.

## Point types

The same files declare the types of points a predicate can take:

```text
point Explicit(p: F64x3) = p;

point Lnc(p: F64x3, q: F64x3, t: f64) = q + t*(p - q)
    where 0 <= t, t <= 1;
```

A declaration is the word `point`, a name, parameters of type `F64x3` (three
`f64` coordinates) or `f64`, `=`, an expression and `;`. The expression is
built from the parameters, integer literals, `+`, `-` (binary and unary), `*`
and parentheses. Operations on an `F64x3` are per coordinate: a sum or a
difference combines two values of the same type, a product combines an `f64`
with an `F64x3` (in either order) or two `f64`, and the value of the whole
expression must be an `F64x3`. Every parameter must be used by the
expression.

An optional `where` clause lists, separated by commas, comparisons `<=`
between sums of `f64` parameters and integer literals, such as
`0 <= u, 0 <= v, u + v <= 1`: what the parameters must satisfy for the point
to exist. The names of points and parameters cannot be Rust keywords, and two
points cannot share a name.

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
- `AnyPoint`, an enum with one variant per type, which implements `Point`
  too.

The error bound is computed operation by operation, by a rule that
`error_bound` documents together with the proof that the result is an upper
bound: a sum adds the errors of its operands and `2^-53` times its value, a
product propagates the errors of its factors and adds `2^-53` times its
value plus `2^-1074`, each rounding term only when no operand is zero, and
a slack factor derived from the number of roundings of the computation
covers the rounding of the computation itself. A coordinate that is exact,
such as one shared by the parents of a point on a line, gets an error of
exactly zero.

The name of a point type must start with an uppercase letter and cannot
be a name that the generated module uses, such as `Point` or `Result`; a
parameter cannot be named like a field or a function of the generated type,
such as `approx` or `new`.

For now a `where` comparison must reduce, once the parameters on both sides
cancel, to one parameter against an integer of magnitude at most `2^53`, so
that it is checked exactly; a comparison that names no parameter is an
error. The checks module of the point types holds `exact_<name>`, the exact
coordinates of each type that is not explicit.

## What the generator checks

Before writing anything, the generator expands both forms and requires them
to be the same nonzero polynomial, checks that the main form is homogeneous
in the differences, and derives, in exact arithmetic:

- the error bound of the floating-point filter and the range of scales in
  which overflow and underflow cannot invalidate it;
- for each exact stage, the range of input magnitudes for which every exact
  product of the arithmetic core stays in its domain and no value
  overflows, and the worst-case length of every temporary.

A definition that fails any of these is reported as `file:line:column:
message`.

## Generated code

For each predicate the generator writes one module with:

- the constants of the filter and the ranges of the exact stages
  (`DIFFERENCES_RANGE`, `TRANSLATED_RANGE`, `UNTRANSLATED_RANGE`);
- `filter`, the main form evaluated in `f64` with its error bound. When
  every term of the main form has the same degree in the differences of
  each axis, as for an orientation, the bound has one size and one
  uncertainty per axis: the constants are `DEGREES`, `EPSILON`, `KAPPA` (one
  per axis), `D_MIN` and `D_MAX`, the filter takes `e: [f64; 3]`, and it
  returns `Some(Sign::Zero)` when some axis is flat and every term has
  positive degree in it. Otherwise the bound is global: the constants are
  `DEGREE`, `EPSILON`, `KAPPA`, `D_MIN` and `D_MAX`, and the filter takes
  `e: f64`;
- `exact_differences`, `expansions_translated` and, when there is an
  `#[exact]` form, `expansions_untranslated`: exact evaluations with
  floating-point expansions on stack buffers;
- `staged`, which runs the filter and then the exact stages in the order:
  differences that are exact, the translated expansion form, then the
  `#[exact]` form if one is given. The order is the same for every
  predicate for now;
- for points of any type: `filter_points`, the filter on the approximations
  of the points with the uncertainty of their errors (zero when every point
  is explicit); `exact_stages`, the exact stages of `staged`;
  `exact_stages_in`, the expansion forms over spans of an arena, on the
  exact coordinates of the points; and `staged_in`, which runs
  `filter_points`, then `exact_stages` when every point is explicit and
  `exact_stages_in` otherwise.

Each stage returns `None` when it cannot decide.

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
  uncertainty that is not zero on one axis, for each axis, or on all of
  them, given with each pair;
- `check_bound_edge_cases` and `check_uncertainty_edge_cases`, which run a
  stage, given as a closure of the coordinates of the points (and of their
  errors), on those cases and check its signs, so that the tests can run
  them on every stage and on points of any type.

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
