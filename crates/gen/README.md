# indirect-predicates-gen

Code generator for
[`indirect-predicates`](https://crates.io/crates/indirect-predicates): turns
the definition of a predicate into specialized Rust code.

> **Status: early development.** The generator handles predicates over
> explicit points; its API may change.

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

- the constants of the filter (`DEGREE`, `EPSILON`, `KAPPA`, `D_MIN`,
  `D_MAX`) and the ranges of the exact stages (`DIFFERENCES_RANGE`,
  `TRANSLATED_RANGE`, `UNTRANSLATED_RANGE`);
- `filter`, the main form evaluated in `f64` with its error bound;
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
  `indirect-predicates-oracle`, the reference the stages are tested against;
- `bound_edge_cases`, pairs of inputs on either side of the error bound of
  the filter, one representable value apart in one coordinate, built with
  exact arithmetic from points on a grid at scales from just above `D_MIN`
  to just below `D_MAX`, and written as bit patterns.

Generated files start with a header saying that they must not be edited
and giving the command that regenerates them.

## Command line

```sh
indirect-predicates-gen <pred-dir> --stages <dir> --checks <dir> [--check]
```

reads every `.pred` file in `<pred-dir>` and writes `<name>.rs` for each
predicate to the `--stages` directory, and the module of its checks to the
`--checks` directory. The output is formatted with `rustfmt --edition 2024`,
which must be installed; the text can depend on the rustfmt version, and the
files committed in this repository are produced with the stable toolchain's
rustfmt. The header of each file gives the command that wrote it, with its
arguments as they were given. With `--check` nothing is written: the tool
compares the files, header included, with what it would write, names each
one that differs or is missing, and exits with a nonzero status if any does,
so it must be run with the arguments the files were written with.

In the `indirect-predicates` repository, the generated code is regenerated
with a single command from the root:

```sh
cargo run -p indirect-predicates-gen -- crates/indirect-predicates/predicates --stages crates/indirect-predicates/src/generated --checks crates/indirect-predicates/tests/generated
```

and checked, stages and checks alike, with the same command followed by
`--check`.

## License

Licensed under either of Apache License, Version 2.0 or MIT license, at your
option.
