//! A cursor over the token trees of one bracket level.

use proc_macro2::{Span, TokenStream, TokenTree};

use crate::error::{Diag, diag};

/// The tokens of one level of brackets and a position in them.
pub(crate) struct Cursor {
    tokens: Vec<TokenTree>,
    pos: usize,
    close: Span,
}

impl Cursor {
    /// Reads `stream`. `close` is where an error is reported when the tokens
    /// run out; without it the last token is used.
    pub(crate) fn new(stream: TokenStream, close: Option<Span>) -> Self {
        let tokens: Vec<TokenTree> = stream.into_iter().collect();
        let close = close
            .or_else(|| tokens.last().map(TokenTree::span))
            .unwrap_or_else(Span::call_site);
        Cursor {
            tokens,
            pos: 0,
            close,
        }
    }

    /// The next token, if any.
    pub(crate) fn peek(&self) -> Option<&TokenTree> {
        self.tokens.get(self.pos)
    }

    /// The token `n` places after the next one, if any.
    pub(crate) fn peek_at(&self, n: usize) -> Option<&TokenTree> {
        self.tokens.get(self.pos + n)
    }

    /// Moves past the next token.
    pub(crate) fn bump(&mut self) {
        self.pos += 1;
    }

    /// Tells whether every token has been read.
    pub(crate) fn at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    /// The position where the tokens end.
    pub(crate) fn close(&self) -> Span {
        self.close
    }

    /// Tells whether the next token is the punctuation mark `c`.
    pub(crate) fn peek_punct(&self, c: char) -> bool {
        is_punct(self.peek(), c)
    }

    /// Tells whether the next token is the word `word`.
    pub(crate) fn peek_word(&self, word: &str) -> bool {
        matches!(self.peek(), Some(TokenTree::Ident(id)) if id == word)
    }

    /// An error at the next token, or at the end, saying what was expected.
    pub(crate) fn expected(&self, what: &str) -> Diag {
        match self.peek() {
            None => diag(self.close, format!("expected {what}")),
            Some(TokenTree::Punct(p)) if p.as_char() == '/' => {
                diag(p.span(), "division is not supported")
            }
            Some(token) => diag(
                token.span(),
                format!("unexpected {}, expected {what}", describe(token)),
            ),
        }
    }
}

/// Tells whether `token` is the punctuation mark `c`.
pub(crate) fn is_punct(token: Option<&TokenTree>, c: char) -> bool {
    matches!(token, Some(TokenTree::Punct(p)) if p.as_char() == c)
}

/// Names a token for an error message without printing the contents of a
/// bracket pair or a long literal.
fn describe(token: &TokenTree) -> String {
    match token {
        TokenTree::Group(g) => {
            let open = match g.delimiter() {
                proc_macro2::Delimiter::Parenthesis => "`(`",
                proc_macro2::Delimiter::Brace => "`{`",
                proc_macro2::Delimiter::Bracket => "`[`",
                proc_macro2::Delimiter::None => "group",
            };
            format!("token {open}")
        }
        other => {
            let text: String = other.to_string().chars().take(24).collect();
            format!("token `{text}`")
        }
    }
}
