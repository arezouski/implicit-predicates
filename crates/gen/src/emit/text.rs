//! Helpers for writing generated text: comments, and `f64` literals.

use std::fmt::Write;

/// The width the generated comments are wrapped at, indentation included.
const COMMENT_WIDTH: usize = 80;

/// Writes `text` as comment lines that start with `marker` (`///`, `//!` or
/// `//`), indented by `indent` spaces as rustfmt places them, wrapped at
/// [`COMMENT_WIDTH`]; a blank line in `text` separates paragraphs.
pub(crate) fn comment(out: &mut String, marker: &str, indent: usize, text: &str) {
    let room = COMMENT_WIDTH.saturating_sub(indent + marker.len() + 1);
    for (i, paragraph) in text.split("\n\n").enumerate() {
        if i > 0 {
            writeln!(out, "{marker}").unwrap();
        }
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            if !line.is_empty() && line.len() + 1 + word.len() > room {
                writeln!(out, "{marker} {line}").unwrap();
                line.clear();
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        writeln!(out, "{marker} {line}").unwrap();
    }
}

/// The exponent of a normal power of two.
pub(crate) fn power_of_two(x: f64) -> i64 {
    debug_assert!(x.is_normal() && x.to_bits() & ((1 << 52) - 1) == 0);
    i64::try_from(x.to_bits() >> 52).expect("11 bits") - 1023
}

/// An `f64` literal that reads back as exactly `x`, which must be finite:
/// the shortest decimal that rounds to it.
pub(crate) fn literal(x: f64) -> String {
    debug_assert!(x.is_finite());
    format!("{x:?}")
}
