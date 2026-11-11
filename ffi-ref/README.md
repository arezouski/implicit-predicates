# implicit-predicates-ffi-ref

Rust bindings to two versions of a third-party C++ library of indirect
geometric predicates, used as a reference by the tests and benchmarks of
this repository:

- `legacy`: the predicate library bundled with the CDT mesher
  (MarcoAttene/CDT), the one the corpus files were recorded with;
- `current`: the Indirect_Predicates library (MarcoAttene/Indirect_Predicates)
  with the NFG arithmetic library it includes (MarcoAttene/nfg).

## License warning

Building this package compiles and links third-party code licensed under
the LGPL. That code is **not part of this repository** and is never copied
into it: it is downloaded, at pinned commits, under the git-ignored
`corpus/data/reference/`. The binaries produced by building this package
are for local tests and benchmarks only and must not be distributed.

The wrapper code in this directory (the C interface in `shim/`, the build
script and the Rust sources) is ours, licensed MIT OR Apache-2.0 like the
rest of the repository.

This package is not a member of the Cargo workspace, so the workspace
builds without a C++ compiler or the downloaded sources. It is not
published.

## Requirements

- Linux on x86_64 with a CPU that supports AVX2 and FMA (the default
  configuration of both libraries);
- a C++20 compiler (`c++`, or the one named by `CXX`) and `nm`;
- the reference sources:

```sh
python3 corpus/tools/corpus.py fetch-reference
cargo test --manifest-path ffi-ref/Cargo.toml
```

The build script never downloads anything. It reads the sources from
`IPREF_REFERENCE_DIR` (default `../corpus/data/reference`, relative to this
directory), which must contain `cdt-legacy/`, `indirect-predicates/` and
`nfg/`. If they are missing, the build fails and prints the command above.

## API

The modules `legacy` and `current` have the same API:

```rust
let mut points = legacy::Points::new();
let a = points.explicit([0.0, 0.0, 0.0]);
// ... b, c, d
let m = points.lnc(a, b, 0.5); // t = 0 is a, t = 1 is b; parents must be explicit
let s = points.orient3d(a, b, c, m);
let (points, ids) = legacy::from_corpus(&corpus); // None for LPI and TPI points
```

`Points` owns the points built in C++ and frees them all when dropped. LNC
points hold references to their parents in C++; the C++ store keeps every
point at a fixed address until the whole store is freed, so a parent always
outlives its children. `Points` is not `Send`: the libraries keep
thread-local memory pools.

Signs are returned in the convention of `implicit_predicates_corpus::exact`
(orient3d is the sign of `det[a - d; b - d; c - d]`, insphere the sign of the
lifted 4x4 determinant with rows `(p - e, |p - e|^2)`). Both libraries return
the opposite sign for both predicates; each module applies its factor in one
place. For the current library this was determined by the tests: with
factors of -1 every call of the sample corpora and every generated input
matches the exact sign, and with +1 every call with a nonzero sign differs.

In the current library an LNC point may have implicit parents; with explicit
parents it is built by the same constructor with the same parents and
parameter (`t = 0` at the first parent), so `current::Points::lnc` maps
directly onto it.

## Compile options

Each version is compiled with the options of its own CMake build for the
default x86_64 AVX2 configuration, in the order CMake passes them in a
Release build. The build script prints them as warnings.

| Version | Options |
| --- | --- |
| legacy | `-O3 -DNDEBUG -std=gnu++20 -O2 -frounding-math -mavx2 -mfma` |
| current | `-O3 -DNDEBUG -std=gnu++20 -O3 -frounding-math -ffp-contract=off -mavx2 -mfma` |

The include directories are those of the CMake builds: `include/` for the
legacy library; the repository root, `include/` and NFG's `include/` for the
current one. Neither build defines `NFG_IP_ERROR_THROWS`, so it is not
defined here. The mesher's stack-size link option applies to executables
only and is not used. `CXXFLAGS` is not read.

To build shared libraries that export only their C interface, the build
script adds `-fPIC -fvisibility=hidden -fvisibility-inlines-hidden -shared
-Wl,-z,defs` and a linker version script; these do not change the generated
arithmetic.

## Symbol isolation

The two versions define many identical C++ names, most of them inline
functions and templates in headers (for example the global
`orient3d_exact(double, ...)` of both). In object files these are weak
symbols, and a static link would silently keep one copy for both versions;
hidden visibility alone does not prevent that.

So each version, with its shim, is built into its own shared library,
`libipref_legacy.so` and `libipref_current.so`, linked with a version script
that exports `ipref_<version>_*` and makes every other symbol local. Inside
a library every C++ symbol is bound locally at link time, and the dynamic
linker can bind across libraries only through the exported symbols, which
are the C functions of each interface: six for the bindings, one benchmark
hook and four test hooks. The build script checks this with `nm -D
--defined-only` on both libraries: it fails unless each library exports
exactly the eleven functions of its own interface, and if a symbol is exported
by both. The C++ runtime
(`libstdc++`) is a shared dependency of both and is not duplicated.

The test `both_versions_answer_in_one_process` loads both libraries in one
process and interleaves their calls on the same inputs.

The tests, benchmarks and examples of this package find the libraries
through an rpath to the build directory, without `LD_LIBRARY_PATH`. Another
package linking this one would need its own rpath.

## The C interface

