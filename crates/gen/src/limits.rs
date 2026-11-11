//! Bounds on the size and vocabulary of a definition.
//!
//! `syn` recurses on the shape of the syntax tree and the token trees of
//! `proc_macro2` are cloned recursively, so deeply nested brackets or long
//! chains could exhaust the stack of the caller. The bounds are chosen so that
//! the worst input they accept fits in a 1 MiB stack of a debug build, half of
//! the default stack of a spawned thread. They were sized from the depth at
//! which each construct overflowed that stack:
//!
//! | construct                         | overflows at (1 MiB, debug) |
//! | --------------------------------- | --------------------------- |
//! | nested brackets                   | about 44 to 70 levels       |
//! | right-nested `=`                  | about 116 levels            |
//! | chain of unary operators          | about 100 to 230 levels     |
//! | left-deep `+ - *`, `.x` chains    | about 8000 levels           |
//!
//! Bracket depth is checked on the characters of the source, before any
//! lexing, with comments and literals skipped. The rest is checked on the
//! token stream, where those are already single tokens, before parsing.

use proc_macro2::{TokenStream, TokenTree, token_stream::IntoIter};

use crate::error::{Diag, Res, diag};

/// Deepest nesting of brackets: `(`, `[` and `{`.
pub(crate) const MAX_NESTING: usize = 16;

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
                        message: format!(
                            "brackets are nested more than {MAX_NESTING} deep; split the expression with `let`"
                        ),
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

/// Most `+`, `-`, `*` and `>` tokens on the path from a statement to a leaf.
pub(crate) const MAX_OPERATORS: usize = 600;

/// Most `.` tokens on the path from a statement to a leaf.
pub(crate) const MAX_DOTS: usize = 320;

/// Most `=` tokens on the path from a statement to a leaf.
pub(crate) const MAX_ASSIGNMENTS: usize = 4;

/// Most punctuation tokens in a row.
pub(crate) const MAX_PUNCT_RUN: usize = 2;

/// Words of Rust that a definition may not use. Only `fn` and `let` are part
/// of the language of definitions.
const RESERVED: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "crate",
    "do", "dyn", "else", "enum", "extern", "false", "final", "for", "gen", "if", "impl", "in",
    "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref", "return",
    "self", "Self", "static", "struct", "super", "trait", "true", "try", "type", "typeof",
    "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

/// Counts of the tokens that deepen the syntax tree.
#[derive(Clone, Copy, Default)]
struct Counts {
    operators: usize,
    dots: usize,
    assignments: usize,
}

/// Checks the vocabulary of the token stream and bounds the tokens that nest.
///
/// Only the tokens that the language of definitions needs are accepted, so
/// the parser cannot meet a construct such as `return`, `as` or `!` that it
/// would have to recurse on. Of the rest, a statement may hold at most
/// [`MAX_OPERATORS`] operators, [`MAX_DOTS`] dots and [`MAX_ASSIGNMENTS`]
/// equal signs, counting those of the statements that enclose it, and at most
/// [`MAX_PUNCT_RUN`] punctuation tokens in a row. Each token deepens the
/// syntax tree by at most one level, so the bounds hold by construction.
pub(crate) fn check_tokens(tokens: TokenStream) -> Res<()> {
    let split = "split the expression with `let`";
    // Each frame: the tokens still to read, the counts at its start and now.
    let mut stack: Vec<(IntoIter, Counts, Counts)> =
        vec![(tokens.into_iter(), Counts::default(), Counts::default())];
    let mut run = 0usize;
    while let Some((iter, base, cur)) = stack.last_mut() {
        let Some(tree) = iter.next() else {
            stack.pop();
            run = 0;
            continue;
        };
        let (mut over, mut is_punct) = (None, false);
        match &tree {
            TokenTree::Ident(id) => {
                let name = id.to_string();
                if RESERVED.contains(&name.as_str()) {
                    return Err(diag(id.span(), format!("`{name}` is not supported")));
                }
            }
            TokenTree::Punct(p) => {
                is_punct = true;
                match p.as_char() {
                    ';' => {
                        *cur = *base;
                        is_punct = false;
                    }
                    ',' | ':' | '#' => {}
                    '+' | '-' | '*' | '>' => {
                        cur.operators += 1;
                        if cur.operators > MAX_OPERATORS {
                            over = Some(format!("more than {MAX_OPERATORS} operators"));
                        }
                    }
                    '.' => {
                        cur.dots += 1;
                        if cur.dots > MAX_DOTS {
                            over = Some(format!("more than {MAX_DOTS} field accesses"));
                        }
                    }
                    '=' => {
                        cur.assignments += 1;
                        if cur.assignments > MAX_ASSIGNMENTS {
                            over = Some(format!("more than {MAX_ASSIGNMENTS} equal signs"));
                        }
                    }
                    '/' => return Err(diag(p.span(), "division is not supported")),
                    c => return Err(diag(p.span(), format!("`{c}` is not supported"))),
                }
            }
            TokenTree::Literal(_) | TokenTree::Group(_) => {}
        }
        if let Some(what) = over {
            return Err(diag(
                tree.span(),
                format!("the statement has {what}; {split}"),
            ));
        }
        run = if is_punct { run + 1 } else { 0 };
        if run > MAX_PUNCT_RUN {
            return Err(diag(
                tree.span(),
                format!("too many operators in a row; {split}"),
            ));
        }
        if let TokenTree::Group(group) = tree {
            let here = *cur;
            stack.push((group.stream().into_iter(), here, here));
            run = 0;
        }
    }
    Ok(())
}
