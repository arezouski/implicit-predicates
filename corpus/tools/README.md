# Corpus tooling

Python scripts (standard library only, Python 3.10 or later) that prepare the
third-party reference code used for comparisons. Everything is downloaded and
built under `corpus/data/`, which is git-ignored.

## License warning

The downloaded reference code (CDT and Indirect_Predicates, with their
submodules) is licensed under the LGPL or GPL. It is not part of this
repository and must never be copied into it, in whole or in part. It is only
built and run locally, and used for comparison in tests and benchmarks.

## Requirements

- git
- CMake and a C++20 compiler (Ninja is used when available)
- Python 3.10 or later
- `uv`, for the tools that need Python packages with their own environment

## Commands

Run them from anywhere; paths are derived from the script location.

```sh
python3 corpus/tools/corpus.py fetch-reference   # clone the references at their pinned commits
python3 corpus/tools/corpus.py build             # build the original and the recording mesher
```

`fetch-reference` leaves `corpus/data/reference/cdt-legacy/` and
`corpus/data/reference/indirect-predicates/` (with submodules) at the commits
pinned in `corpus/tools/reference.py`. If a checkout is already at the pinned
commit it does nothing. If a checkout is at another commit or has modified
tracked files, it fails without changing anything. Untracked files, such as
outputs written by running the mesher's examples, are ignored by that check.

`build` compiles the mesher out of tree with `-DLGPL=ON` and `Release` into
`corpus/data/build/cdt-original/`, leaving the downloaded sources untouched.
The executable is `corpus/data/build/cdt-original/cdt`.

`build` also produces `corpus/data/build/cdt-recording/cdt`, a mesher that
behaves like the original and additionally records its predicate calls. It
is built with the same options from an instrumented copy of the sources in
`corpus/data/build/cdt-recording/src/`, made afresh on every build by
`instrument.py`; the downloaded checkout is never modified.

## Recording predicate calls

```sh
IPCORPUS_OUT=bracket.ipcorpus IPCORPUS_MODEL=bracket \
    corpus/data/build/cdt-recording/cdt bracket.off
```

| Variable | Meaning |
| --- | --- |
| `IPCORPUS_OUT` | Output corpus file. Without it nothing is recorded. |
| `IPCORPUS_MODEL` | Model name stored in the file and used to seed the sampling (default: empty). |
| `IPCORPUS_STRATUM_CAP` | Maximum number of calls stored per stratum (default: `100000`). |

The corpus format is described in `corpus/FORMAT.md`. Two runs on the same
model give identical files.

- Before `main` starts, the recorder removes any `IPCORPUS_OUT` and
  `IPCORPUS_OUT.partial` left by an earlier run, so an old file can never
  pass for the output of a later run, whatever happens to that run.
- When the mesher finishes, the file is written to `IPCORPUS_OUT` plus
  `.partial` and renamed to `IPCORPUS_OUT`.
- When the mesher reports an error, it calls `ip_error`, which prints the
  message and exits with status 0. The instrumented `ip_error` first
  abandons the recording: no file is written, and standard error gets one
  line starting with `ipcorpus: recording abandoned after N calls`. A tool
  driving the mesher must therefore treat a missing `IPCORPUS_OUT` as a
  failed run, even when the exit status is 0.
- A run that is killed or aborts leaves no file.

The recorder (`recorder/recorder.hpp` and `recorder/recorder.cpp`) is our own
code. It keeps global state and is not thread-safe; the mesher evaluates
predicates from a single thread. It counts every call, and stores a reservoir
sample of at most `IPCORPUS_STRATUM_CAP` calls per stratum (predicate, number
of implicit points, zero or non-zero sign), drawn with a generator seeded by
the model name. Stored calls are written in the order they happened, with a
point table holding only the points they use and their parents. A point is
identified by value: an explicit point by the bits of its coordinates, an LNC
point by its two parents and the bits of its parameter.

