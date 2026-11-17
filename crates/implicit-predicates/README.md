# implicit-predicates

Exact geometric predicates in Rust for explicit points and for *implicit*
points: points defined by an unevaluated construction, such as the point at a
given parameter on a segment or on a triangle.

> **Status: early development.** The crate provides `orient3d`, `insphere`,
> `insphere_sos`, `dot_sign_3d` and `dihedral_sign` for explicit points,
> points on a segment and points on a triangle; the API may change. Points
> at the intersection of a line and a plane, or of three planes, and points
> defined on other implicit points, are not implemented.

## Example

```rust
use implicit_predicates::{Lnc, Sign, orient3d};

let (a, b, c) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
// a, b and c appear counter-clockwise from (1, 1, 1), above the plane.
assert_eq!(orient3d(&a, &b, &c, &[1.0, 1.0, 1.0]), Sign::Negative);
assert_eq!(orient3d(&a, &b, &c, &[0.0, 0.0, 0.0]), Sign::Positive);
// A point on the plane.
assert_eq!(orient3d(&a, &b, &c, &[0.5, 0.25, 0.25]), Sign::Zero);
// A point on the edge between b and c, c + 0.3 (b - c): on the plane,
// exactly, although its coordinates are not representable as `f64`.
let m = Lnc::new(b, c, 0.3).unwrap();
assert_eq!(orient3d(&a, &b, &c, &m), Sign::Zero);
```

## Predicates

Each predicate takes points of any type that implements `Point` and returns
the exact sign of an expression at their exact coordinates:

