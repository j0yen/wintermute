//! recall-doctor — health-check for the v0.1 recall memory store.
//!
//! Computes file_count (walking <root>/memories/), indexed_count (via shell-out
//! to sqlite3), orphans (files-without-index), missing (index-without-files),
//! embedder_ids (distinct embedding_id values), schema_version.

#![cfg_attr(not(test), forbid(unsafe_code))]
#![allow(
    clippy::module_name_repetitions,
    clippy::option_if_let_else,
    clippy::single_match_else,
    clippy::doc_markdown,
    clippy::indexing_slicing,
    clippy::uninlined_format_args,
    clippy::type_complexity,
    clippy::or_fun_call,
    clippy::unnecessary_wraps
)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

/// Frontmatter fields read from a memory file.
#[derive(Debug, Clone, Deserialize)]
struct Frontmatter {
    id: Option<String>,
}

/// Full doctor report.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// .md files counted under <root>/memories/.
    pub file_count: usize,
    /// Rows in memories_meta; null on DB or sqlite3 absence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexed_count: Option<usize>,
    /// IDs on disk not in the index, sorted.
    pub orphans: Vec<String>,
    /// IDs in the index whose `path` doesn't exist, sorted.
    pub missing: Vec<String>,
    /// Distinct embedding_id values from the index (None excluded), sorted.
    pub embedder_ids: Vec<String>,
    /// Schema version reported by `PRAGMA user_version` or fallback string.
    pub schema_version: String,
    /// Non-fatal warnings (missing DB, no sqlite3 on PATH, etc.).
    pub warnings: Vec<String>,
}

impl Report {
    /// True iff there's nothing diverged. Two clean shapes:
    ///   - completely empty root (no files AND no index) → trivially clean
    ///   - both file_count and indexed_count present and equal, with orphans+missing empty
    #[must_use]
    pub fn is_clean(&self) -> bool {
        if !self.orphans.is_empty() || !self.missing.is_empty() {
            return false;
        }
        match self.indexed_count {
            None => self.file_count == 0,
            Some(n) => n == self.file_count,
        }
    }
}

/// Run all checks against a recall root.
///
/// # Errors
/// Returns `io::Error` if the root is unreadable.
pub fn doctor(root: &Path) -> std::io::Result<Report> {
    let mut warnings = Vec::new();
    let memories_dir = root.join("memories");
    let index_db = root.join("index").join("recall.sqlite");

    let on_disk = if memories_dir.exists() {
        walk_memory_ids(&memories_dir)?
    } else {
        BTreeSet::new()
    };
    let file_count = on_disk.len();

    let (indexed, missing_paths, embedder_ids, schema_version) =
        sqlite_probe(&index_db, &mut warnings);
    let indexed_count = indexed.as_ref().map(BTreeSet::len);
    let orphans = indexed
        .as_ref()
        .map(|ix| on_disk.difference(ix).cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    let missing_ids = indexed
        .as_ref()
        .map(|ix| ix.difference(&on_disk).cloned().collect::<Vec<_>>())
        .unwrap_or_default();

    let mut missing_combined: BTreeSet<String> = missing_ids.into_iter().collect();
    for p in missing_paths {
        missing_combined.insert(p);
    }
    let missing: Vec<String> = missing_combined.into_iter().collect();

    Ok(Report {
        file_count,
        indexed_count,
        orphans,
        missing,
        embedder_ids,
        schema_version,
        warnings,
    })
}

fn walk_memory_ids(memories_dir: &Path) -> std::io::Result<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    let walker = walkdir::WalkDir::new(memories_dir)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !(e.depth() > 0 && e.file_type().is_dir() && name.starts_with('.'))
        });
    for entry in walker {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().is_none_or(|x| x != "md") {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Some(id) = parse_id(&content) {
                out.insert(id);
            }
        }
    }
    Ok(out)
}

