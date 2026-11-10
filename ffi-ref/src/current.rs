//! The current Indirect_Predicates library.
//!
//! Like the legacy library, it returns the opposite of our sign for both
//! orient3d and insphere. An LNC point with explicit parents is built with
//! the library's LNC constructor, which takes the same parents and
//! parameter, with `t = 0` at the first parent.

reference_points!(crate::sys::Current, -1, -1);
