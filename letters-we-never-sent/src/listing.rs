//! Recursive letter discovery, table formatting, and stats aggregation.

use crate::error::{LetterError, LetterResult};
use crate::frontmatter::{self, LetterDoc};
use crate::state::{State, StateFilter};
use std::fs;
use std::path::{Path, PathBuf};

/// A single row in `letter list` output.
#[derive(Debug, Clone)]
pub struct Entry {
    /// On-disk path of the letter file.
    pub path: PathBuf,
    /// The filename component (e.g. `01.md`).
    pub filename: String,
    /// `None` if the letter could not be parsed.
    pub doc: Option<LetterDoc>,
}

impl Entry {
    /// Effective state for filtering and display: `None` for unparseable.
    #[must_use]
    pub fn state(&self) -> Option<State> {
        self.doc.as_ref().map(|d| d.front.state)
    }

    /// Display state string (`(unparseable)` for parse failures).
    #[must_use]
    pub fn state_str(&self) -> &'static str {
        self.doc.as_ref().map_or("(unparseable)", |d| d.front.state.as_str())
    }

    /// Accepted-at column value (`-` when missing).
    #[must_use]
    pub fn accepted_at_col(&self) -> &str {
        self.doc
            .as_ref()
            .and_then(|d| d.front.accepted_at.as_deref())
            .unwrap_or("-")
    }

    /// Subject column: explicit `subject` field, else first H1, else filename.
    #[must_use]
    pub fn subject_col(&self) -> String {
        if let Some(doc) = &self.doc {
            if let Some(s) = &doc.front.subject {
                return s.clone();
            }
            if let Some(h1) = first_h1(&doc.body) {
                return h1;
            }
        }
        self.filename.clone()
    }
}

fn first_h1(body: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(body).ok()?;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("# ") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

/// Recursively list `.md` files under `root`, optionally restricted to a
/// year subdirectory.
///
/// # Errors
/// Returns `LetterError::Io` if directory enumeration fails.
pub fn collect(root: &Path, year: Option<i32>) -> LetterResult<Vec<Entry>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    walk(root, &mut out)?;
    if let Some(y) = year {
        out.retain(|e| keep_for_year(e, y));
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

fn keep_for_year(entry: &Entry, year: i32) -> bool {
    let by_frontmatter = entry.doc.as_ref().is_some_and(|d| {
        d.front
            .month
            .as_deref()
            .and_then(|m| m.get(..4))
            .and_then(|s| s.parse::<i32>().ok())
            == Some(year)
    });
    let by_dir = entry
        .path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .and_then(|n| n.parse::<i32>().ok())
        == Some(year);
    by_frontmatter || by_dir
}

fn walk(dir: &Path, out: &mut Vec<Entry>) -> LetterResult<()> {
    let rd = fs::read_dir(dir).map_err(|e| LetterError::Io {
        path: dir.to_path_buf(),
        source: e,
    })?;
    for entry in rd {
        let entry = entry.map_err(|e| LetterError::Io {
            path: dir.to_path_buf(),
            source: e,
        })?;
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out)?;
        } else if path.is_file() && path.extension().is_some_and(|e| e == "md") {
            push_entry(&path, out);
        }
    }
    Ok(())
}

fn push_entry(path: &Path, out: &mut Vec<Entry>) {
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();
    let doc = fs::read(path).ok().and_then(|b| frontmatter::parse(&b, path).ok());
    out.push(Entry {
        path: path.to_path_buf(),
        filename,
        doc,
    });
}

/// Apply a `StateFilter` to a list of entries. Unparseable entries always
/// pass through (so the user sees them).
#[must_use]
pub fn filter(entries: Vec<Entry>, filter: StateFilter) -> Vec<Entry> {
    entries
        .into_iter()
        .filter(|e| e.state().is_none_or(|s| filter.matches(s)))
        .collect()
}

/// Aggregate state counts for the `stats` subcommand.
#[derive(Debug, Default, Clone, Copy)]
pub struct Stats {
    /// Accepted count.
    pub accepted: usize,
    /// Declined count.
    pub declined: usize,
    /// Pending count.
    pub pending: usize,
    /// Marked-for-real count.
    pub send_real: usize,
    /// Total entries seen (includes unparseable).
    pub total: usize,
}

impl Stats {
    /// Build a `Stats` from an entry list.
    #[must_use]
    pub fn from_entries(entries: &[Entry]) -> Self {
        let mut s = Self::default();
        for e in entries {
            s.total = s.total.saturating_add(1);
            match e.state() {
                Some(State::Accepted) => s.accepted = s.accepted.saturating_add(1),
                Some(State::Declined) => s.declined = s.declined.saturating_add(1),
                Some(State::Pending) => s.pending = s.pending.saturating_add(1),
                Some(State::SendReal) => s.send_real = s.send_real.saturating_add(1),
                None => {}
            }
        }
        s
    }
}
