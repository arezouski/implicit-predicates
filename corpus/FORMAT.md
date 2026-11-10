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
four; the fifth reads back as 0.

A counter counts calls with one outcome: predicate, number of implicit
points among the call's points, returned sign, and deciding stage. A stratum
groups calls by predicate, implicit-point count and whether the sign is zero;
`total` is how many calls of the stratum were seen and `kept` how many are
stored in the file.

## Validity rules

A reader rejects a file unless all of the following hold:

- the magic and version are correct, and every code is known;
- the model name is valid UTF-8;
- the parents of a point have a smaller index than the point;
- the parents of an LNC point are explicit points;
- every coordinate and every LNC parameter is finite;
- every sign is -1, 0 or 1;
- every call index refers to an existing point;
- for each stratum, `kept` equals the number of stored calls of that stratum
  and does not exceed `total`;
- for each stratum, the counters of that stratum add up to its `total`;
- no counter or stratum is repeated;
- there are no bytes after the last call.

Readers never size allocations from the header counts: data is read first and
a truncated file is an error.

## LNC parameter

An LNC point lies on the line through its two parents `p` and `q`. The
parameter `t = 0` is the first parent `p` and `t = 1` is the second parent
`q`.

## Sign conventions

Signs are stored exactly as returned by the program that recorded the calls.
