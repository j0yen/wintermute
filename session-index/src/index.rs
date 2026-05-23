//! `SQLite` + FTS5 index over session-jsonl records.

use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use std::path::Path;

use crate::jsonl::IndexableRecord;

pub const SCHEMA_VERSION: &str = "1";

pub struct Index {
    conn: Connection,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Hit {
    pub session_id: String,
    pub project_dir: String,
    pub turn_index: i64,
    pub ts: Option<String>,
    pub role: String,
    pub tool_name: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub project_dir: String,
    pub row_count: i64,
    pub first_ts: Option<String>,
    pub last_ts: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Stats {
    pub session_count: i64,
    pub row_count: i64,
    pub project_count: i64,
    pub schema_version: String,
}

impl Index {
    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create index dir {}", parent.display()))?;
        }
        let conn = Connection::open(db_path)
            .with_context(|| format!("open sqlite at {}", db_path.display()))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.execute_batch(SCHEMA)?;
        conn.execute(
            "INSERT OR IGNORE INTO transcript_state(key, value) VALUES ('schema_version', ?1)",
            params![SCHEMA_VERSION],
        )?;
        Ok(Self { conn })
    }

    /// Drop all rows. Schema and state survive.
    pub fn truncate(&mut self) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM transcript_fts", [])?;
        tx.execute("DELETE FROM transcript_meta", [])?;
        tx.commit()?;
        Ok(())
    }

    /// Insert all records from one session file. Caller is responsible for
    /// truncating first when doing a full reindex.
    pub fn insert_session(
        &mut self,
        session_id: &str,
        project_dir: &str,
        records: &[IndexableRecord],
    ) -> Result<()> {
        let tx = self.conn.transaction()?;
        {
            let mut ins_fts = tx.prepare(
                "INSERT INTO transcript_fts(rowid, session_id, role, text) VALUES (?1, ?2, ?3, ?4)",
            )?;
            let mut ins_meta = tx.prepare(
                "INSERT INTO transcript_meta(rowid, session_id, turn_index, project_dir, ts, role, tool_name)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for (i, rec) in records.iter().enumerate() {
                // rowid is auto-managed; we let SQLite assign and use
                // last_insert_rowid in the meta insert via a single rowid.
                // Simpler: NULL for fts rowid then read back, but doing two
                // parallel auto-keys is finicky. Use a CTE-style approach:
                ins_fts.execute(params![
                    rusqlite::types::Null,
                    session_id,
                    rec.role,
                    rec.text
                ])?;
                let rowid = tx.last_insert_rowid();
                let turn = i64::try_from(i).unwrap_or(i64::MAX);
                ins_meta.execute(params![
                    rowid,
                    session_id,
                    turn,
                    project_dir,
                    rec.ts,
                    rec.role,
                    rec.tool_name,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// FTS5 keyword search. `limit` rows; newest first by `ts`.
    pub fn query(&self, q: &str, limit: usize) -> Result<Vec<Hit>> {
        let fts = sanitize_fts_query(q);
        let mut stmt = self.conn.prepare(
            "SELECT m.session_id, m.project_dir, m.turn_index, m.ts, m.role, m.tool_name,
                    snippet(transcript_fts, 2, '[', ']', '…', 24) AS excerpt
             FROM transcript_fts f
             JOIN transcript_meta m ON m.rowid = f.rowid
             WHERE transcript_fts MATCH ?1
             ORDER BY COALESCE(m.ts, '') DESC
             LIMIT ?2",
        )?;
        let limit_i = i64::try_from(limit).unwrap_or(i64::MAX);
        let rows = stmt
            .query_map(params![fts, limit_i], |row| {
                Ok(Hit {
                    session_id: row.get(0)?,
                    project_dir: row.get(1)?,
                    turn_index: row.get(2)?,
                    ts: row.get(3)?,
                    role: row.get(4)?,
                    tool_name: row.get(5)?,
                    text: row.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn list_sessions(&self, limit: usize) -> Result<Vec<SessionSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT session_id,
                    MAX(project_dir) AS project_dir,
                    COUNT(*) AS row_count,
                    MIN(ts) AS first_ts,
                    MAX(ts) AS last_ts
             FROM transcript_meta
             GROUP BY session_id
             ORDER BY COALESCE(MAX(ts), '') DESC
             LIMIT ?1",
        )?;
        let limit_i = i64::try_from(limit).unwrap_or(i64::MAX);
        let rows = stmt
            .query_map(params![limit_i], |row| {
                Ok(SessionSummary {
                    session_id: row.get(0)?,
                    project_dir: row.get(1)?,
                    row_count: row.get(2)?,
                    first_ts: row.get(3)?,
                    last_ts: row.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn stats(&self) -> Result<Stats> {
        let row_count: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM transcript_meta", [], |r| r.get(0))?;
        let session_count: i64 = self.conn.query_row(
            "SELECT COUNT(DISTINCT session_id) FROM transcript_meta",
            [],
            |r| r.get(0),
        )?;
        let project_count: i64 = self.conn.query_row(
            "SELECT COUNT(DISTINCT project_dir) FROM transcript_meta",
            [],
            |r| r.get(0),
        )?;
        let schema_version: String = self
            .conn
            .query_row(
                "SELECT value FROM transcript_state WHERE key = 'schema_version'",
                [],
                |r| r.get(0),
            )
            .unwrap_or_else(|_| SCHEMA_VERSION.to_owned());
        Ok(Stats {
            session_count,
            row_count,
            project_count,
            schema_version,
        })
    }

    pub fn set_state(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO transcript_state(key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
}

const SCHEMA: &str = r"
CREATE VIRTUAL TABLE IF NOT EXISTS transcript_fts USING fts5(
    session_id UNINDEXED,
    role UNINDEXED,
    text
);

CREATE TABLE IF NOT EXISTS transcript_meta (
    rowid INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL,
    turn_index INTEGER NOT NULL,
    project_dir TEXT NOT NULL,
    ts TEXT,
    role TEXT NOT NULL,
    tool_name TEXT
);

CREATE INDEX IF NOT EXISTS idx_meta_session ON transcript_meta(session_id);
CREATE INDEX IF NOT EXISTS idx_meta_project ON transcript_meta(project_dir);
CREATE INDEX IF NOT EXISTS idx_meta_ts      ON transcript_meta(ts);

CREATE TABLE IF NOT EXISTS transcript_state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
";

/// FTS5 has a small query mini-language; strip characters that turn user input
/// into a syntax error. Words become an OR of prefix matches.
fn sanitize_fts_query(q: &str) -> String {
    let cleaned: String = q
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect();
    let terms: Vec<String> = cleaned
        .split_whitespace()
        .filter(|t| !t.is_empty())
        .map(|t| format!("{t}*"))
        .collect();
    if terms.is_empty() {
        return "__no_match__".into();
    }
    terms.join(" OR ")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn rec(role: &str, text: &str) -> IndexableRecord {
        IndexableRecord {
            ts: Some("2026-05-23T00:00:00Z".into()),
            role: role.into(),
            tool_name: None,
            text: text.into(),
        }
    }

    #[test]
    fn roundtrip_insert_query() {
        let dir = tempdir().unwrap();
        let mut idx = Index::open(&dir.path().join("t.sqlite")).unwrap();
        idx.insert_session(
            "sess-A",
            "-home-jsy",
            &[
                rec("user", "please git status the repo"),
                rec("assistant", "ok i ran git status"),
            ],
        )
        .unwrap();
        let hits = idx.query("status", 10).unwrap();
        assert_eq!(hits.len(), 2);
        let s = idx.stats().unwrap();
        assert_eq!(s.row_count, 2);
        assert_eq!(s.session_count, 1);
        assert_eq!(s.project_count, 1);
    }

    #[test]
    fn truncate_then_reinsert() {
        let dir = tempdir().unwrap();
        let mut idx = Index::open(&dir.path().join("t.sqlite")).unwrap();
        idx.insert_session("s", "p", &[rec("user", "alpha")])
            .unwrap();
        idx.truncate().unwrap();
        idx.insert_session("s", "p", &[rec("user", "beta")])
            .unwrap();
        let hits = idx.query("alpha", 10).unwrap();
        assert!(hits.is_empty());
        let hits = idx.query("beta", 10).unwrap();
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn empty_query_matches_nothing() {
        let dir = tempdir().unwrap();
        let mut idx = Index::open(&dir.path().join("t.sqlite")).unwrap();
        idx.insert_session("s", "p", &[rec("user", "hello")])
            .unwrap();
        assert!(idx.query("   ", 10).unwrap().is_empty());
    }

    #[test]
    fn punctuated_query_does_not_panic() {
        let dir = tempdir().unwrap();
        let mut idx = Index::open(&dir.path().join("t.sqlite")).unwrap();
        idx.insert_session("s", "p", &[rec("user", "rust 1.85.0")])
            .unwrap();
        let hits = idx.query("1.85", 10).unwrap();
        assert!(!hits.is_empty());
    }
}
