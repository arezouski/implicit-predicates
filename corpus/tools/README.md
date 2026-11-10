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
python3 corpus/tools/corpus.py build             # build the original mesher
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

## Tests

```sh
python3 -m unittest discover corpus/tools/tests
```

The tests use small local git repositories and never touch the network.
