//! Bounds on the size and nesting of a definition.
//!
//! `syn` and the evaluator recurse on the shape of the syntax tree, and the
//! token trees of `proc_macro2` are cloned recursively, so deeply nested
//! brackets or a long chain of operators could exhaust the stack. Bracket
//! depth is checked on the characters of the source, before any lexing, with
//! comments and literals skipped. Statement size is checked on the token
//! stream, where those are already single tokens, before anything is parsed.

use proc_macro2::{TokenStream, TokenTree, token_stream::IntoIter};

use crate::error::{Diag, Res, diag};

/// Deepest nesting of brackets: `(`, `[` and `{`.
pub(crate) const MAX_NESTING: usize = 64;

/// Most tokens in one statement. A statement ends at `;`; the contents of a
/// bracket pair count as their own statement and the pair itself as one token.
pub(crate) const MAX_STATEMENT_TOKENS: usize = 6000;

/// Deepest expression tree. A chain of binary operators is as deep as it is
/// long, so this is also the longest chain of terms in one expression.
pub(crate) const MAX_EXPR_DEPTH: usize = 1000;

/// Stack size of the thread that parses a definition. Parsing and evaluation
/// recurse once per level of the syntax tree, and a debug build spends several
/// kilobytes per level, so the work runs on its own thread with room for the
/// deepest tree the other limits allow. Only the pages that get used are
/// committed.
pub(crate) const PARSER_STACK_BYTES: usize = 256 * 1024 * 1024;

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
                        message: "brackets are nested too deeply".into(),
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

/// Checks that no statement has more than [`MAX_STATEMENT_TOKENS`] tokens.
pub(crate) fn check_statements(tokens: TokenStream) -> Res<()> {
    let mut stack: Vec<(IntoIter, usize)> = vec![(tokens.into_iter(), 0)];
    while let Some((iter, count)) = stack.last_mut() {
        let Some(tree) = iter.next() else {
            stack.pop();
            continue;
        };
        if matches!(&tree, TokenTree::Punct(p) if p.as_char() == ';') {
            *count = 0;
            continue;
        }
        *count += 1;
        if *count > MAX_STATEMENT_TOKENS {
            return Err(diag(tree.span(), "the statement has too many tokens"));
        }
        if let TokenTree::Group(group) = tree {
            stack.push((group.stream().into_iter(), 0));
        }
    }
    Ok(())
}
