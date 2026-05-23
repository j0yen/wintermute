//! recall-ops — mutating + read-only operations on the v0.1 recall store.

#![cfg_attr(not(test), forbid(unsafe_code))]
#![allow(
    clippy::module_name_repetitions,
    clippy::doc_markdown,
    clippy::indexing_slicing,
    clippy::uninlined_format_args,
    clippy::option_if_let_else,
    clippy::single_match_else,
    clippy::or_fun_call,
    clippy::missing_errors_doc,
    clippy::manual_let_else
)]

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Result of touching one memory id.
#[derive(Debug, Clone, Serialize)]
pub struct TouchResult {
    /// Memory id.
    pub id: String,
    /// New recall_count after the bump.
    pub new_count: u64,
    /// New last_recalled_at timestamp.
    pub last_recalled_at: String,
}

/// Aggregated touch results.
#[derive(Debug, Clone, Serialize)]
pub struct TouchReport {
    /// Number of memories successfully touched.
    pub touched: usize,
    /// Per-id results.
    pub results: Vec<TouchResult>,
    /// Ids the caller asked for that weren't found.
    pub unknown: Vec<String>,
}

/// Touch one or more memory ids: bumps recall_count by 1 and sets last_recalled_at.
pub fn touch(root: &Path, ids: &[String]) -> std::io::Result<TouchReport> {
    let db = index_db_path(root);
    require_sqlite3()?;
    if !db.exists() {
        return Err(io_err(format!("index DB not found: {}", db.display())));
    }
    let now = Utc::now().to_rfc3339();
    let mut results = Vec::new();
    let mut unknown = Vec::new();
    for id in ids {
        let current = sqlite_scalar(
            &db,
            &format!("SELECT recall_count FROM memories_meta WHERE id='{}';", esc(id)),
        )?;
        if current.is_empty() {
            unknown.push(id.clone());
            continue;
        }
        let next: u64 = current.parse::<u64>().unwrap_or(0) + 1;
        let sql = format!(
            "UPDATE memories_meta SET recall_count={}, last_recalled_at='{}' WHERE id='{}';",
            next, now, esc(id)
        );
        sqlite_exec(&db, &sql)?;
        results.push(TouchResult {
            id: id.clone(),
            new_count: next,
            last_recalled_at: now.clone(),
        });
    }
    Ok(TouchReport {
        touched: results.len(),
        results,
        unknown,
    })
}

/// One GC candidate.
#[derive(Debug, Clone, Serialize)]
pub struct GcCandidate {
    /// Memory id.
    pub id: String,
    /// File path on disk.
    pub path: String,
    /// Effective freshness timestamp considered.
    pub last_seen: String,
}

/// GC dry-run result.
#[derive(Debug, Clone, Serialize)]
pub struct GcReport {
    /// Whether --apply was passed.
    pub applied: bool,
    /// Candidate count.
    pub count: usize,
    /// Per-candidate listing.
    pub candidates: Vec<GcCandidate>,
    /// Ids actually deleted (when applied).
    pub deleted: Vec<String>,
}

/// Garbage-collect memories older than the given duration.
pub fn gc(
    root: &Path,
    older_than: Duration,
    never_recalled: bool,
    apply: bool,
) -> std::io::Result<GcReport> {
    let db = index_db_path(root);
    require_sqlite3()?;
    if !db.exists() {
        return Err(io_err(format!("index DB not found: {}", db.display())));
    }
    let cutoff = (Utc::now() - older_than).to_rfc3339();
    let extra = if never_recalled { " AND recall_count = 0" } else { "" };
    let sql = format!(
        "SELECT id || '|' || path || '|' || COALESCE(last_recalled_at, created_at) FROM memories_meta WHERE COALESCE(last_recalled_at, created_at) < '{cutoff}'{extra} ORDER BY id;"
    );
    let rows = sqlite_exec_capture(&db, &sql)?;
    let candidates: Vec<GcCandidate> = rows
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '|');
            let id = parts.next()?.to_string();
            let path = parts.next()?.to_string();
            let last_seen = parts.next()?.to_string();
            Some(GcCandidate { id, path, last_seen })
        })
        .collect();
    let mut deleted = Vec::new();
    if apply {
        for c in &candidates {
            let _ = std::fs::remove_file(&c.path);
            let del = format!("DELETE FROM memories_meta WHERE id='{}';", esc(&c.id));
            sqlite_exec(&db, &del)?;
            deleted.push(c.id.clone());
        }
    }
    Ok(GcReport {
        applied: apply,
        count: candidates.len(),
        candidates,
        deleted,
    })
}

/// Aggregated stats report.
#[derive(Debug, Clone, Serialize)]
pub struct StatsReport {
    /// Total memory rows.
    pub total: usize,
    /// Count per subject (sorted alphabetically).
    pub by_subject: BTreeMap<String, usize>,
    /// Count per kind (sorted alphabetically).
    pub by_kind: BTreeMap<String, usize>,
    /// Earliest created_at across all memories (RFC3339 string or null).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oldest_created: Option<String>,
    /// Distinct embedding_id values (sorted, NULL excluded).
    pub distinct_embedder_ids: Vec<String>,
}

