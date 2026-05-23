//! recall-io — NDJSON export + import for the v0.1 recall store.

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

use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

/// One memory record on the wire (both export and import).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    /// Memory id (ULID).
    pub id: String,
    /// Memory kind.
    pub kind: String,
    /// Memory subject (e.g. user, self, project:foo).
    pub subject: String,
    /// Path on disk (relative or absolute as stored).
    pub path: String,
    /// Confidence [0,1].
    #[serde(default)]
    pub confidence: f64,
    /// RFC3339 created timestamp.
    pub created_at: String,
    /// RFC3339 last-recalled timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_recalled_at: Option<String>,
    /// Recall count.
    #[serde(default)]
    pub recall_count: u64,
    /// Embedder id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding_id: Option<String>,
    /// JSON array of superseded ids (stored as text in the index).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes_json: Option<String>,
    /// Body (post-frontmatter).
    #[serde(default)]
    pub body: String,
}

/// Summary of an import run.
#[derive(Debug, Clone, Serialize)]
pub struct ImportSummary {
    /// Records successfully written.
    pub imported: usize,
    /// Records skipped because the id already exists (no --replace).
    pub skipped: usize,
    /// Lines that failed to parse or validate.
    pub errors: usize,
}

/// Export every memory to a writer, one NDJSON record per line.
pub fn export<W: std::io::Write>(root: &Path, w: &mut W) -> std::io::Result<usize> {
    let db = root.join("index").join("recall.sqlite");
    if !db.exists() {
        return Ok(0);
    }
    require_sqlite3()?;
    // sqlite3 default output uses `|` as field separator. We use that and assume
    // recall paths don't contain pipes (safe in practice for ~/.claude/recall/).
    let rows = sqlite_exec_capture(
        &db,
        "SELECT id, kind, subject, path, COALESCE(confidence, 0.5), created_at, COALESCE(last_recalled_at, ''), COALESCE(recall_count, 0), COALESCE(embedding_id, ''), COALESCE(supersedes_json, '') FROM memories_meta ORDER BY id;",
    )?;
    let mut count = 0;
    for line in rows.lines() {
        let parts: Vec<&str> = line.splitn(10, '|').collect();
        if parts.len() < 10 {
            continue;
        }
        let path = parts[3].to_string();
        let body = std::fs::read_to_string(&path)
            .ok()
            .map(|c| split_body(&c).to_string())
            .unwrap_or_default();
        let record = Record {
            id: parts[0].to_string(),
            kind: parts[1].to_string(),
            subject: parts[2].to_string(),
            path,
            confidence: parts[4].parse().unwrap_or(0.5),
            created_at: parts[5].to_string(),
            last_recalled_at: if parts[6].is_empty() { None } else { Some(parts[6].to_string()) },
            recall_count: parts[7].parse().unwrap_or(0),
            embedding_id: if parts[8].is_empty() { None } else { Some(parts[8].to_string()) },
            supersedes_json: if parts[9].is_empty() { None } else { Some(parts[9].to_string()) },
            body,
        };
        let s = serde_json::to_string(&record).map_err(std::io::Error::other)?;
        writeln!(w, "{s}")?;
        count += 1;
    }
    Ok(count)
}