Besides the predicate hooks described below, `instrument.py` adds a call to
the recorder at the start of `ip_error` (`include/numerics.h`). It places
each hook with a one-line anchor that must appear
exactly once, or with a regular expression over function definitions that
must match a number of times fixed in the script; otherwise it fails, naming
the hook, before writing anything. It contains no excerpt of the reference
code.

### What is recorded

Every orient3d and insphere call of the mesher, as returned by
`genericPoint::orient3D` and `genericPoint::inSphere` (the reference's
indirect predicates on explicit and implicit points), with the points in the
order the mesher passed them and the sign the mesher received. The mesher
(`src/`) reaches these predicates only through those two functions:

- `TetMesh::vOrient3D` and `TetMesh::vInSphere` (`src/delaunay.h`), used
  throughout the Delaunay construction in `src/delaunay.cpp` and by the
  segment and face recovery in `src/PLC.h` and `src/PLC.cpp` (directly and
  through `cachedOrient3D` and `localOrient3d`);
- `invOrient3D` and `invInSphere` (`src/giftWrap.cpp`), used by the gift-
  wrapping face recovery;
- direct calls of `pointType::orient3D` in `src/delaunay.cpp` and
  `src/giftWrap.cpp`, including the snapping checks on rounded copies of the
  vertices;
- the reference's geometric helpers that call `genericPoint::orient3D`
  (segment and triangle crossing tests in `include/implicit_point.hpp`),
  when the mesher calls them.

The `vOrient3D`, `vInSphere`, `invOrient3D` and `invInSphere` wrappers negate
the sign; the corpus stores the sign before that negation. For both
predicates the recorded sign is the opposite of the exact sign of
`corpus/src/exact.rs`: the reference uses the opposite sign convention.

Not recorded, and why:

- The double-only predicates in `src/numeric_wrapper.h` (`orient3d` and
  `::inSphere` on coordinates) belong to the alternative number types of that
  header and are compiled only without `USE_INDIRECT_PREDS`, which the
  mesher always defines.
- `USE_MAROTS_METHOD` (in `src/delaunay.h` and `src/delaunay.cpp`) is
  disabled by `-DLGPL=ON`; that code is not compiled.
- Two-dimensional predicates (`orient2D*`, `incircle*`) and other
  predicates (`lessThan*`, `dotProductSign*`) are not part of the corpus.
- A predicate evaluated inside another recorded predicate would not be a
  call of the mesher; a depth counter discards it. The reference does not do
  this today.

The mesher only creates explicit and LNC points (Steiner points on input
edges), so no LPI or TPI point is expected; the recorder supports them
anyway.

The stage that decided a call is the last stage function of the reference's
cascades (`*_filtered`, `*_interval`, `*_bigfloat`, `*_exact` of orient3d and
insphere) entered during the call. A call that enters none would be stored
with stage `Unknown`.

### Independent counter

On exit the recording mesher prints to standard error the number of recorded
calls and, independently, the number of entries into the cascade entry
functions (`orient3d` and `inSphere` on coordinates, `orient3d_indirect_*`
and `inSphere_*` on implicit points), counted at any depth. Each recorded
call evaluates exactly one cascade, and no cascade is entered outside a
recorded call, so the two numbers must be equal. A larger cascade count would
reveal a call path that bypasses the recorder.

## Tests

```sh
python3 -m unittest discover corpus/tools/tests
```

The tests use small local git repositories and toy source trees, and never
touch the network. The end-to-end tests of the recording mesher are in Rust
and need the built executables and the downloaded example models:

```sh
cargo test -p indirect-predicates-corpus -- --ignored recorded
```

For every example model they check that the original and the recording
mesher write identical `.tet` files, that the corpus is valid, that every
recorded sign equals the exact sign times one factor per predicate, the same
for every call, that a small stratum cap bounds the stored calls without
changing any counter, and that two runs give identical files. They also
check that a model the mesher rejects after evaluating predicates leaves no
corpus. The Python tests of the recorder alone (`tests/test_recorder.py`)
build a small driver program and are skipped when no C++ compiler is found.
