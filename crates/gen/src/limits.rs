//! The one limit on a definition: how deeply brackets may nest.
//!
//! Parenthesized expressions are the only construct the parser recurses on, so
//! the depth of brackets bounds its stack. The lexer of `proc_macro2` builds
//! nested token trees, and so does its destructor, so the depth is also
//! checked on the characters of the source before lexing, with comments and
//! literals skipped. Every other chain (sums, products, unary minus) is read in
//! a loop and has no limit apart from memory.

use crate::error::{Diag, Res};

/// Deepest nesting of brackets: `(`, `[` and `{`.
pub(crate) const MAX_NESTING: usize = 32;

/// The message of the error for brackets nested deeper than [`MAX_NESTING`].
pub(crate) fn too_deep() -> String {
    format!("brackets are nested more than {MAX_NESTING} deep; split the expression with `let`")
}

/// Checks that brackets nest at most [`MAX_NESTING`] deep.
///
/// Comments (including nested block comments), string and character literals
/// and raw strings are skipped, so brackets inside them do not count.
pub(crate) fn check_nesting(source: &str) -> Res<()> {
    let chars: Vec<char> = source.chars().collect();
    let (mut i, mut depth) = (0, 0usize);
    let (mut line, mut column) = (1usize, 1usize);
    while i < chars.len() {
        let start = i;
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        match c {
            '(' | '[' | '{' => {
                depth += 1;
                if depth > MAX_NESTING {
                    return Err(Diag {
                        line,
                        column,
                        message: too_deep(),
                    });
                }
                i += 1;
            }
            ')' | ']' | '}' => {
                depth = depth.saturating_sub(1);
                i += 1;
            }
            '/' if next == Some('/') => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            '/' if next == Some('*') => i = skip_block_comment(&chars, i),
            '"' => i = skip_quoted(&chars, i, '"'),
            '\'' => i = skip_char_or_lifetime(&chars, i),
            _ if starts_raw_string(&chars, i) => i = skip_raw_string(&chars, i),
            _ if is_word(c) => {
                while i < chars.len() && is_word(chars[i]) {
                    i += 1;
                }
            }
            _ => i += 1,
        }
        i = i.min(chars.len());
        for &c in &chars[start..i] {
            if c == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }
    }
    Ok(())
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Index after the block comment that starts at `i`, honoring nesting.
fn skip_block_comment(chars: &[char], mut i: usize) -> usize {
    let mut depth = 0usize;
    while i < chars.len() {
        match (chars[i], chars.get(i + 1)) {
            ('/', Some('*')) => {
                depth += 1;
                i += 2;
            }
            ('*', Some('/')) => {
                depth -= 1;
                i += 2;
                if depth == 0 {
                    return i;
                }
            }
            _ => i += 1,
        }
    }
    i
}

/// Index after the literal that starts at `i` with the quote `quote`, honoring
/// backslash escapes.
fn skip_quoted(chars: &[char], mut i: usize, quote: char) -> usize {
    i += 1;
    while i < chars.len() {
        match chars[i] {
            '\\' => i += 2,
            c if c == quote => return i + 1,
            _ => i += 1,
        }
    }
    i
}

/// Index after the character literal or lifetime that starts at `i`.
fn skip_char_or_lifetime(chars: &[char], i: usize) -> usize {
    match (chars.get(i + 1), chars.get(i + 2)) {
        (Some('\\'), _) => skip_quoted(chars, i, '\''),
        (Some(_), Some('\'')) => i + 3,
        _ => i + 1,
    }
}

/// Tells whether a raw string such as `r"..."`, `br#"..."#` or `cr"..."`
/// starts at `i`.
fn starts_raw_string(chars: &[char], i: usize) -> bool {
    if i > 0 && is_word(chars[i - 1]) {
        return false;
    }
    let mut j = i;
    if matches!(chars.get(j), Some('b' | 'c')) {
        j += 1;
    }
    if chars.get(j) != Some(&'r') {
        return false;
    }
    j += 1;
    while chars.get(j) == Some(&'#') {
        j += 1;
    }
    chars.get(j) == Some(&'"')
}

/// Index after the raw string that starts at `i`.
fn skip_raw_string(chars: &[char], mut i: usize) -> usize {
    while chars[i] != 'r' {
        i += 1;
    }
    i += 1;
    let mut hashes = 0;
    while chars[i] == '#' {
        hashes += 1;
        i += 1;
    }
    i += 1;
    while i < chars.len() {
        if chars[i] == '"' && (1..=hashes).all(|k| chars.get(i + k) == Some(&'#')) {
            return i + 1 + hashes;
        }
        i += 1;
    }
    i
}
