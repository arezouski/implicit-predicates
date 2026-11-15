# Corpus file format

A corpus file stores a table of points, the geometric predicate calls recorded
over them, and the counters and sampling statistics of the recording run. All
integers and floats are little-endian; floats are IEEE 754 binary64.

## Layout

| Section  | Content |
| -------- | ------- |
| header   | `"IPCORPUS"` (8 bytes), `u32` version (= 1), `u32` name length, name (UTF-8), `u64` point count, `u64` counter count, `u64` stratum count, `u64` call count |
| points   | per point: `u8` kind, then the fields below |
| counters | `u8` predicate, `u8` implicit count, `i8` sign, `u8` stage, `u64` count |
| strata   | `u8` predicate, `u8` implicit count, `u8` zero flag (0 or 1), `u64` total, `u64` kept |
| calls    | `u8` predicate, `i8` sign, `u8` stage, then 4 or 5 `u32` point indices |

Point kinds:

| Kind | Name     | Fields |
| ---- | -------- | ------ |
| 0    | explicit | 3 x `f64` (x, y, z) |
| 1    | LNC      | `u32` p, `u32` q, `f64` t |
| 2    | LPI      | 2 x `u32` line points, 3 x `u32` plane points |
| 3    | TPI      | 3 planes of 3 x `u32` points each (9 x `u32`) |

Predicates: 0 = orient3d (4 points), 1 = insphere (5 points).
Stages: 0 = unknown, 1 = filtered, 2 = interval, 3 = bigfloat, 4 = exact.

A call has room for five point indices in memory. An orient3d call stores
four indices on disk; a reader sets the fifth to 0, and an in-memory call whose
unused fifth index is not 0 is invalid. An insphere call stores five.

The implicit count of a call is the number of its points (4 or 5) that are not
explicit points. A counter counts calls with one outcome: predicate, implicit
count, returned sign, and deciding stage. A stratum groups calls by predicate,
implicit count and whether the sign is zero; `total` is how many calls of the
stratum were seen and `kept` how many are stored in the file. The counters
and `total` always describe the whole recording run: a corpus cut from a
larger one (`ipcorpus extract`) keeps them unchanged and stores fewer calls,
so only `kept` changes. A stored call thus stands for `total / kept` calls
of its stratum.

## Validity rules

A reader rejects a file unless all of the following hold:

- the magic and version are correct, and every predicate, stage and point-kind
  code is known;
- the model name is valid UTF-8;
- the parents of a point have a smaller index than the point;
- the parents of an LNC point are explicit points;
- every coordinate and every LNC parameter is finite;
- every sign, in calls and in counters, is -1, 0 or 1;
- every call index refers to an existing point;
- the implicit count of every counter and every stratum is at most the
  arity of its predicate (4 for orient3d, 5 for insphere);
- the zero flag of a stratum is 0 or 1;
- no counter (same predicate, implicit count, sign and stage) and no stratum
  (same predicate, implicit count and zero flag) appears twice;
- every stratum that has stored calls has a sampling entry, and `kept` equals
  the number of stored calls of that stratum, computed with the implicit
  count defined above;
- `kept` does not exceed `total`;
- the counters of a stratum (same predicate and implicit count, with a zero
  sign for a zero stratum and a non-zero sign otherwise) add up to its
  `total`, and their sum does not overflow `u64`;
- every counter belongs to a stratum that has a sampling entry, so a writer
  emits a stratum for every counter group, even when `kept` is 0;
- there are no bytes after the last call.

Readers never size allocations from the header counts: data is read first and
a truncated file is an error.

## LNC parameter

An LNC point lies on the line through its two parents `p` and `q`. Its
coordinates are `p + t * (q - p)`, evaluated exactly. The parameter `t = 0` is
the first parent `p` and `t = 1` is the second parent `q`; values outside
`[0, 1]` are allowed and give points on the line beyond the segment.

The `Lnc` type of the `implicit-predicates` crate takes its parents in the
other order: `Lnc::new(a, b, t)` is `b + t * (a - b)`. A stored point with
parents `p` and `q` and parameter `t` is therefore `Lnc::new(q, p, t)`.

## Sign conventions

Calls store the sign exactly as returned by the program that recorded them.
The verifier compares it with an exact sign defined as follows, the same
convention as Shewchuk's predicates and the `robust` crate:

- `orient3d(a, b, c, d)` is the sign of the determinant of the 3x3 matrix
  with rows `a - d`, `b - d` and `c - d`.
- `insphere(a, b, c, d, e)` is the sign of the 4x4 determinant whose rows are
  `(p - e, |p - e|^2)` for `p` in `a, b, c, d`.

A recorded sign matches when it equals the exact sign multiplied by a
per-predicate factor (+1 or -1), which accounts for a recording program that
uses the opposite convention. Calls that involve an LPI or TPI point are not
verified.

The corpus files recorded from the reference mesher use the opposite
convention for both predicates: the factor is -1 for orient3d and -1 for
insphere (`REFERENCE_ORIENT3D_FACTOR` and `REFERENCE_INSPHERE_FACTOR` in
`src/exact.rs`). That holds for every recorded call of every model of the
sample corpus.
