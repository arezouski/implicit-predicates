# Fuzzing

Fuzz targets for the public predicates of `implicit-predicates`. Each one
decodes the input into five points (explicit, `Lnc` or `Bpt`) and compares the
result with the exact reference computed on dyadic numbers.

| Target | What it checks |
| --- | --- |
| `orient3d` | `orient3d` and `orient3d_in` on `AnyPoint` and, for explicit points, on `[f64; 3]`, against the exact sign |
| `insphere` | the same for `insphere` and `insphere_in` |
| `insphere_sos` | `insphere_sos` equals `insphere` when that is not zero, is never zero unless the five points are coplanar, and changes sign with the parity of a permutation of the points and their indices |
| `dot_sign_3d` | `dot_sign_3d` and `dot_sign_3d_in` against the exact sign, and that swapping `p` and `r` does not change it |
| `dihedral_sign` | `dihedral_sign` and `dihedral_sign_in` against the exact sign, and that swapping `a` and `b`, or `c` and `d`, does not change it |
| `point_check` | `check` accepts every point the decoder builds, and every `Bpt` built from the same parents with `new_unchecked` |

This package is outside the workspace and is not published. It needs the
nightly toolchain and [`cargo-fuzz`](https://github.com/rust-fuzz/cargo-fuzz):

```sh
cargo install cargo-fuzz --locked
cd fuzz
cargo +nightly fuzz run insphere -- -max_total_time=300
```

Replace `insphere` with any target. Debug assertions are on, so the checks of
the contract of a point that only debug builds run are exercised too. A
failing input is written to `fuzz/artifacts/<target>/`; run it again with
`cargo +nightly fuzz run <target> <path>`.

The decoder has its own tests, which run on the stable toolchain:

```sh
cargo test --manifest-path fuzz/Cargo.toml --lib
```

The scheduled CI run fuzzes each target for five minutes.
