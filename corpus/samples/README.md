# Recorded samples

Small corpus files (format in `../FORMAT.md`) recorded from the reference
mesher on three models of the Thingi10K dataset, so that tests and
benchmarks have real predicate calls without downloading anything. Each file
was cut from the full recording of its model with

```sh
ipcorpus extract --max-calls 200 <full>.ipcorpus thingi10k-<file id>.ipcorpus
```

which keeps the first 200 stored calls of each stratum (predicate, number of
implicit points, zero or non-zero sign), and the points they use.

Together they contain orient3d and insphere calls, calls with explicit and
LNC points, and calls with a zero sign, for both predicates; every recorded
sign is checked against the exact sign by `tests/samples.rs`.

## Source models

Only models released into the public domain are used. The identifiers,
authors and licenses are as given by the Thingi10K metadata.

| File | Thingi10K file id | Thing id | Author | License |
| --- | ---: | ---: | --- | --- |
| `thingi10k-97167.ipcorpus` | 97167 | 33696 | chris | Creative Commons - Public Domain Dedication |
| `thingi10k-217026.ipcorpus` | 217026 | 101105 | chris | Creative Commons - Public Domain Dedication |
| `thingi10k-1716275.ipcorpus` | 1716275 | 1092050 | tomburtonwood | Creative Commons - Public Domain Dedication |

Thingi10K: Q. Zhou and A. Jacobson, "Thingi10K: A Dataset of 10,000
3D-Printing Models", arXiv:1605.04797, 2016.
