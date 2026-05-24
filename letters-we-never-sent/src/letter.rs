//! Letter file resolution and state-mutation helpers.

use crate::error::{LetterError, LetterResult};
use crate::frontmatter::{self, LetterDoc};
use crate::state::State;
use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};

/// Resolve a user-supplied filename against `root`. Absolute paths pass
/// through unchanged; relative paths are joined onto `root`.
#[must_use]
pub fn resolve(filename: &str, root: &Path) -> PathBuf {
    let p = Path::new(filename);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        root.join(p)
    }
}

/// Read a letter file from disk.
///
/// # Errors
/// Returns `LetterError::NotFound` if the path does not exist, `LetterError::Io`
/// on read failure, or `LetterError::Unparseable` if frontmatter is malformed.
pub fn read(path: &Path) -> LetterResult<LetterDoc> {
    if !path.exists() {
        return Err(LetterError::NotFound(path.to_path_buf()));
    }
    let bytes = fs::read(path).map_err(|e| LetterError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    frontmatter::parse(&bytes, path)
}

/// Write a letter file back to disk.
///
/// # Errors
/// Returns `LetterError::Io` on write failure or `LetterError::Serialize`
/// if YAML encoding fails.
pub fn write(path: &Path, doc: &LetterDoc) -> LetterResult<()> {
    let bytes = frontmatter::serialize(doc)?;
    fs::write(path, &bytes).map_err(|e| LetterError::Io {
        path: path.to_path_buf(),
        source: e,
    })
}

/// Transition a letter to a new state. Updates `accepted_at` to now (UTC).
///
/// # Errors
/// Propagates read/parse/serialize/write errors from the underlying helpers.
pub fn transition(path: &Path, new_state: State) -> LetterResult<()> {
    let mut doc = read(path)?;
    doc.front.state = new_state;
    doc.front.accepted_at = Some(now_rfc3339());
    write(path, &doc)
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}