`shim/ipref.h` declares opaque handles and functions that take and return
only doubles and integers. Points are named by indices; the Rust wrapper
checks every index and that the parents of an LNC point are explicit
before calling.

Every function of the interface runs its work through one guard
(`shim/guard.hpp`), always on, in release builds too:

- **Rounding mode.** Rust code requires round-to-nearest. The interval
  stages of both libraries switch the rounding mode to upward and back
  inside their functions. The guard reads the rounding mode on entry and
  again on return, and aborts with a message if it is not to nearest:
  `ipref: called with the rounding mode not to nearest` (the caller's
  fault) or `ipref: the reference library returned with the rounding mode
  not to nearest` (the library's). On x86_64 the guard reads the SSE
  control register (MXCSR), whose rounding mode is the one Rust's
  arithmetic and the libraries' SSE and AVX code follow; `fegetround`
  would read the x87 control word, which can differ. Elsewhere it uses
  `fegetround`. That is one read of a control register per check,
  negligible next to a predicate call.
- **Exceptions.** Any C++ exception is caught and the process aborts with a
  message, so no exception crosses the interface.
- **Error routine.** The libraries report internal errors (such as a
  division by zero in their exact arithmetic) through their error routine
  `ip_error`, which prints a message and ends the process with `exit`: with
  status 0 in the legacy library and with a failure status in the current
  one. A status of 0 would make a test run that ended there look
  successful. The guard marks the calling thread as inside a library call,
  and an exit handler registered once per process (when each library is
  loaded) turns an `exit` from inside a call into `_exit(EXIT_FAILURE)`,
  after printing `ipref: the reference library ended the process from
  inside a call`. `exit` runs the handlers on the thread that called it, so
  the per-thread flag is correct with tests running on several threads.
  Nothing in the libraries is modified. The error routine is not reachable
  from orient3d and insphere on explicit and LNC points: in both libraries
  it is called only from their rational and big-integer division and from
  coordinate conversions of point types these bindings never create, and
  the built bindings, without the test hook below, contain neither its
  messages nor a reference to `exit`.

The `bench_overhead` function of the interface is a benchmark hook,
exposed in Rust only as the hidden method `Points::overhead`: it does
everything an orient3d call does (the call through the interface, the
guard, the index checks and the lookup of four points) except evaluating
the predicate, and returns 0.

The four `test_*` functions of the interface are test hooks, exposed in
Rust only through the hidden module `test_hooks`: one calls the error
routine from inside a call, one returns from a call with the rounding mode
set upward, one creates a handle with the rounding mode set upward, and one
does the same with upward rounding set only in MXCSR on x86_64. The
tests in `tests/failures.rs` run each in a child process and check that it
ends the process as described above.

## Benchmarks

```sh
cargo bench --manifest-path ffi-ref/Cargo.toml
# On generated corpus files, see the known crash below; this runs the
# legacy groups alone:
IPREF_CORPUS_DIR=corpus/data/corpus cargo bench --manifest-path ffi-ref/Cargo.toml -- '^legacy/'
```

`benches/reference.rs` measures both versions on recorded predicate calls:
the versioned sample corpora in `corpus/samples/` by default, or every
`.ipcorpus` file of the directory named by `IPREF_CORPUS_DIR`. For each
version it has these groups:

| Group | What one element is |
| --- | --- |
| `<version>/orient3d`, `<version>/insphere` | one call, on points built beforehand, by number of implicit points: `0`, `1`, `2` and `3+` (a bucket with no calls is skipped and named on standard error); the calls are a weighted resample of the stored ones, see below |
| `<version>/overhead` | one call of the benchmark hook, on the points of every orient3d call |
| `<version>/construct` | one explicit or LNC point built, per corpus file, without freeing them |
| `<version>/replay` | one stored call, per corpus file, each once and in stored order, including building all of its points |

The throughput in elements per second is the inverse of the time per
element. Every reference figure includes the cost of these bindings, which
`overhead` measures; subtracting it gives an estimate of the time spent in
the library itself. Calls on LPI or TPI points, which these bindings cannot
build, are left out and counted on standard error.

A corpus file stores a stratified sample of the calls of its recording run
(see `corpus/FORMAT.md`), with each stratum capped, so the stored calls
over-represent rare strata such as zero signs. Each bucket of the
`orient3d` and `insphere` groups therefore holds as many calls as the files
store for it, drawn by deterministic weighted resampling: a stored call
weighs `total / kept` of its stratum, so the bucket reflects the mix of
calls of the recording runs. `replay` replays the stored calls as they
are.

### Known crash of the current library

On some calls of generated corpus files (not of the versioned samples), the
`current` library ends the process with SIGSEGV: in insphere with 3 or more
implicit points, and so in the replay of any corpus file that contains such
calls. The fault happens inside the library's multi-precision arithmetic, on
memory the library allocates itself, and whether it happens depends on the
heap layout: the same call may succeed in another process. It was observed
with GCC 16 and the library's own `-O3 -mavx2` options. The `legacy` library
is not affected, and the versioned samples run to completion with both.

The compile options of each library are deliberately left as upstream's, so
the figures of the affected groups (`current/insphere/3+`, and
`current/replay` of such files) are not available on generated corpora.
Select the other groups with criterion's filter, for example
`-- '^current/(orient3d|insphere/[012]|overhead|construct)'`.