fn parse_id(content: &str) -> Option<String> {
    let after = content.strip_prefix("---\n")?;
    let close_idx = after.find("\n---")?;
    let yaml = &after[..close_idx];
    let fm: Frontmatter = serde_yml::from_str(yaml).ok()?;
    fm.id
}

fn sqlite_probe(
    db: &Path,
    warnings: &mut Vec<String>,
) -> (Option<BTreeSet<String>>, Vec<String>, Vec<String>, String) {
    if !db.exists() {
        warnings.push(format!("index DB not found at {}", db.display()));
        return (None, Vec::new(), Vec::new(), "v0.1-undeclared".to_string());
    }
    if !sqlite3_available() {
        warnings.push("sqlite3 binary not on PATH; indexed metrics unavailable".to_string());
        return (None, Vec::new(), Vec::new(), "v0.1-undeclared".to_string());
    }

    let ids_csv = sqlite_query(db, "SELECT id FROM memories_meta;").unwrap_or_default();
    let ids: BTreeSet<String> = ids_csv.lines().map(str::to_string).collect();

    let paths_csv = sqlite_query(db, "SELECT id, path FROM memories_meta;").unwrap_or_default();
    let mut missing_paths = Vec::new();
    for line in paths_csv.lines() {
        if let Some((id, path)) = line.split_once('|') {
            if !Path::new(path).exists() {
                missing_paths.push(id.to_string());
            }
        }
    }

    let embedder_csv =
        sqlite_query(db, "SELECT DISTINCT embedding_id FROM memories_meta WHERE embedding_id IS NOT NULL;")
            .unwrap_or_default();
    let mut embedder_ids: Vec<String> = embedder_csv.lines().map(str::to_string).filter(|s| !s.is_empty()).collect();
    embedder_ids.sort();

    let user_version = sqlite_query(db, "PRAGMA user_version;")
        .unwrap_or_default()
        .lines()
        .next()
        .unwrap_or("0")
        .to_string();
    let schema_version = if user_version == "0" || user_version.is_empty() {
        "v0.1-undeclared".to_string()
    } else {
        format!("user_version={user_version}")
    };

    (Some(ids), missing_paths, embedder_ids, schema_version)
}

fn sqlite3_available() -> bool {
    Command::new("sqlite3")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

fn sqlite_query(db: &Path, sql: &str) -> Option<String> {
    let out = Command::new("sqlite3").arg(db).arg(sql).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
}

/// Render a report as deterministic JSON.
///
/// # Errors
/// Returns serde error on serialization failure.
pub fn render_json(report: &Report) -> serde_json::Result<String> {
    serde_json::to_string(report)
}

/// Render a report as human-readable text.
#[must_use]
pub fn render_text(report: &Report) -> String {
    let mut s = String::new();
    s.push_str(&format!("file_count: {}\n", report.file_count));
    s.push_str(&format!(
        "indexed_count: {}\n",
        report.indexed_count.map_or_else(|| "null".to_string(), |n| n.to_string())
    ));
    s.push_str(&format!("schema_version: {}\n", report.schema_version));
    s.push_str(&format!("embedder_ids: [{}]\n", report.embedder_ids.join(", ")));
    s.push_str(&format!("orphans ({}):\n", report.orphans.len()));
    for o in &report.orphans {
        s.push_str(&format!("  {o}\n"));
    }
    s.push_str(&format!("missing ({}):\n", report.missing.len()));
    for m in &report.missing {
        s.push_str(&format!("  {m}\n"));
    }
    for w in &report.warnings {
        s.push_str(&format!("warning: {w}\n"));
    }
    s
}

/// Invoke `recall reindex` to attempt a fix. Returns the child's exit status.
///
/// # Errors
/// Returns io::Error if `recall` isn't on PATH or the invocation fails.
pub fn invoke_reindex(root: &Path) -> std::io::Result<std::process::ExitStatus> {
    Command::new("recall")
        .args(["reindex", "--root"])
        .arg(root)
        .status()
}
