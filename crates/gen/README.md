# implicit-predicates-gen

Code generator for
[`implicit-predicates`](https://crates.io/crates/implicit-predicates): turns
the definition of a predicate into specialized Rust code.

> **Status: early development.** The generator handles predicates over
> explicit points and reads declarations of point types; its API may change.

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
only points gives no predicates, not an error). Each coordinate of a point is held in one graph
of `f64` operations in which equal subexpressions share a node, and the
clause as a list of comparisons, so that they can be evaluated and emitted
operation by operation.

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
  predicate for now.

Each stage returns `None` when it cannot decide.

For the tests of each predicate it writes a second module, of checks, with:

- `exact`, the main form evaluated with the exact dyadic numbers of
  `implicit-predicates-oracle`, the reference the stages are tested against;
- `bound_edge_cases`, pairs of inputs on either side of the error bound of
  the filter for explicit points, one representable value apart in one
  coordinate, built with exact arithmetic from points on a grid at scales
  from just above `D_MIN` to just below `D_MAX`, and written as bit
  patterns;
- for a filter with one uncertainty per axis, `uncertainty_edge_cases`,
  pairs built in the same way on either side of its bound with an
  uncertainty that is not zero on one axis, for each axis, or on all of
  them, given with each pair.

Generated files start with a header saying that they must not be edited
and giving the command that regenerates them.

## Command line

```sh
implicit-predicates-gen <pred-dir> --stages <dir> --checks <dir> [--check]
```

reads every `.pred` file in `<pred-dir>` and writes `<name>.rs` for each
predicate to the `--stages` directory, and the module of its checks to the
`--checks` directory. The output is formatted with `rustfmt --edition 2024`,
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