/// Import an NDJSON stream into the recall store at `root`.
pub fn import<R: Read>(
    root: &Path,
    reader: R,
    replace: bool,
) -> std::io::Result<ImportSummary> {
    require_sqlite3()?;
    std::fs::create_dir_all(root.join("memories"))?;
    std::fs::create_dir_all(root.join("index"))?;
    let db = root.join("index").join("recall.sqlite");
    ensure_schema(&db)?;

    let mut summary = ImportSummary { imported: 0, skipped: 0, errors: 0 };
    let buf = std::io::BufRead::lines(std::io::BufReader::new(reader));
    for (line_no, line_result) in buf.enumerate() {
        let Ok(line) = line_result else { continue };
        if line.trim().is_empty() {
            continue;
        }
        let record: Record = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                eprintln_helper(&format!("recall-io: line {} parse error: {}", line_no + 1, e));
                summary.errors += 1;
                continue;
            }
        };
        if record.id.is_empty() || record.kind.is_empty() || record.subject.is_empty() {
            eprintln_helper(&format!("recall-io: line {} missing required field", line_no + 1));
            summary.errors += 1;
            continue;
        }

        let exists = !sqlite_scalar(
            &db,
            &format!("SELECT id FROM memories_meta WHERE id='{}';", esc(&record.id)),
        )?
        .is_empty();
        if exists && !replace {
            eprintln_helper(&format!("recall-io: skip existing id {}", record.id));
            summary.skipped += 1;
            continue;
        }
        if exists && replace {
            let old_path = sqlite_scalar(
                &db,
                &format!("SELECT path FROM memories_meta WHERE id='{}';", esc(&record.id)),
            )?;
            if !old_path.is_empty() {
                let _ = std::fs::remove_file(&old_path);
            }
            sqlite_exec(
                &db,
                &format!("DELETE FROM memories_meta WHERE id='{}';", esc(&record.id)),
            )?;
        }

        let target_path = memory_path_for(root, &record);
        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let md = render_md(&record);
        std::fs::write(&target_path, md)?;

        let path_str = target_path.to_string_lossy().to_string();
        let lr = record.last_recalled_at.as_deref().unwrap_or("");
        let eid = record.embedding_id.as_deref().unwrap_or("");
        let sj = record.supersedes_json.as_deref().unwrap_or("");
        let sql = format!(
            "INSERT INTO memories_meta (id, kind, subject, path, confidence, created_at, last_recalled_at, recall_count, embedding_id, supersedes_json) VALUES ('{}','{}','{}','{}', {}, '{}', {}, {}, {}, {});",
            esc(&record.id),
            esc(&record.kind),
            esc(&record.subject),
            esc(&path_str),
            record.confidence,
            esc(&record.created_at),
            if lr.is_empty() { "NULL".to_string() } else { format!("'{}'", esc(lr)) },
            record.recall_count,
            if eid.is_empty() { "NULL".to_string() } else { format!("'{}'", esc(eid)) },
            if sj.is_empty() { "NULL".to_string() } else { format!("'{}'", esc(sj)) },
        );
        sqlite_exec(&db, &sql)?;
        summary.imported += 1;
    }
    Ok(summary)
}

fn memory_path_for(root: &Path, r: &Record) -> PathBuf {
    let safe_subject = r.subject.replace(':', "/");
    root.join("memories").join(safe_subject).join(format!("{}.md", r.id))
}

fn render_md(r: &Record) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!("id: {}\n", r.id));
    out.push_str(&format!("kind: {}\n", r.kind));
    out.push_str(&format!("subject: {}\n", r.subject));
    out.push_str(&format!("confidence: {}\n", r.confidence));
    out.push_str(&format!("created_at: {}\n", r.created_at));
    if let Some(lr) = &r.last_recalled_at {
        out.push_str(&format!("last_recalled_at: {}\n", lr));
    }
    out.push_str(&format!("recall_count: {}\n", r.recall_count));
    out.push_str("---\n");
    out.push_str(&r.body);
    if !r.body.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn split_body(content: &str) -> &str {
    let after_open = if let Some(rest) = content.strip_prefix("---\n") { rest } else { return content; };
    if let Some(body) = after_open.strip_prefix("---\n") {
        return body;
    }
    if let Some(idx) = after_open.find("\n---\n") {
        return &after_open[idx + "\n---\n".len()..];
    }
    content
}

fn ensure_schema(db: &Path) -> std::io::Result<()> {
    let sql = "CREATE TABLE IF NOT EXISTS memories_meta (id TEXT PRIMARY KEY, kind TEXT NOT NULL, subject TEXT NOT NULL, path TEXT NOT NULL, confidence REAL NOT NULL DEFAULT 0.5, created_at TEXT NOT NULL, last_recalled_at TEXT, recall_count INTEGER NOT NULL DEFAULT 0, decays_after TEXT, supersedes_json TEXT, embedding BLOB, embedding_id TEXT, embedding_dim INTEGER);";
    sqlite_exec(db, sql)
}

fn require_sqlite3() -> std::io::Result<()> {
    let ok = Command::new("sqlite3").arg("--version").output().is_ok_and(|o| o.status.success());
    if ok {
        Ok(())
    } else {
        Err(std::io::Error::other("sqlite3 binary not available on PATH"))
    }
}

fn sqlite_exec(db: &Path, sql: &str) -> std::io::Result<()> {
    let out = Command::new("sqlite3").arg(db).arg(sql).output()?;
    if !out.status.success() {
        return Err(std::io::Error::other(format!(
            "sqlite3 failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(())
}

fn sqlite_exec_capture(db: &Path, sql: &str) -> std::io::Result<String> {
    let out = Command::new("sqlite3").arg(db).arg(sql).output()?;
    if !out.status.success() {
        return Err(std::io::Error::other(format!(
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

#[allow(clippy::print_stderr)]
fn eprintln_helper(msg: &str) {
    eprintln!("{msg}");
}
