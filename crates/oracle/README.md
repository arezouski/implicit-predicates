# implicit-predicates-oracle

Slow, exact reference implementation used to test
[`implicit-predicates`](https://crates.io/crates/implicit-predicates). It is
meant for tests only, not for production use.

> **Status: early development.** The API is limited to `Dyadic`, an exact
> dyadic rational number (arbitrary-precision mantissa times a power of two)
> with exact conversion from `f64`, addition, subtraction, multiplication,
> negation, sign and ordering.

## License

Licensed under either of Apache License, Version 2.0 or MIT license, at your
option.
