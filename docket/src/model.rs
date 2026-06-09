//! Data model for docket findings.

use serde::{Deserialize, Serialize};

/// The severity of a finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Informational.
    Info,
    /// Warning — default.
    Warn,
    /// Critical.
    Crit,
}

impl Severity {
    /// Parse from a string slice, returning `None` if unrecognised.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "info" => Some(Self::Info),
            "warn" => Some(Self::Warn),
            "crit" => Some(Self::Crit),
            _ => None,
        }
    }

    /// Numeric rank for min-severity filtering (higher = more severe).
    pub const fn rank(&self) -> u8 {
        match self {
            Self::Info => 0,
            Self::Warn => 1,
            Self::Crit => 2,
        }
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Info => write!(f, "info"),
            Self::Warn => write!(f, "warn"),
            Self::Crit => write!(f, "crit"),
        }
    }
}

/// The status of a finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// The finding is currently open.
    Open,
    /// The finding has been escalated — it recurred ≥ threshold consecutive runs.
    Escalated,
    /// The finding has been resolved.
    Resolved,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open => write!(f, "open"),
            Self::Escalated => write!(f, "escalated"),
            Self::Resolved => write!(f, "resolved"),
        }
    }
}

/// A typed, parsed evidence reference.
///
/// Known kinds: `recall`, `journal`, `pid`, `provfs`, `commit`, `path`.
/// Unknown prefixes are stored as kind `raw`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceRow {
    /// Row id (`SQLite` rowid).
    pub id: i64,
    /// Finding key this row belongs to.
    pub key: String,
    /// Run-id that produced this evidence.
    pub run_id: String,
    /// Evidence kind (`recall`, `journal`, `pid`, `provfs`, `commit`, `path`, `raw`).
    pub kind: String,
    /// The ref value (everything after the `kind:` prefix, or the raw string).
    pub ref_val: String,
    /// Optional free-text note (reserved; always null for now).
    pub note: Option<String>,
    /// `RFC3339` timestamp when this row was inserted.
    pub seen_at: String,
}

/// Parse a raw `--evidence` string into `(kind, ref_val)`.
///
/// Known prefixes: `recall:`, `journal:`, `pid:`, `provfs:`, `commit:`, `path:`.
/// Anything else → kind `"raw"`, `ref_val` = full string.
///
/// Parsing is always lenient — even a malformed recall ULID is stored.
#[must_use]
pub fn parse_evidence_ref(raw: &str) -> (&'static str, &str) {
    const KNOWN: &[(&str, &str)] = &[
        ("recall:", "recall"),
        ("journal:", "journal"),
        ("pid:", "pid"),
        ("provfs:", "provfs"),
        ("commit:", "commit"),
        ("path:", "path"),
    ];
    for (prefix, kind) in KNOWN {
        if let Some(rest) = raw.strip_prefix(prefix) {
            return (kind, rest);
        }
    }
    ("raw", raw)
}

/// A single finding row from the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// Stable slug primary key.
    pub key: String,
    /// Human one-liner (latest report wins).
    pub title: String,
    /// Severity level.
    pub severity: Severity,
    /// Current status.
    pub status: Status,
    /// `RFC3339` timestamp of first report.
    pub first_seen: String,
    /// `RFC3339` timestamp of most recent report.
    pub last_seen: String,
    /// Run-id of the first report.
    pub first_run: String,
    /// Run-id of the most recent report.
    pub last_run: String,
    /// Count of distinct run-ids that reported this finding.
    pub runs_seen: i64,
    /// Current consecutive-run streak.
    pub consecutive_runs: i64,
    /// Raw report call count.
    pub report_count: i64,
    /// `RFC3339` timestamp when resolved, or null.
    pub resolved_at: Option<String>,
    /// Reason provided at resolution, or null.
    pub resolve_reason: Option<String>,
    /// Latest opaque evidence string (legacy `findings.evidence` column), or null.
    pub evidence: Option<String>,
    /// `RFC3339` timestamp when escalated, or null.
    pub escalated_at: Option<String>,
    /// Reason for escalation (cites threshold + SKILL.md §359), or null.
    pub escalation_reason: Option<String>,
    /// Typed evidence trail (populated by `db::show`; empty for `db::list`).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub evidence_trail: Vec<EvidenceRow>,
    /// Number of evidence rows for this finding (populated by `db::list`; 0 for `db::show`).
    #[serde(skip_serializing_if = "is_zero", default)]
    pub evidence_count: i64,
}

const fn is_zero(n: &i64) -> bool {
    #![allow(clippy::trivially_copy_pass_by_ref)]
    *n == 0
}

impl Finding {
    /// Format this finding for human-readable text output.
    ///
    /// When `evidence_trail` is non-empty the *Evidence* section is rendered
    /// grouped by `run_id` in chronological order (the DB preserves insertion
    /// order via the `id` rowid).
    #[must_use]
    pub fn format_text(&self) -> String {
        let resolved_info = match (&self.resolved_at, &self.resolve_reason) {
            (Some(at), Some(reason)) => format!("\n  resolved_at: {at}\n  reason: {reason}"),
            (Some(at), None) => format!("\n  resolved_at: {at}"),
            _ => String::new(),
        };
        let escalation_info = match (&self.escalated_at, &self.escalation_reason) {
            (Some(at), Some(reason)) => {
                format!("\n  escalated_at: {at}\n  escalation_reason: {reason}")
            }
            (Some(at), None) => format!("\n  escalated_at: {at}"),
            _ => String::new(),
        };

        // Legacy single-line evidence column (shown only if no structured trail).
        let legacy_evidence = if self.evidence_trail.is_empty() {
            self.evidence
                .as_ref()
                .map_or_else(String::new, |e| format!("\n  evidence: {e}"))
        } else {
            String::new()
        };

        // Structured evidence trail, grouped by run_id.
        let evidence_section = if self.evidence_trail.is_empty() {
            String::new()
        } else {
            let mut lines = String::from("\n  Evidence:");
            let mut last_run: Option<&str> = None;
            for row in &self.evidence_trail {
                if last_run != Some(row.run_id.as_str()) {
                    last_run = Some(row.run_id.as_str());
                }
                lines.push_str(&format!("\n    [{}] {}: {}", row.run_id, row.kind, row.ref_val));
            }
            lines
        };

        format!(
            "[{status}] {key} ({severity})\n  title: {title}\n  first_seen: {first_seen}\n  last_seen: {last_seen}\n  first_run: {first_run}\n  last_run: {last_run}\n  runs_seen: {runs_seen}  consecutive_runs: {consecutive_runs}  report_count: {report_count}{resolved_info}{escalation_info}{legacy_evidence}{evidence_section}",
            status = self.status,
            key = self.key,
            severity = self.severity,
            title = self.title,
            first_seen = self.first_seen,
            last_seen = self.last_seen,
            first_run = self.first_run,
            last_run = self.last_run,
            runs_seen = self.runs_seen,
            consecutive_runs = self.consecutive_runs,
            report_count = self.report_count,
        )
    }
}

/// A single run ledger row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunEntry {
    /// Run identifier (caller-supplied opaque string).
    pub run_id: String,
    /// Monotonic sequence number (1-based, in arrival order).
    pub seq: i64,
    /// `RFC3339` timestamp when this `run_id` was first seen.
    pub seen_at: String,
}
