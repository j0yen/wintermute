//! Database layer for docket — SQLite via rusqlite.
//!
//! All mutations use `BEGIN IMMEDIATE` transactions for safe concurrent
//! access in WAL mode.

use std::path::PathBuf;

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};

use crate::error::{DocketError, Result};
use crate::model::{Finding, Severity, Status};

/// Open (or create) the docket database, applying migrations.
///
/// # Errors
///
/// Returns an error if the parent directory cannot be created or the
/// database cannot be opened or migrated.
pub fn open() -> Result<Connection> {
    let path = db_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(&path)?;
    // Enable WAL for concurrent readers + writers.
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    migrate(&conn)?;
    Ok(conn)
}

/// Open a database at a specific path (used in tests).
///
/// # Errors
///
/// Returns an error if the database cannot be opened or migrated.
pub fn open_at(path: &std::path::Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    migrate(&conn)?;
    Ok(conn)
}

fn db_path() -> Result<PathBuf> {
    let base = if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(xdg)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".local").join("share")
    } else {
        return Err(DocketError::DbPath(
            "neither XDG_DATA_HOME nor HOME is set".to_owned(),
        ));
    };
    Ok(base.join("docket").join("docket.db"))
}

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS findings (
            key              TEXT PRIMARY KEY,
            title            TEXT NOT NULL,
            severity         TEXT NOT NULL DEFAULT 'warn',
            status           TEXT NOT NULL DEFAULT 'open',
            first_seen       TEXT NOT NULL,
            last_seen        TEXT NOT NULL,
            first_run        TEXT NOT NULL,
            last_run         TEXT NOT NULL,
            runs_seen        INTEGER NOT NULL DEFAULT 1,
            consecutive_runs INTEGER NOT NULL DEFAULT 1,
            report_count     INTEGER NOT NULL DEFAULT 1,
            resolved_at      TEXT,
            resolve_reason   TEXT,
            evidence         TEXT
        );",
    )?;
    Ok(())
}

/// Report a finding.  Upserts by `key`.
///
/// - If new: inserts with status=open, streak=1.
/// - If existing and run_id is new: bumps runs_seen + consecutive_runs + last_seen/run.
/// - If existing and same run_id: bumps report_count + last_seen only.
/// - If resolved and reported again: reopens with streak reset to 1.
///
/// # Errors
///
/// Returns an error if the database operation fails.
pub fn report(
    conn: &Connection,
    run_id: &str,
    key: &str,
    title: &str,
    severity: &str,
    evidence: Option<&str>,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let sev = severity.to_owned();

    // Use raw BEGIN IMMEDIATE so concurrent reporters don't corrupt counts.
    // We cannot use `conn.unchecked_transaction()` here because rusqlite
    // issues `BEGIN DEFERRED` under the hood and SQLite rejects a nested
    // `BEGIN IMMEDIATE` on the same connection.
    conn.execute_batch("BEGIN IMMEDIATE")?;

    let result = (|| -> Result<()> {
        let existing: Option<(String, String, i64, i64, i64, Option<String>)> = conn
            .query_row(
                "SELECT status, last_run, runs_seen, consecutive_runs, report_count, resolved_at
                 FROM findings WHERE key = ?1",
                params![key],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, Option<String>>(5)?,
                    ))
                },
            )
            .optional()?;

        match existing {
            None => {
                // New finding.
                conn.execute(
                    "INSERT INTO findings
                     (key, title, severity, status, first_seen, last_seen,
                      first_run, last_run, runs_seen, consecutive_runs, report_count,
                      resolved_at, resolve_reason, evidence)
                     VALUES (?1, ?2, ?3, 'open', ?4, ?4, ?5, ?5, 1, 1, 1, NULL, NULL, ?6)",
                    params![key, title, sev, now, run_id, evidence],
                )?;
            }
            Some((status, last_run, runs_seen, consecutive_runs, _report_count, _resolved_at)) => {
                if status == "resolved" {
                    // Reopen: reset streak to 1, clear resolved fields.
                    let new_runs = if run_id != last_run {
                        runs_seen + 1
                    } else {
                        runs_seen
                    };
                    conn.execute(
                        "UPDATE findings
                         SET title = ?1, severity = ?2, status = 'open',
                             last_seen = ?3, last_run = ?4,
                             runs_seen = ?5, consecutive_runs = 1,
                             report_count = report_count + 1,
                             resolved_at = NULL, resolve_reason = NULL,
                             evidence = COALESCE(?6, evidence)
                         WHERE key = ?7",
                        params![title, sev, now, run_id, new_runs, evidence, key],
                    )?;
                } else if run_id == last_run {
                    // Same run — idempotent for streak, bump report_count + last_seen.
                    conn.execute(
                        "UPDATE findings
                         SET title = ?1, severity = ?2, last_seen = ?3,
                             report_count = report_count + 1,
                             evidence = COALESCE(?4, evidence)
                         WHERE key = ?5",
                        params![title, sev, now, evidence, key],
                    )?;
                } else {
                    // New run — advance streak + runs_seen.
                    conn.execute(
                        "UPDATE findings
                         SET title = ?1, severity = ?2, last_seen = ?3,
                             last_run = ?4,
                             runs_seen = ?5,
                             consecutive_runs = ?6,
                             report_count = report_count + 1,
                             evidence = COALESCE(?7, evidence)
                         WHERE key = ?8",
                        params![
                            title,
                            sev,
                            now,
                            run_id,
                            runs_seen + 1,
                            consecutive_runs + 1,
                            evidence,
                            key
                        ],
                    )?;
                }
            }
        }
        Ok(())
    })();

    if result.is_ok() {
        conn.execute_batch("COMMIT")?;
    } else {
        // Best-effort rollback; ignore any rollback error.
        let _ = conn.execute_batch("ROLLBACK");
    }
    result
}

