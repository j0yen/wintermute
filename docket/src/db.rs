//! Database layer for docket — `SQLite` via rusqlite.
//!
//! All mutations use `BEGIN IMMEDIATE` transactions for safe concurrent
//! access in WAL mode.

use std::path::PathBuf;

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};

use crate::error::{DocketError, Result};
use crate::model::{EvidenceRow, Finding, Severity, Status, parse_evidence_ref};

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
#[allow(dead_code)]
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

/// Apply idempotent schema migrations.
///
/// Migration M0: original findings table (docket-core).
/// Migration M1 (docket-escalate): `escalated_at`, `escalation_reason`
///   columns + `runs` ledger table.  Uses `ALTER TABLE … ADD COLUMN IF NOT
///   EXISTS` semantics via a SELECT-from-pragma guard to stay compatible with
///   `SQLite` < 3.37 (which lacks IF NOT EXISTS on ADD COLUMN).
/// Migration M2 (docket-evidence): `evidence` append-only table for typed
///   evidence refs keyed to findings.
fn migrate(conn: &Connection) -> Result<()> {
    // M0: core findings table.
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

    // M1: escalate columns — add if not present.
    add_column_if_missing(conn, "findings", "escalated_at", "TEXT")?;
    add_column_if_missing(conn, "findings", "escalation_reason", "TEXT")?;

    // M1: runs ledger table — append-only, ordered by arrival.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS runs (
            run_id  TEXT PRIMARY KEY,
            seq     INTEGER NOT NULL,
            seen_at TEXT NOT NULL
        );",
    )?;

    // M2: typed evidence trail — append-only, FK to findings.key.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS evidence (
            id      INTEGER PRIMARY KEY AUTOINCREMENT,
            key     TEXT NOT NULL REFERENCES findings(key),
            run_id  TEXT NOT NULL,
            kind    TEXT NOT NULL,
            ref_val TEXT NOT NULL,
            note    TEXT,
            seen_at TEXT NOT NULL
        );",
    )?;

    Ok(())
}

/// Add a column to `table` if it does not already exist.
/// Uses `PRAGMA table_info` to check before attempting `ALTER TABLE`.
fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    col_type: &str,
) -> Result<()> {
    let mut stmt =
        conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let exists = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .any(|r| r.is_ok_and(|name| name == column));
    if !exists {
        conn.execute_batch(&format!(
            "ALTER TABLE {table} ADD COLUMN {column} {col_type};"
        ))?;
    }
    Ok(())
}


/// Insert one or more typed evidence refs for a finding (M2).
///
/// Each raw ref string in `refs` is parsed by prefix into `(kind, ref_val)`.
/// Parsing is lenient — malformed refs are stored as kind `raw`.
/// Never fails for bad ref syntax; only returns `Err` on DB error.
///
/// # Errors
///
/// Returns an error if the database operation fails.
pub fn insert_evidence(
    conn: &Connection,
    key: &str,
    run_id: &str,
    refs: &[String],
) -> Result<()> {
    if refs.is_empty() {
        return Ok(());
    }
    let now = Utc::now().to_rfc3339();
    for raw in refs {
        let (kind, ref_val) = parse_evidence_ref(raw);
        conn.execute(
            "INSERT INTO evidence (key, run_id, kind, ref_val, note, seen_at)
             VALUES (?1, ?2, ?3, ?4, NULL, ?5)",
            params![key, run_id, kind, ref_val, now],
        )?;
    }
    Ok(())
}

