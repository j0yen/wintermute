//! Error type for the extractor.

use std::path::PathBuf;

use thiserror::Error;

/// Errors that can occur while extracting per-file history.
#[derive(Debug, Error)]
pub enum Error {
    /// The supplied path is not a git repository.
    #[error("{path}: not a git repository")]
    NotARepository {
        /// Path the user supplied.
        path: PathBuf,
    },
    /// The file never existed under any name in the repository's
    /// reachable history.
    #[error("{path}: no commits found")]
    NoCommitsFound {
        /// Repo-relative path the user asked about.
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
    /// JSON serialisation failed.
    #[error("serialise error: {0}")]
    Serialize(#[from] serde_json::Error),
}