/// Compute stats over the recall store.
pub fn stats(root: &Path) -> std::io::Result<StatsReport> {
    let db = index_db_path(root);
    require_sqlite3()?;
    if !db.exists() {
        return Ok(StatsReport {
            total: 0,
            by_subject: BTreeMap::new(),
            by_kind: BTreeMap::new(),
            oldest_created: None,
            distinct_embedder_ids: Vec::new(),
        });
    }
    let total: usize = sqlite_scalar(&db, "SELECT COUNT(*) FROM memories_meta;")?
        .parse()
        .unwrap_or(0);
    let by_subject = group_count(&db, "subject")?;
    let by_kind = group_count(&db, "kind")?;
    let oldest_raw = sqlite_scalar(&db, "SELECT MIN(created_at) FROM memories_meta;")?;
    let oldest_created = if oldest_raw.is_empty() {
        None
    } else {
        Some(oldest_raw)
    };
    let emb_raw = sqlite_exec_capture(
        &db,
        "SELECT DISTINCT embedding_id FROM memories_meta WHERE embedding_id IS NOT NULL ORDER BY embedding_id;",
    )?;
    let distinct_embedder_ids: Vec<String> = emb_raw
        .lines()
        .map(str::to_string)
        .filter(|s| !s.is_empty())
        .collect();
    Ok(StatsReport {
        total,
        by_subject,
        by_kind,
        oldest_created,
        distinct_embedder_ids,
    })
}

fn group_count(db: &Path, col: &str) -> std::io::Result<BTreeMap<String, usize>> {
    let rows = sqlite_exec_capture(
        db,
        &format!("SELECT {col}, COUNT(*) FROM memories_meta GROUP BY {col};"),
    )?;
    let mut out = BTreeMap::new();
    for line in rows.lines() {
        if let Some((key, val)) = line.split_once('|') {
            let n: usize = val.parse().unwrap_or(0);
            out.insert(key.to_string(), n);
        }
    }
    Ok(out)
}

/// Lineage entry.
#[derive(Debug, Clone, Serialize)]
pub struct LineageEntry {
    /// Memory id.
    pub id: String,
    /// Kind.
    pub kind: String,
    /// Subject.
    pub subject: String,
    /// Created-at timestamp.
    pub created_at: String,
}

/// Lineage report: ancestors + descendants.
#[derive(Debug, Clone, Serialize)]
pub struct LineageReport {
    /// The starting memory.
    pub root_id: String,
    /// Memories the root claims to supersede (recursively).
    pub ancestors: Vec<LineageEntry>,
    /// Memories that claim to supersede the root (recursively).
    pub descendants: Vec<LineageEntry>,
}

/// Walk supersedes chains for the given id.
pub fn lineage(root: &Path, id: &str) -> std::io::Result<LineageReport> {
    let db = index_db_path(root);
    require_sqlite3()?;
    if !db.exists() {
        return Err(io_err(format!("index DB not found: {}", db.display())));
    }
    let mut ancestors = Vec::new();
    let mut visited = BTreeSet::new();
    let mut queue: Vec<String> = ancestors_of(&db, id)?;
    while let Some(next) = queue.pop() {
        if !visited.insert(next.clone()) {
            continue;
        }
        if let Some(entry) = lookup_entry(&db, &next)? {
            ancestors.push(entry);
            for a in ancestors_of(&db, &next)? {
                queue.push(a);
            }
        }
    }

    let mut descendants = Vec::new();
    let mut visited_d = BTreeSet::new();
    let mut queue: Vec<String> = descendants_of(&db, id)?;
    while let Some(next) = queue.pop() {
        if !visited_d.insert(next.clone()) {
            continue;
        }
        if let Some(entry) = lookup_entry(&db, &next)? {
            descendants.push(entry);
            for d in descendants_of(&db, &next)? {
                queue.push(d);
            }
        }
    }

    ancestors.sort_by(|a, b| a.id.cmp(&b.id));
    descendants.sort_by(|a, b| a.id.cmp(&b.id));

    Ok(LineageReport {
        root_id: id.to_string(),
        ancestors,
        descendants,
    })
}

fn ancestors_of(db: &Path, id: &str) -> std::io::Result<Vec<String>> {
    let raw = sqlite_scalar(
        db,
        &format!("SELECT COALESCE(supersedes_json, '[]') FROM memories_meta WHERE id='{}';", esc(id)),
    )?;
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    let parsed: Vec<String> = serde_json::from_str(&raw).unwrap_or_default();
    Ok(parsed)
}