/// List all evidence rows for a given finding key, ordered by insertion id.
///
/// # Errors
///
/// Returns an error if the database query fails.
pub fn list_evidence(conn: &Connection, key: &str) -> Result<Vec<EvidenceRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, key, run_id, kind, ref_val, note, seen_at
         FROM evidence WHERE key = ?1 ORDER BY id ASC",
    )?;
    let rows = stmt
        .query_map(params![key], |row| {
            Ok(EvidenceRow {
                id: row.get(0)?,
                key: row.get(1)?,
                run_id: row.get(2)?,
                kind: row.get(3)?,
                ref_val: row.get(4)?,
                note: row.get(5)?,
                seen_at: row.get(6)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Count evidence rows for a given finding key.
///
/// # Errors
///
/// Returns an error if the database query fails.
pub fn count_evidence(conn: &Connection, key: &str) -> Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM evidence WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .map_err(Into::into)
}


/// Report a finding.  Upserts by `key`.
///
/// - If new: inserts with `status=open`, streak=1.
/// - If existing and `run_id` is new: bumps `runs_seen` + consecutive streak.
/// - If existing and same `run_id`: bumps `report_count` + `last_seen` only.
/// - If resolved and reported again: reopens with streak reset to 1.
/// - After streak update: if `consecutive_runs >= escalate_threshold` and
///   status is `open`, transitions to `escalated`.
///
/// Also records the `run_id` in the `runs` ledger (idempotent).
///
/// # Errors
///
/// Returns an error if the database operation fails.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub fn report(
    conn: &Connection,
    run_id: &str,
    key: &str,
    title: &str,
    severity: &str,
    evidence: Option<&str>,
    escalate_threshold: i64,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let sev = severity.to_owned();

    // Use raw BEGIN IMMEDIATE so concurrent reporters don't corrupt counts.
    conn.execute_batch("BEGIN IMMEDIATE")?;

    let result = (|| -> Result<()> {
        // Record this run in the ledger (idempotent inside the transaction).
        let existing_run: Option<i64> = conn
            .query_row(
                "SELECT seq FROM runs WHERE run_id = ?1",
                params![run_id],
                |row| row.get(0),
            )
            .optional()?;
        if existing_run.is_none() {
            let next_seq: i64 = conn.query_row(
                "SELECT COALESCE(MAX(seq), 0) + 1 FROM runs",
                [],
                |row| row.get(0),
            )?;
            let run_seen_at = now.clone();
            conn.execute(
                "INSERT INTO runs (run_id, seq, seen_at) VALUES (?1, ?2, ?3)",
                params![run_id, next_seq, run_seen_at],
            )?;
        }

        let existing: Option<(String, String, i64, i64, i64)> = conn
            .query_row(
                "SELECT status, last_run, runs_seen, consecutive_runs, report_count
                 FROM findings WHERE key = ?1",
                params![key],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                },
            )
            .optional()?;

        match existing {
            None => {
                // New finding.
                let (new_status, esc_at, esc_reason) =
                    escalation_fields("open", 1, escalate_threshold, &now);
                conn.execute(
                    "INSERT INTO findings
                     (key, title, severity, status, first_seen, last_seen,
                      first_run, last_run, runs_seen, consecutive_runs, report_count,
                      resolved_at, resolve_reason, evidence, escalated_at, escalation_reason)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6, ?6, 1, 1, 1, NULL, NULL, ?7, ?8, ?9)",
                    params![
                        key, title, sev, new_status, now, run_id, evidence,
                        esc_at, esc_reason
                    ],
                )?;
            }
            Some((status, last_run, runs_seen, consecutive_runs, _report_count)) => {
                if status == "resolved" {
                    // Reopen: reset streak to 1, clear resolved fields.
                    let new_runs = if run_id == last_run {
                        runs_seen
                    } else {
                        runs_seen + 1
                    };
                    // Reopened findings start open with streak 1.
                    let (new_status, esc_at, esc_reason) =
                        escalation_fields("open", 1, escalate_threshold, &now);
                    conn.execute(
                        "UPDATE findings
                         SET title = ?1, severity = ?2, status = ?3,
                             last_seen = ?4, last_run = ?5,
                             runs_seen = ?6, consecutive_runs = 1,
                             report_count = report_count + 1,
                             resolved_at = NULL, resolve_reason = NULL,
                             evidence = COALESCE(?7, evidence),
                             escalated_at = ?8, escalation_reason = ?9
                         WHERE key = ?10",
                        params![
                            title, sev, new_status, now, run_id,
                            new_runs, evidence, esc_at, esc_reason, key
                        ],
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
                    let new_streak = consecutive_runs + 1;
                    let (new_status, esc_at, esc_reason) =
                        escalation_fields(&status, new_streak, escalate_threshold, &now);
                    conn.execute(
                        "UPDATE findings
                         SET title = ?1, severity = ?2, last_seen = ?3,
                             last_run = ?4,
                             runs_seen = ?5,
                             consecutive_runs = ?6,
                             status = ?7,
                             report_count = report_count + 1,
                             evidence = COALESCE(?8, evidence),
                             escalated_at = COALESCE(escalated_at, ?9),
                             escalation_reason = COALESCE(escalation_reason, ?10)
                         WHERE key = ?11",
                        params![
                            title, sev, now, run_id,
                            runs_seen + 1, new_streak, new_status,
                            evidence, esc_at, esc_reason, key
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
        let _ = conn.execute_batch("ROLLBACK");
    }
    result
}

/// Compute the escalation state after updating the streak.
///
/// Returns `(status_str, escalated_at, escalation_reason)`.
/// - If `current_status` is already `"escalated"`, it stays escalated.
/// - If `current_status` is `"open"` and `new_streak >= threshold`, escalates.
/// - Otherwise stays as-is with `None` fields.
fn escalation_fields(
    current_status: &str,
    new_streak: i64,
    threshold: i64,
    now: &str,
) -> (String, Option<String>, Option<String>) {
    if current_status == "escalated" {
        // Sticky: stays escalated, don't overwrite existing timestamps.
        return ("escalated".to_owned(), None, None);
    }
    if current_status == "open" && new_streak >= threshold {
        let reason = format!(
            "recurred {new_streak} consecutive runs (\u{2265}{threshold}); \
             durable handling justified per self-review SKILL.md \u{00a7}359"
        );
        return ("escalated".to_owned(), Some(now.to_owned()), Some(reason));
    }
    (current_status.to_owned(), None, None)
}

/// Sweep: auto-resolve open/escalated findings absent from the current run.
///
/// For each open or escalated finding whose `last_run != current_run_id`:
/// - Count how many `runs` ledger entries have a seq > the finding's
///   `last_run` seq AND seq < the current run's seq (i.e. runs elapsed since
///   it was last seen).
/// - If that count >= `stale_after`, mark the finding
///   `resolved(stale: not seen in <count> runs (swept at <current_run_id>))`.
/// - Otherwise, if the finding was seen in a prior run but not the current
///   one, reset `consecutive_runs` to 0 (gap breaks the streak), without
///   resolving.
///
/// The current run-id is recorded in the ledger as a side effect.
///
/// # Errors
///
/// Returns an error if the database operation fails.
#[allow(clippy::too_many_lines)]
pub fn sweep(
    conn: &Connection,
    current_run_id: &str,
    stale_after: i64,
) -> Result<SweepResult> {
    let now = Utc::now().to_rfc3339();

    conn.execute_batch("BEGIN IMMEDIATE")?;

    let result = (|| -> Result<SweepResult> {
        // Record the sweep run in the ledger.
        let existing_seq: Option<i64> = conn
            .query_row(
                "SELECT seq FROM runs WHERE run_id = ?1",
                params![current_run_id],
                |row| row.get(0),
            )
            .optional()?;
        let current_seq = if let Some(s) = existing_seq {
            s
        } else {
            let next_seq: i64 = conn.query_row(
                "SELECT COALESCE(MAX(seq), 0) + 1 FROM runs",
                [],
                |row| row.get(0),
            )?;
            conn.execute(
                "INSERT INTO runs (run_id, seq, seen_at) VALUES (?1, ?2, ?3)",
                params![current_run_id, next_seq, now],
            )?;
            next_seq
        };

        // Gather open/escalated findings not seen in the current run.
        // Note: the intermediate `rows` binding is required to satisfy the
        // borrow checker — the stmt borrow must end before the block ends.
        #[allow(clippy::let_and_return)]
        let candidates: Vec<(String, String, i64)> = {
            let mut stmt = conn.prepare(
                "SELECT key, last_run, consecutive_runs FROM findings
                 WHERE status IN ('open', 'escalated') AND last_run != ?1",
            )?;
            let rows = stmt.query_map(params![current_run_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
            rows
        };

        let mut resolved_count = 0i64;
        let mut streak_reset_count = 0i64;

        for (key, last_run, _cons_runs) in candidates {
            // Find the seq for this finding's last_run.
            let last_seq: Option<i64> = conn
                .query_row(
                    "SELECT seq FROM runs WHERE run_id = ?1",
                    params![last_run],
                    |row| row.get(0),
                )
                .optional()?;

            // Count runs that elapsed since last_run (seq > last_seq, seq < current_seq).
            let elapsed = if let Some(ls) = last_seq {
                conn.query_row(
                    "SELECT COUNT(*) FROM runs WHERE seq > ?1 AND seq < ?2",
                    params![ls, current_seq],
                    |row| row.get::<_, i64>(0),
                )?
            } else {
                // last_run not in ledger — treat as maximum elapsed.
                current_seq
            };

            if elapsed >= stale_after {
                let reason = format!(
                    "stale: not seen in {elapsed} runs (swept at {current_run_id})"
                );
                conn.execute(
                    "UPDATE findings
                     SET status = 'resolved', resolved_at = ?1, resolve_reason = ?2,
                         consecutive_runs = 0
                     WHERE key = ?3",
                    params![now, reason, key],
                )?;
                resolved_count += 1;
            } else {
                // Gap breaks the streak but finding is not stale yet.
                conn.execute(
                    "UPDATE findings SET consecutive_runs = 0 WHERE key = ?1",
                    params![key],
                )?;
                streak_reset_count += 1;
            }
        }

        Ok(SweepResult {
            resolved: resolved_count,
            streak_reset: streak_reset_count,
        })
    })();

    match result {
        Ok(r) => {
            conn.execute_batch("COMMIT")?;
            Ok(r)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

/// Result of a `sweep` operation.
#[derive(Debug)]
pub struct SweepResult {
    /// Number of findings resolved as stale.
    pub resolved: i64,
    /// Number of findings that had their streak reset (not stale yet).
    pub streak_reset: i64,
}

/// List findings matching the given filters.
///
/// `status_filter`: `"open"`, `"resolved"`, `"escalated"`, or `"all"`.
/// `min_severity`: optional minimum severity rank (info=0, warn=1, crit=2).
///
/// Populates `evidence_count` for each finding (cheap aggregate); the full
/// `evidence_trail` is NOT populated here — use `show` for that.
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
        "escalated" => {
            "SELECT * FROM findings WHERE status = 'escalated' ORDER BY last_seen DESC"
        }
        _ => "SELECT * FROM findings ORDER BY last_seen DESC",
    };
    let mut stmt = conn.prepare(sql)?;
    let findings = stmt
        .query_map([], row_to_finding)?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut result: Vec<Finding> = findings
        .into_iter()
        .filter(|f| {
            min_severity.is_none_or(|min| {
                Severity::from_str(&f.severity.to_string())
                    .is_some_and(|s| s.rank() >= min)
            })
        })
        .collect();

    // Populate evidence_count for each finding via cheap aggregate.
    for f in &mut result {
        f.evidence_count = count_evidence(conn, &f.key)?;
    }

    Ok(result)
}

/// Retrieve a single finding by key, including its full typed evidence trail.
///
/// # Errors
///
/// Returns `DocketError::NotFound` if the key does not exist.
pub fn show(conn: &Connection, key: &str) -> Result<Finding> {
    let mut finding = conn
        .query_row(
            "SELECT * FROM findings WHERE key = ?1",
            params![key],
            row_to_finding,
        )
        .optional()?
        .ok_or_else(|| DocketError::NotFound(key.to_owned()))?;
    // Populate the typed evidence trail (M2).
    finding.evidence_trail = list_evidence(conn, key)?;
    Ok(finding)
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

/// List findings in `open` or `escalated` state, applying an optional minimum
/// severity filter.  Used by `docket digest`.
///
/// # Errors
///
/// Returns an error if the database query fails.
pub fn list_active(conn: &Connection, min_severity: Option<u8>) -> Result<Vec<Finding>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM findings WHERE status IN ('open', 'escalated') ORDER BY runs_seen DESC",
    )?;
    let findings = stmt
        .query_map([], row_to_finding)?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(findings
        .into_iter()
        .filter(|f| {
            min_severity.is_none_or(|min| {
                Severity::from_str(&f.severity.to_string())
                    .is_some_and(|s| s.rank() >= min)
            })
        })
        .collect())
}

fn row_to_finding(row: &rusqlite::Row<'_>) -> rusqlite::Result<Finding> {
    let severity_str: String = row.get(2)?;
    let status_str: String = row.get(3)?;

    let severity = match severity_str.as_str() {
        "info" => Severity::Info,
        "crit" => Severity::Crit,
        _ => Severity::Warn,
    };
    let status = match status_str.as_str() {
        "resolved" => Status::Resolved,
        "escalated" => Status::Escalated,
        _ => Status::Open,
    };

    // Columns 0..13 are the M0 schema.
    // Columns 14..15 are the M1 escalate columns (may be absent in old DBs —
    // rusqlite returns an error for out-of-range column indices, so we use
    // `get_ref` which lets us return None when the value is NULL or absent).
    let escalated_at: Option<String> = row.get(14).unwrap_or(None);
    let escalation_reason: Option<String> = row.get(15).unwrap_or(None);

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
        escalated_at,
        escalation_reason,
        // evidence_trail and evidence_count are populated by show/list after row mapping.
        evidence_trail: Vec::new(),
        evidence_count: 0,
    })
}
