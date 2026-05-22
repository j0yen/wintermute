//! `SQLite` + FTS5 keyword index over memories.
//!
//! The Markdown files are the source of truth. This index is rebuildable
//! at any time from `FileStore::iter_all` via `Index::reindex`.

use crate::memory::Memory;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};

pub struct Index {
    conn: Connection,
}

#[derive(Debug, Clone)]
pub struct Hit {
    pub id: String,
    pub kind: String,
    pub subject: String,
    pub path: PathBuf,
    pub snippet: String,
    pub bm25: f64,
    pub confidence: f64,
    pub recall_count: u32,
    pub last_recalled_at: Option<DateTime<Utc>>,
}

impl Index {
    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)
            .with_context(|| format!("open sqlite at {}", db_path.display()))?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn })
    }

    /// Insert or replace the index entry for a memory.
    pub fn upsert(&self, mem: &Memory, path: &Path) -> Result<()> {
        let path_str = path.to_string_lossy().to_string();
        let supersedes_json = if mem.front.supersedes.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&mem.front.supersedes)?)
        };

        // FTS5: delete-then-insert to update.
        self.conn
            .execute("DELETE FROM memories_fts WHERE id = ?1", params![mem.front.id])?;
        self.conn.execute(
            "INSERT INTO memories_fts (id, body, subject, kind) VALUES (?1, ?2, ?3, ?4)",
            params![
                mem.front.id,
                mem.body,
                mem.front.subject.as_str(),
                mem.front.kind.as_str()
            ],
        )?;

        self.conn.execute(
            "INSERT INTO memories_meta (id, kind, subject, path, confidence, created_at, last_recalled_at, recall_count, decays_after, supersedes_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(id) DO UPDATE SET
               kind = excluded.kind,
               subject = excluded.subject,
               path = excluded.path,
               confidence = excluded.confidence,
               decays_after = excluded.decays_after,
               supersedes_json = excluded.supersedes_json",
            params![
                mem.front.id,
                mem.front.kind.as_str(),
                mem.front.subject.as_str(),
                path_str,
                mem.front.confidence,
                mem.front.created_at.to_rfc3339(),
                mem.front.last_recalled_at.map(|t| t.to_rfc3339()),
                mem.front.recall_count,
                mem.front.decays_after,
                supersedes_json,
            ],
        )?;
        Ok(())
    }

    pub fn remove(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM memories_fts WHERE id = ?1", params![id])?;
        self.conn
            .execute("DELETE FROM memories_meta WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// FTS5 query. Returns hits ordered by BM25 (lower is better in `SQLite`'s `bm25()`).
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<Hit>> {
        let sanitized = sanitize_fts_query(query);
        let mut stmt = self.conn.prepare(
            "SELECT
                m.id, m.kind, m.subject, m.path,
                snippet(memories_fts, 1, '[', ']', '…', 12) AS snip,
                bm25(memories_fts) AS rank,
                m.confidence, m.recall_count, m.last_recalled_at
             FROM memories_fts
             JOIN memories_meta m ON m.id = memories_fts.id
             WHERE memories_fts MATCH ?1
             ORDER BY rank ASC
             LIMIT ?2",
        )?;
        let lim = i64::try_from(limit).unwrap_or(i64::MAX);
        let rows = stmt.query_map(params![sanitized, lim], |row| {
            let last_str: Option<String> = row.get(8)?;
            let last = last_str.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc)));
            Ok(Hit {
                id: row.get(0)?,
                kind: row.get(1)?,
                subject: row.get(2)?,
                path: PathBuf::from(row.get::<_, String>(3)?),
                snippet: row.get(4)?,
                bm25: row.get(5)?,
                confidence: row.get(6)?,
                recall_count: u32::try_from(row.get::<_, i64>(7)?).unwrap_or(0),
                last_recalled_at: last,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn list(&self, subject_prefix: Option<&str>, limit: usize) -> Result<Vec<Hit>> {
        let lim = i64::try_from(limit).unwrap_or(i64::MAX);
        let (sql, args): (&str, Vec<rusqlite::types::Value>) = match subject_prefix {
            Some(p) => (
                "SELECT id, kind, subject, path, '', 0.0, confidence, recall_count, last_recalled_at
                 FROM memories_meta
                 WHERE subject LIKE ?1
                 ORDER BY created_at DESC
                 LIMIT ?2",
                vec![format!("{p}%").into(), lim.into()],
            ),
            None => (
                "SELECT id, kind, subject, path, '', 0.0, confidence, recall_count, last_recalled_at
                 FROM memories_meta
                 ORDER BY created_at DESC
                 LIMIT ?1",
                vec![lim.into()],
            ),
        };
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(args), |row| {
            let last_str: Option<String> = row.get(8)?;
            let last = last_str.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc)));
            Ok(Hit {
                id: row.get(0)?,
                kind: row.get(1)?,
                subject: row.get(2)?,
                path: PathBuf::from(row.get::<_, String>(3)?),
                snippet: row.get(4)?,
                bm25: row.get(5)?,
                confidence: row.get(6)?,
                recall_count: u32::try_from(row.get::<_, i64>(7)?).unwrap_or(0),
                last_recalled_at: last,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Wipe and rebuild from a fresh iterator. Used by `recall reindex`.
    pub fn rebuild_from<I>(&self, iter: I) -> Result<usize>
    where
        I: Iterator<Item = (Memory, PathBuf)>,
    {
        self.conn.execute_batch(
            "DELETE FROM memories_fts; DELETE FROM memories_meta;",
        )?;
        let mut n = 0;
        for (mem, path) in iter {
            self.upsert(&mem, &path)?;
            n += 1;
        }
        Ok(n)
    }

    /// Bump `last_recalled_at = now` and increment `recall_count`. Called after a successful query.
    pub fn touch_recall(&self, id: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE memories_meta
             SET recall_count = recall_count + 1,
                 last_recalled_at = ?1
             WHERE id = ?2",
            params![now, id],
        )?;
        Ok(())
    }

    pub fn count(&self) -> Result<usize> {
        let n: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM memories_meta", [], |row| row.get(0))?;
        Ok(usize::try_from(n).unwrap_or(0))
    }
}

const SCHEMA: &str = r"
CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
    id UNINDEXED,
    body,
    subject,
    kind UNINDEXED
);

CREATE TABLE IF NOT EXISTS memories_meta (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    subject TEXT NOT NULL,
    path TEXT NOT NULL,
    confidence REAL NOT NULL DEFAULT 0.5,
    created_at TEXT NOT NULL,
    last_recalled_at TEXT,
    recall_count INTEGER NOT NULL DEFAULT 0,
    decays_after TEXT,
    supersedes_json TEXT
);

CREATE INDEX IF NOT EXISTS idx_meta_subject ON memories_meta(subject);
CREATE INDEX IF NOT EXISTS idx_meta_kind    ON memories_meta(kind);
CREATE INDEX IF NOT EXISTS idx_meta_created ON memories_meta(created_at);
";

/// FTS5 has a small query mini-language; strip characters that turn user input
/// into a syntax error. We're lenient: words become an OR of prefix matches.
fn sanitize_fts_query(q: &str) -> String {
    let cleaned: String = q
        .chars()
        .map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' })
        .collect();
    let terms: Vec<String> = cleaned
        .split_whitespace()
        .filter(|t| !t.is_empty())
        .map(|t| format!("{t}*"))
        .collect();
    if terms.is_empty() {
        // FTS will error on an empty query; match nothing instead.
        return "__no_match__".into();
    }
    terms.join(" OR ")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::memory::{Kind, Subject};

    #[test]
    fn upsert_and_search_finds_match() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("idx.sqlite");
        let idx = Index::open(&db).unwrap();
        let mem = Memory::new(
            Kind::Semantic,
            Subject::user(),
            "user prefers integration tests over mocks for the auth code",
        );
        let path = tmp.path().join("mem.md");
        std::fs::write(&path, mem.to_markdown().unwrap()).unwrap();
        idx.upsert(&mem, &path).unwrap();
        let hits = idx.search("integration auth", 5).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, mem.front.id);
    }

    #[test]
    fn list_filters_by_subject_prefix() {
        let tmp = tempfile::tempdir().unwrap();
        let idx = Index::open(&tmp.path().join("idx.sqlite")).unwrap();
        let m_user = Memory::new(Kind::Semantic, Subject::user(), "u");
        let m_proj = Memory::new(Kind::Procedural, Subject::project("recall"), "p");
        idx.upsert(&m_user, &tmp.path().join("a.md")).unwrap();
        idx.upsert(&m_proj, &tmp.path().join("b.md")).unwrap();
        let proj_hits = idx.list(Some("project:"), 10).unwrap();
        assert_eq!(proj_hits.len(), 1);
        assert_eq!(proj_hits[0].subject, "project:recall");
    }

    #[test]
    fn touch_recall_increments_count() {
        let tmp = tempfile::tempdir().unwrap();
        let idx = Index::open(&tmp.path().join("idx.sqlite")).unwrap();
        let mem = Memory::new(Kind::Semantic, Subject::user(), "x");
        idx.upsert(&mem, &tmp.path().join("a.md")).unwrap();
        idx.touch_recall(&mem.front.id).unwrap();
        idx.touch_recall(&mem.front.id).unwrap();
        let hits = idx.list(None, 10).unwrap();
        assert_eq!(hits[0].recall_count, 2);
        assert!(hits[0].last_recalled_at.is_some());
    }

    #[test]
    fn sanitize_strips_punctuation() {
        assert_eq!(sanitize_fts_query("hello, world!"), "hello* OR world*");
        assert_eq!(sanitize_fts_query(""), "__no_match__");
        assert_eq!(sanitize_fts_query("foo's bar"), "foo* OR s* OR bar*");
    }
}
