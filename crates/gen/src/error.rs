//! Errors reported while reading a predicate definition.

use std::fmt;

use proc_macro2::Span;

/// An error found in a definition file, with the position of the offending
/// token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    /// Name of the file, as given by the caller.
    pub file: String,
    /// Line of the offending token, starting at 1.
    pub line: usize,
    /// Column of the offending token, starting at 1.
    pub column: usize,
    /// What is wrong.
    pub message: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}: {}",
            self.file, self.line, self.column, self.message
        )
    }
}

impl std::error::Error for Error {}

/// A problem at a position of the source, before the file name is known.
pub(crate) struct Diag {
    pub(crate) line: usize,
    pub(crate) column: usize,
    pub(crate) message: String,
}

pub(crate) type Res<T> = Result<T, Diag>;

/// Reports `message` at the first token of `span`.
pub(crate) fn diag(span: Span, message: impl Into<String>) -> Diag {
    let start = span.start();
    Diag {
        line: start.line,
        column: start.column + 1,
        message: message.into(),
    }
}
