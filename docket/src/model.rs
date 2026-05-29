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
    /// The finding has been resolved.
    Resolved,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open => write!(f, "open"),
            Self::Resolved => write!(f, "resolved"),
        }
    }
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
    /// Latest evidence reference (raw string), or null.
    pub evidence: Option<String>,
}

impl Finding {
    /// Format this finding for human-readable text output.
    #[must_use]
    pub fn format_text(&self) -> String {
        let resolved_info = match (&self.resolved_at, &self.resolve_reason) {
            (Some(at), Some(reason)) => format!("\n  resolved_at: {at}\n  reason: {reason}"),
            (Some(at), None) => format!("\n  resolved_at: {at}"),
            _ => String::new(),
        };
        let evidence_info = self
            .evidence
            .as_ref()
            .map_or_else(String::new, |e| format!("\n  evidence: {e}"));
        format!(
            "[{status}] {key} ({severity})\n  title: {title}\n  first_seen: {first_seen}\n  last_seen: {last_seen}\n  first_run: {first_run}\n  last_run: {last_run}\n  runs_seen: {runs_seen}  consecutive_runs: {consecutive_runs}  report_count: {report_count}{resolved_info}{evidence_info}",
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