/// List findings matching the given filters.
///
/// `status_filter`: `"open"`, `"resolved"`, or `"all"`.
/// `min_severity`: optional minimum severity rank (info=0, warn=1, crit=2).
///
/// # Errors
///
/// Returns an error if the database query fails.
pub fn list(
    conn: &Connection,
    status_filter: &str,
    min_severity: Option<u8>,
) -> Result<Vec<Finding>> {
    let sql = match status_filter {
        "open" => "SELECT * FROM findings WHERE status = 'open' ORDER BY last_seen DESC",
        "resolved" => "SELECT * FROM findings WHERE status = 'resolved' ORDER BY last_seen DESC",
        _ => "SELECT * FROM findings ORDER BY last_seen DESC",
    };
    let mut stmt = conn.prepare(sql)?;
    let findings = stmt
        .query_map([], row_to_finding)?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(findings
        .into_iter()
        .filter(|f| {
            min_severity.map_or(true, |min| {
                Severity::from_str(&f.severity.to_string())
                    .map_or(false, |s| s.rank() >= min)
            })
        })
        .collect())
}

/// Retrieve a single finding by key.
///
/// # Errors
///
/// Returns `DocketError::NotFound` if the key does not exist.
pub fn show(conn: &Connection, key: &str) -> Result<Finding> {
    conn.query_row(
        "SELECT * FROM findings WHERE key = ?1",
        params![key],
        row_to_finding,
    )
    .optional()?
    .ok_or_else(|| DocketError::NotFound(key.to_owned()))
}

/// Resolve a finding.
///
/// # Errors
///
/// Returns `DocketError::NotFound` if the key does not exist.
pub fn resolve(conn: &Connection, key: &str, reason: Option<&str>) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    // First verify the key exists.
    let _existing = show(conn, key)?;
    conn.execute(
        "UPDATE findings SET status = 'resolved', resolved_at = ?1, resolve_reason = ?2
         WHERE key = ?3",
        params![now, reason, key],
    )?;
    Ok(())
}

fn row_to_finding(row: &rusqlite::Row<'_>) -> rusqlite::Result<Finding> {
    let severity_str: String = row.get(2)?;
    let status_str: String = row.get(3)?;

    let severity = match severity_str.as_str() {
        "info" => Severity::Info,
        "crit" => Severity::Crit,
        _ => Severity::Warn,
    };
    let status = if status_str == "resolved" {
        Status::Resolved
    } else {
        Status::Open
    };

    Ok(Finding {
        key: row.get(0)?,
        title: row.get(1)?,
        severity,
        status,
        first_seen: row.get(4)?,
        last_seen: row.get(5)?,
        first_run: row.get(6)?,
        last_run: row.get(7)?,
        runs_seen: row.get(8)?,
        consecutive_runs: row.get(9)?,
        report_count: row.get(10)?,
        resolved_at: row.get(11)?,
        resolve_reason: row.get(12)?,
        evidence: row.get(13)?,
    })
}
