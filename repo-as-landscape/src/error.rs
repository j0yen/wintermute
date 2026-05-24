//! Error type for the walker.

use std::path::PathBuf;

use thiserror::Error;

/// Errors that can occur while walking a repository.
#[derive(Debug, Error)]
pub enum Error {
    /// The supplied path is not a git repository.
    #[error("{path}: not a git repository")]
    NotARepository {
        /// Path the user supplied.
        path: PathBuf,
    },
    /// HEAD could not be resolved (e.g. empty repository).
    #[error("{path}: HEAD could not be resolved: {source}")]
    HeadUnresolved {
        /// Path the user supplied.
        path: PathBuf,
        /// Underlying libgit2 error.
        #[source]
        source: git2::Error,
    },
    /// A libgit2 operation failed.
    #[error("git error: {0}")]
    Git(#[from] git2::Error),
    /// A filesystem operation failed.
    #[error("io error at {path}: {source}")]
    Io {
        /// Path being operated on when the error occurred.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// JSON serialisation failed.
    #[error("serialise error: {0}")]
    Serialize(#[from] serde_json::Error),
}
