//! Path resolution. Honors `TRANSCRIPT_HOME` and `CLAUDE_PROJECTS` for tests.

use anyhow::{Context, Result};
use std::path::PathBuf;

/// Root of the transcript data dir. `$TRANSCRIPT_HOME` if set, else
/// `~/.claude/transcript`.
pub fn root() -> Result<PathBuf> {
    if let Ok(custom) = std::env::var("TRANSCRIPT_HOME") {
        return Ok(PathBuf::from(custom));
    }
    let dirs = directories::BaseDirs::new().context("could not resolve user home directory")?;
    Ok(dirs.home_dir().join(".claude").join("transcript"))
}

/// Root containing per-project JSONL directories. `$CLAUDE_PROJECTS` if set,
/// else `~/.claude/projects`.
pub fn projects_root() -> Result<PathBuf> {
    if let Ok(custom) = std::env::var("CLAUDE_PROJECTS") {
        return Ok(PathBuf::from(custom));
    }
    let dirs = directories::BaseDirs::new().context("could not resolve user home directory")?;
    Ok(dirs.home_dir().join(".claude").join("projects"))
}

pub fn index_dir(root: &std::path::Path) -> PathBuf {
    root.join("index")
}

pub fn index_db(root: &std::path::Path) -> PathBuf {
    index_dir(root).join("transcript.sqlite")
}
