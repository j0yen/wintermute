//! Error types for docket.

use thiserror::Error;

/// All errors that docket can produce.
#[derive(Debug, Error)]
pub enum DocketError {
    /// `SQLite` database error.
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    /// A finding key was not found.
    #[error("finding '{0}' not found")]
    NotFound(String),

    /// `JSON` serialization/deserialization error.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// I/O error (e.g. creating directories).
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// DB path could not be determined.
    #[error("could not determine database path: {0}")]
    DbPath(String),
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, DocketError>;