fn descendants_of(db: &Path, id: &str) -> std::io::Result<Vec<String>> {
    let raw = sqlite_exec_capture(
        db,
        &format!(
            "SELECT id FROM memories_meta WHERE supersedes_json LIKE '%\"{}\"%' ORDER BY id;",
            esc(id)
        ),
    )?;
    Ok(raw.lines().map(str::to_string).filter(|s| !s.is_empty()).collect())
}

fn lookup_entry(db: &Path, id: &str) -> std::io::Result<Option<LineageEntry>> {
    let row = sqlite_exec_capture(
        db,
        &format!(
            "SELECT id || '|' || kind || '|' || subject || '|' || created_at FROM memories_meta WHERE id='{}';",
            esc(id)
        ),
    )?;
    let line = row.lines().next();
    let Some(line) = line else { return Ok(None) };
    let mut parts = line.splitn(4, '|');
    Ok(Some(LineageEntry {
        id: parts.next().unwrap_or("").to_string(),
        kind: parts.next().unwrap_or("").to_string(),
        subject: parts.next().unwrap_or("").to_string(),
        created_at: parts.next().unwrap_or("").to_string(),
    }))
}

/// Update one memory: body and/or confidence.
pub fn update(
    root: &Path,
    id: &str,
    body: Option<&str>,
    confidence: Option<f64>,
) -> std::io::Result<bool> {
    let db = index_db_path(root);
    require_sqlite3()?;
    if !db.exists() {
        return Err(io_err(format!("index DB not found: {}", db.display())));
    }
    let path_row = sqlite_scalar(&db, &format!("SELECT path FROM memories_meta WHERE id='{}';", esc(id)))?;
    if path_row.is_empty() {
        return Ok(false);
    }
    if let Some(b) = body {
        let path = PathBuf::from(&path_row);
        let original = std::fs::read_to_string(&path)?;
        let (frontmatter, _old_body) = split_frontmatter(&original);
        let new_content = if frontmatter.is_empty() {
            b.to_string()
        } else {
            format!("---\n{}\n---\n{b}\n", frontmatter.trim())
        };
        std::fs::write(&path, new_content)?;
    }
    if let Some(c) = confidence {
        let c_clamped = c.clamp(0.0, 1.0);
        let sql = format!(
            "UPDATE memories_meta SET confidence={} WHERE id='{}';",
            c_clamped, esc(id)
        );
        sqlite_exec(&db, &sql)?;
    }
    Ok(true)
}

fn split_frontmatter(content: &str) -> (String, String) {
    let after_open = if let Some(rest) = content.strip_prefix("---\n") {
        rest
    } else {
        return (String::new(), content.to_string());
    };
    if let Some(body) = after_open.strip_prefix("---\n") {
        return (String::new(), body.to_string());
    }
    if let Some(idx) = after_open.find("\n---\n") {
        let yaml = &after_open[..idx];
        let body = &after_open[idx + "\n---\n".len()..];
        return (yaml.to_string(), body.to_string());
    }
    (String::new(), content.to_string())
}

// --- helpers ---

fn index_db_path(root: &Path) -> PathBuf {
    root.join("index").join("recall.sqlite")
}

fn require_sqlite3() -> std::io::Result<()> {
    let ok = Command::new("sqlite3").arg("--version").output().is_ok_and(|o| o.status.success());
    if ok {
        Ok(())
    } else {
        Err(io_err("sqlite3 binary not available on PATH".to_string()))
    }
}

fn sqlite_exec(db: &Path, sql: &str) -> std::io::Result<()> {
    let out = Command::new("sqlite3").arg(db).arg(sql).output()?;
    if !out.status.success() {
        return Err(io_err(format!(
            "sqlite3 failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(())
}

fn sqlite_exec_capture(db: &Path, sql: &str) -> std::io::Result<String> {
    let out = Command::new("sqlite3").arg(db).arg(sql).output()?;
    if !out.status.success() {
        return Err(io_err(format!(
            "sqlite3 failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
}

fn sqlite_scalar(db: &Path, sql: &str) -> std::io::Result<String> {
    let raw = sqlite_exec_capture(db, sql)?;
    Ok(raw.lines().next().unwrap_or("").to_string())
}

fn esc(s: &str) -> String {
    s.replace('\'', "''")
}

fn io_err(msg: String) -> std::io::Error {
    std::io::Error::other(msg)
}

/// Parse a duration suffix string: 30d / 7d / 12h / 5m.
#[must_use]
pub fn parse_duration(s: &str) -> Option<Duration> {
    let t = s.trim();
    if t.len() < 2 {
        return None;
    }
    let (num_part, suffix) = t.split_at(t.len() - 1);
    let n: i64 = num_part.parse().ok()?;
    match suffix {
        "d" => Some(Duration::days(n)),
        "h" => Some(Duration::hours(n)),
        "m" => Some(Duration::minutes(n)),
        _ => None,
    }
}

/// Convenience type alias used by main.rs.
pub type Now = DateTime<Utc>;