- `orient3d(a, b, c, d)`: the determinant of `a - d`, `b - d`, `c - d`, which
  tells on which side of the plane through `a`, `b` and `c` the point `d`
  lies (Shewchuk's convention);
- `insphere(a, b, c, d, e)`: the position of `e` with respect to the sphere
  through `a`, `b`, `c` and `d` (Shewchuk's convention). It is positive when
  `e` is inside the sphere if `orient3d(a, b, c, d)` is positive; the
  function does not check that orientation;
- `insphere_sos(a, b, c, d, e, idx)`: `insphere` with its ties broken by a
  symbolic perturbation that depends on the order of the five distinct
  indices in `idx`. It is zero only when the five points are coplanar;
- `dot_sign_3d(p, q, r)`: the sign of `(p - q) . (r - q)`. The apex of the
  angle is the middle argument `q`. It is positive when the angle is acute,
  negative when it is obtuse, and zero when it is right, or when `p` or `r`
  is `q`;
- `dihedral_sign(a, b, c, d)`: the sign of
  `[(b - d) x (c - d)] . [(a - d) x (c - d)]`, for the triangles `a, c, d`
  and `b, c, d` that share the edge `c, d`. It is positive when the dihedral
  angle at that edge is acute, negative when it is obtuse, and zero when it
  is right or when a triangle is degenerate.

`dot_sign_3d` and `dihedral_sign` have no symbolic perturbation: zero is a
possible answer.

```rust
use implicit_predicates::{Sign, insphere, insphere_sos, orient3d};

// Four points of the unit sphere, positively oriented.
let (a, b, c, d) = (
    [1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, 0.0, 1.0],
    [-1.0, 0.0, 0.0],
);
assert_eq!(orient3d(&a, &b, &c, &d), Sign::Positive);
assert_eq!(insphere(&a, &b, &c, &d, &[0.0, 0.0, 0.0]), Sign::Positive); // inside
assert_eq!(insphere(&a, &b, &c, &d, &[2.0, 0.0, 0.0]), Sign::Negative); // outside
let e = [0.0, 0.0, -1.0];
assert_eq!(insphere(&a, &b, &c, &d, &e), Sign::Zero); // on the sphere
// The indices break the tie: `e` has the smallest one, so it counts as inside.
assert_eq!(insphere_sos(&a, &b, &c, &d, &e, [1, 2, 3, 4, 0]), Sign::Positive);
```

The point types are `[f64; 3]` (an explicit point, not validated),
`Explicit` (a validated explicit point), `Lnc`, built from two explicit
points `p` and `q` and a parameter `t` in `[0, 1]`, the point `q + t (p - q)`
on the segment between them, `Bpt`, built from three explicit points `p`,
`q` and `r` and two parameters `u` and `v` with `u >= 0`, `v >= 0` and
`u + v <= 1`, the point `u p + v q + (1 - u - v) r` on the triangle between
them (`Bpt::new` checks the last condition exactly, without rounding), and
`AnyPoint`, an `Explicit`, an `Lnc` or a `Bpt`. `AnyPoint` is
`#[non_exhaustive]`: a `match` on it needs a wildcard arm.

```rust
use implicit_predicates::{Bpt, Sign, dihedral_sign, dot_sign_3d, orient3d};

// A triangle in the plane z = 0 with its right angle at the origin.
let (p, q, r) = ([2.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0; 3]);
// The point 0.2 p + 0.3 q + 0.5 r, which is not representable as `f64`.
let m = Bpt::new(p, q, r, 0.2, 0.3).unwrap();
// It is on the plane of the triangle, exactly, and sees p at an acute angle.
assert_eq!(orient3d(&p, &q, &r, &m), Sign::Zero);
assert_eq!(dot_sign_3d(&p, &r, &m), Sign::Positive);

// The edge is the z axis, from d to c. The half-plane of `a` is at 45
// degrees from the x axis; those of the points `b` are at 90 and at 135.
let (c, d) = ([0.0, 0.0, 1.0], [0.0; 3]);
let corners = ([1.0, 1.0, 0.0], [2.0, 2.0, 1.0], [3.0, 3.0, 5.0]);
let a = Bpt::new(corners.0, corners.1, corners.2, 0.1, 0.2).unwrap();
assert_eq!(dihedral_sign(&a, &[0.0, 1.0, 0.0], &c, &d), Sign::Positive);
assert_eq!(dihedral_sign(&a, &[-1.0, 1.0, 0.0], &c, &d), Sign::Zero);
```

A floating-point filter on the approximations of the points decides most
calls. When it cannot, the exact stages take over. `orient3d` on explicit
points goes to exact stages on floating-point expansions held on the stack,
so in release builds no call on explicit points whose nonzero coordinates
have magnitudes in `2^-188..=2^238` allocates; debug builds allocate an
arena for the check of the contract of the points. `dot_sign_3d` has such
stages on the stack too, for a range of magnitudes that its documentation
gives, and `dihedral_sign` has a stage on the stack only when every
difference of coordinates it takes is exact in `f64` and in the range its
documentation gives. With a point on a segment or on a triangle, for
`insphere` and `insphere_sos` whatever the points, and for the calls of
`dot_sign_3d` and `dihedral_sign` that those stages do not decide, the exact
coordinates go to an arena of the calling thread, 16,384 `f64` to start
with, allocated on the first such call, that grows on the heap when a call
needs more. The last exact stage evaluates the determinant on big integers
in that arena; it decides every input, and for `orient3d` and `dot_sign_3d`
it runs only when the expansions cannot decide, because a coordinate is too
large or too small for them. `orient3d_in`, `insphere_in`,
`insphere_sos_in`, `dot_sign_3d_in` and `dihedral_sign_in` take the arena
instead and report `ScratchExhausted` if it is too small. Without its
default `std` feature the crate is `no_std`: `orient3d`, `insphere`,
`insphere_sos`, `dot_sign_3d` and `dihedral_sign` are not available, and the
`_in` functions are the way to call the predicates.

So the result is a `Sign` for every finite input, whatever the magnitude of
the coordinates. An input that is not finite (NaN or infinite) makes the
predicates panic.

### Point types of your own

`Point` is a public trait, and a type of your own can implement it. Its
contract: `err` bounds the error of each coordinate of `approx`,
`is_explicit` returns `true` only when `approx` holds the exact coordinates
with zero error, `exact` writes valid expansions, and `exact_big` gives the
same coordinates as `exact` whenever `exact` succeeds, failing with
`OutOfRange` only for inputs that are not finite, and with `Weighted` for
exact coordinates with a weight other than one. The predicates support only
exact coordinates with a weight of `Weight::One` for now: a point whose
exact coordinates have another weight makes them panic.

Release builds of the predicates do not check the contract, and an
implementation that breaks it makes them return wrong signs, silently.
`check` tests that a point keeps it, and is meant for the tests of a type of
your own:

```rust
use implicit_predicates::{Lnc, Scratch, check};

let m = Lnc::new([0.3, 2.0, 5.0], [0.1, 2.0, 5.0], 0.5).unwrap();
let mut buf = vec![0.0; 4096];
assert_eq!(check(&m, &mut Scratch::borrowed(&mut buf)), Ok(()));
```

`check` sees only what the views of a point show: it cannot know that the
exact coordinates are the ones the point is meant to have. Debug builds also
catch part of the violations, with a panic whose message begins with
``breaks the contract of `Point`:``: the predicates check that no error bound
is NaN or negative, that a point that says it is explicit has no error, and,
when the filter does not decide, that the exact coordinates a point writes to
the arena are valid; and the predicates without the `_in` suffix, when a call
reaches the exact stages, run `check` on each point. An input that is not
finite, or a weight other than one, keeps the panic of the predicates in all
builds.

The point types of this crate are generated by `implicit-predicates-gen`
from their declarations.

## Goals

- Exact sign for every finite input.
- Speed close to classical filtered predicates in the common case.
- Explicit points and implicit points through one API.
- New predicates and point types through a code generator.

Versions 0.0.x are development releases: each of them may change the API.

## Minimum supported Rust version

Rust 1.86.

## License

Licensed under either of Apache License, Version 2.0 or MIT license, at your
option.
