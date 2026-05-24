//! Error types for the letters-we-never-sent CLI.

use std::path::PathBuf;
use thiserror::Error;

/// All recoverable errors surfaced by the library.
#[derive(Debug, Error)]
pub enum LetterError {
    /// An I/O operation failed.
    #[error("io error at {path}: {source}")]
    Io {
        /// The path whose access failed.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// The file has no frontmatter or the YAML block could not be parsed.
    #[error("unparseable letter {path}: {reason}")]
    Unparseable {
        /// The offending file.
        path: PathBuf,
        /// Human-readable parse reason.
        reason: String,
    },

    /// The user requested an action on a file that does not exist.
    #[error("letter not found: {0}")]
    NotFound(PathBuf),

    /// Serializing the frontmatter back to YAML failed.
    #[error("frontmatter serialize error: {0}")]
    Serialize(String),
}

/// Convenience result alias.
pub type LetterResult<T> = Result<T, LetterError>;

impl LetterError {
    /// The intended process exit code for this error.
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::Unparseable { .. } => 3,
            Self::NotFound(_) => 2,
            Self::Io { .. } | Self::Serialize(_) => 1,
        }
    }
}
