//! Reading and writing the corpus file format.

mod read;
mod write;

use std::fmt;

pub(crate) const MAGIC: &[u8; 8] = b"IPCORPUS";
pub(crate) const VERSION: u32 = 1;

/// Error reading, writing or validating a corpus.
#[derive(Debug)]
pub enum CorpusError {
    /// An I/O error, including a file that ends too early.
    Io(std::io::Error),
    /// The bytes do not follow the file format.
    Format(String),
    /// The content is well formed but violates a validity rule.
    Invalid(String),
}

impl fmt::Display for CorpusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CorpusError::Io(e) => write!(f, "I/O error: {e}"),
            CorpusError::Format(m) => write!(f, "malformed corpus file: {m}"),
            CorpusError::Invalid(m) => write!(f, "invalid corpus: {m}"),
        }
    }
}

impl std::error::Error for CorpusError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CorpusError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for CorpusError {
    fn from(e: std::io::Error) -> Self {
        CorpusError::Io(e)
    }
}
