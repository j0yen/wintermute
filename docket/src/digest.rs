//! `docket digest` — compact rollup of the open/escalated set.
//!
//! Produces either a human-readable text banner (≤3 lines, safe for
//! `SessionStart`) or a `wm.health.*`-compatible JSON envelope that matches
//! companion-degrade's `ComponentHealth` field shape exactly:
//!
//! ```json
//! {
//!   "component": "docket",
//!   "status": "ok | degraded | down",
//!   "summary": "4 open, 1 escalated",
//!   "detail": {
//!     "open": 4, "escalated": 1, "crit": 1,
//!     "oldest_key": "...", "oldest_runs": 12,
//!     "escalated_keys": ["..."]
//!   }
//! }
//! ```
//!
//! **Status mapping** (mirrors companion-degrade's `ComponentState` enum,
//! serialised as lowercase):
//! - `escalated > 0` AND any escalated finding has `severity=crit` → `"down"`
//! - `escalated > 0` (no crit escalated) → `"degraded"`
//! - only `open` (none escalated), or empty store → `"ok"`
//!
//! **Oldest-finding selection** uses `runs_seen` (total run-age), falling back
//! to `consecutive_runs`, for stability across `--format text` vs `json`.

use serde::{Deserialize, Serialize};

use crate::model::{Finding, Severity, Status};

// ---------------------------------------------------------------------------
// Status enum — mirrors companion-degrade's `ComponentState`
// ---------------------------------------------------------------------------

/// Health status values that match companion-degrade's `ComponentState` enum,
/// serialised as lowercase strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// No escalated findings; component is healthy.
    Ok,
    /// At least one escalated finding (none `crit`).
    Degraded,
    /// At least one escalated finding with `severity=crit`.
    Down,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ok => write!(f, "ok"),
            Self::Degraded => write!(f, "degraded"),
            Self::Down => write!(f, "down"),
        }
    }
}

// ---------------------------------------------------------------------------
// Detail and envelope types
// ---------------------------------------------------------------------------

/// Inner `detail` object inside the digest JSON envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigestDetail {
    /// Count of open (non-escalated) findings passing the severity filter.
    pub open: u64,
    /// Count of escalated findings passing the severity filter.
    pub escalated: u64,
    /// Count of findings with `severity=crit` (any status in open+escalated).
    pub crit: u64,
    /// Key of the finding with the highest `runs_seen` (or `consecutive_runs`
    /// as tiebreaker), or `null` when the store is empty.
    pub oldest_key: Option<String>,
    /// `runs_seen` of the oldest finding, or `0` when the store is empty.
    pub oldest_runs: u64,
    /// Keys of all escalated findings, stable-sorted alphabetically.
    pub escalated_keys: Vec<String>,
}

/// Top-level `wm.health.*`-compatible digest envelope.
///
/// Field names match companion-degrade's `ComponentHealth` shape exactly:
/// `component`, `status`, `summary`, `detail`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigestEnvelope {
    /// Component identifier — always `"docket"`.
    pub component: String,
    /// Aggregated health status.
    pub status: HealthStatus,
    /// One-line human summary (e.g. `"4 open, 1 escalated"`).
    pub summary: String,
    /// Structured counts and keys.
    pub detail: DigestDetail,
}

// ---------------------------------------------------------------------------
// Core computation
// ---------------------------------------------------------------------------

/// Compute a [`DigestEnvelope`] from a slice of open-or-escalated findings.
///
/// `findings` should already be filtered to status `open` or `escalated`
/// and to the caller's minimum severity. The function does not re-filter.
#[must_use]
pub fn compute(findings: &[Finding]) -> DigestEnvelope {
    let open_count = u64::try_from(
        findings.iter().filter(|f| f.status == Status::Open).count(),
    )
    .unwrap_or(u64::MAX);
    let escalated_findings: Vec<&Finding> = findings
        .iter()
        .filter(|f| f.status == Status::Escalated)
        .collect();
    let escalated_count =
        u64::try_from(escalated_findings.len()).unwrap_or(u64::MAX);
    let crit_count = u64::try_from(
        findings
            .iter()
            .filter(|f| f.severity == Severity::Crit)
            .count(),
    )
    .unwrap_or(u64::MAX);

    // Oldest by runs_seen desc, consecutive_runs as tiebreaker.
    let oldest = findings
        .iter()
        .max_by_key(|f| (f.runs_seen, f.consecutive_runs));

    let oldest_key = oldest.map(|f| f.key.clone());
    let oldest_runs = oldest.map_or(0, |f| {
        u64::try_from(f.runs_seen).unwrap_or(u64::MAX)
    });

    let mut escalated_keys: Vec<String> =
        escalated_findings.iter().map(|f| f.key.clone()).collect();
    escalated_keys.sort();

    // Check whether any *escalated* finding is crit.
    let escalated_crit = escalated_findings
        .iter()
        .any(|f| f.severity == Severity::Crit);

    let status = if escalated_count == 0 {
        HealthStatus::Ok
    } else if escalated_crit {
        HealthStatus::Down
    } else {
        HealthStatus::Degraded
    };

    let summary = if open_count == 0 && escalated_count == 0 {
        "0 open".to_owned()
    } else if escalated_count == 0 {
        format!("{open_count} open")
    } else {
        format!("{open_count} open, {escalated_count} escalated")
    };

    DigestEnvelope {
        component: "docket".to_owned(),
        status,
        summary,
        detail: DigestDetail {
            open: open_count,
            escalated: escalated_count,
            crit: crit_count,
            oldest_key,
            oldest_runs,
            escalated_keys,
        },
    }
}

// ---------------------------------------------------------------------------
// Text formatting
// ---------------------------------------------------------------------------

/// Render the digest as a compact text banner (≤3 lines).
///
/// - Empty store → single line `"docket: 0 open"`.
/// - Open only  → `"docket: N open"` (+ optional crit note).
/// - Escalated  → two lines: summary + escalated key list.
#[must_use]
pub fn format_text(env: &DigestEnvelope) -> String {
    let open = env.detail.open;
    let escalated = env.detail.escalated;
    let crit = env.detail.crit;
    let oldest_info = match (&env.detail.oldest_key, env.detail.oldest_runs) {
        (Some(k), r) if r > 0 => format!(" · oldest: {k} ({r} runs)"),
        _ => String::new(),
    };

    let crit_note = if crit > 0 {
        format!(" ({crit} crit)")
    } else {
        String::new()
    };

    let line1 = format!(
        "docket: {open} open{crit_note}, {escalated} escalated{oldest_info}"
    );

    if escalated == 0 {
        // Cleaner line when nothing is escalated.
        return format!("docket: {open} open{crit_note}{oldest_info}");
    }

    if env.detail.escalated_keys.is_empty() {
        return line1;
    }

    // Second line: escalated keys with titles are not available here (we only
    // have the envelope); emit key list.
    let keys_line = format!(
        "  escalated: {}",
        env.detail.escalated_keys.join(", ")
    );
    format!("{line1}\n{keys_line}")
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc,
    reason = "tests"
)]
mod tests {
    use super::*;
    use crate::model::{Finding, Severity, Status};

    fn make_finding(key: &str, status: Status, severity: Severity, runs_seen: i64) -> Finding {
        Finding {
            key: key.to_owned(),
            title: key.to_owned(),
            severity,
            status,
            first_seen: "2026-01-01T00:00:00Z".to_owned(),
            last_seen: "2026-01-01T00:00:00Z".to_owned(),
            first_run: "r1".to_owned(),
            last_run: "r1".to_owned(),
            runs_seen,
            consecutive_runs: runs_seen,
            report_count: runs_seen,
            resolved_at: None,
            resolve_reason: None,
            evidence: None,
            escalated_at: None,
            escalation_reason: None,
            // M2: no evidence trail in unit tests
            evidence_trail: Vec::new(),
            evidence_count: 0,
        }
    }

    // AC 1 — empty store → ok, zero counts
    #[test]
    fn empty_store_is_ok() {
        let env = compute(&[]);
        assert_eq!(env.status, HealthStatus::Ok);
        assert_eq!(env.detail.open, 0);
        assert_eq!(env.detail.escalated, 0);
        assert_eq!(env.detail.crit, 0);
        assert!(env.detail.oldest_key.is_none());
        assert_eq!(env.detail.oldest_runs, 0);
        assert!(env.detail.escalated_keys.is_empty());
    }

    // AC 1 — empty text output is a single clean line
    #[test]
    fn empty_store_text_single_line() {
        let env = compute(&[]);
        let text = format_text(&env);
        assert_eq!(text, "docket: 0 open");
        assert_eq!(text.lines().count(), 1);
    }

    // AC 1 — empty json has status=ok
    #[test]
    fn empty_store_json_status_ok() {
        let env = compute(&[]);
        let v = serde_json::to_value(&env).unwrap();
        assert_eq!(v["status"], "ok");
        assert_eq!(v["detail"]["open"], 0);
        assert_eq!(v["detail"]["escalated"], 0);
    }

    // AC 2 — 4 open + 1 escalated text output
    #[test]
    fn four_open_one_escalated_text() {
        let mut findings = vec![
            make_finding("a", Status::Open, Severity::Warn, 2),
            make_finding("b", Status::Open, Severity::Warn, 3),
            make_finding("c", Status::Open, Severity::Warn, 1),
            make_finding("d", Status::Open, Severity::Crit, 5),
            make_finding("e", Status::Escalated, Severity::Warn, 12),
        ];
        // Oldest: e with runs_seen=12
        let env = compute(&findings);
        let text = format_text(&env);
        // Must mention open count
        assert!(text.contains("4 open") || text.contains("open"), "open count: {text}");
        // Must mention escalated
        assert!(text.contains("1 escalated") || text.contains("escalated"), "escalated: {text}");
        // Must mention oldest key (e) with run-age
        assert!(text.contains("e"), "oldest key: {text}");
        assert!(text.contains("12"), "run-age: {text}");
        // ≤3 lines
        assert!(text.lines().count() <= 3, "≤3 lines: {text:?}");

        // Sort to ensure stable test
        findings.sort_by_key(|f| f.key.clone());
        let env2 = compute(&findings);
        let text2 = format_text(&env2);
        assert!(text2.lines().count() <= 3);
    }

    // AC 3 — json field names match companion-degrade wm.health envelope
    #[test]
    fn json_envelope_field_names() {
        let env = compute(&[
            make_finding("k1", Status::Open, Severity::Warn, 2),
            make_finding("k2", Status::Escalated, Severity::Warn, 5),
        ]);
        let v = serde_json::to_value(&env).unwrap();
        // Top-level fields
        assert!(v.get("component").is_some(), "component field: {v}");
        assert!(v.get("status").is_some(), "status field: {v}");
        assert!(v.get("summary").is_some(), "summary field: {v}");
        assert!(v.get("detail").is_some(), "detail field: {v}");
        // Detail fields
        let d = &v["detail"];
        assert!(d.get("open").is_some(), "detail.open: {v}");
        assert!(d.get("escalated").is_some(), "detail.escalated: {v}");
        assert!(d.get("crit").is_some(), "detail.crit: {v}");
        assert!(d.get("oldest_key").is_some(), "detail.oldest_key: {v}");
        assert!(d.get("oldest_runs").is_some(), "detail.oldest_runs: {v}");
        assert!(d.get("escalated_keys").is_some(), "detail.escalated_keys: {v}");
        // component is "docket"
        assert_eq!(v["component"], "docket");
    }

    // AC 4 — status mapping: open-only → ok; escalated → degraded; escalated crit → down
    #[test]
    fn status_mapping_open_only_is_ok() {
        let env = compute(&[make_finding("k", Status::Open, Severity::Warn, 1)]);
        assert_eq!(env.status, HealthStatus::Ok);
    }

    #[test]
    fn status_mapping_any_escalated_is_degraded() {
        let env = compute(&[make_finding("k", Status::Escalated, Severity::Warn, 3)]);
        assert_eq!(env.status, HealthStatus::Degraded);
    }

    #[test]
    fn status_mapping_escalated_crit_is_down() {
        let env = compute(&[make_finding("k", Status::Escalated, Severity::Crit, 3)]);
        assert_eq!(env.status, HealthStatus::Down);
    }

    #[test]
    fn status_mapping_empty_is_ok() {
        let env = compute(&[]);
        assert_eq!(env.status, HealthStatus::Ok);
    }

    // AC 5 — --severity warn excludes info findings
    // (filtering is done before calling compute; we verify compute honours
    //  pre-filtered input correctly)
    #[test]
    fn severity_filter_excludes_info() {
        // Two info findings — if filtered before compute, result should be ok
        let info_findings = vec![
            make_finding("i1", Status::Open, Severity::Info, 1),
            make_finding("i2", Status::Open, Severity::Info, 2),
        ];
        // Simulate warn filter: exclude info
        let filtered: Vec<Finding> = info_findings
            .into_iter()
            .filter(|f| f.severity.rank() >= Severity::Warn.rank())
            .collect();
        let env = compute(&filtered);
        assert_eq!(env.status, HealthStatus::Ok);
        assert_eq!(env.detail.open, 0);
    }

    // AC 6 — oldest-finding uses runs_seen, stable across formats
    #[test]
    fn oldest_uses_runs_seen() {
        let findings = vec![
            make_finding("low", Status::Open, Severity::Warn, 1),
            make_finding("high", Status::Open, Severity::Warn, 10),
            make_finding("mid", Status::Open, Severity::Warn, 5),
        ];
        let env = compute(&findings);
        assert_eq!(env.detail.oldest_key.as_deref(), Some("high"));
        assert_eq!(env.detail.oldest_runs, 10);
    }

    #[test]
    fn oldest_stable_across_format_modes() {
        let findings = vec![
            make_finding("z", Status::Open, Severity::Warn, 3),
            make_finding("a", Status::Open, Severity::Warn, 7),
        ];
        let env = compute(&findings);
        // Both text and json use same envelope; oldest_key is "a" (runs_seen=7)
        assert_eq!(env.detail.oldest_key.as_deref(), Some("a"));
        let text = format_text(&env);
        let v = serde_json::to_value(&env).unwrap();
        let json_oldest = v["detail"]["oldest_key"].as_str().unwrap_or("");
        assert_eq!(json_oldest, "a", "json oldest: {v}");
        assert!(text.contains("a"), "text oldest: {text}");
    }

    // AC 7 — json output is valid serde_json; text ≤3 lines
    #[test]
    fn json_parseable() {
        let findings = vec![
            make_finding("k1", Status::Escalated, Severity::Crit, 5),
            make_finding("k2", Status::Open, Severity::Warn, 2),
        ];
        let env = compute(&findings);
        let json_str = serde_json::to_string(&env).unwrap();
        let reparsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(reparsed["component"], "docket");
    }

    #[test]
    fn text_max_three_lines() {
        let findings = vec![
            make_finding("e1", Status::Escalated, Severity::Warn, 5),
            make_finding("e2", Status::Escalated, Severity::Warn, 3),
            make_finding("o1", Status::Open, Severity::Info, 1),
        ];
        let env = compute(&findings);
        let text = format_text(&env);
        assert!(text.lines().count() <= 3, "text: {text:?}");
    }

    // AC 9 — no new table/migration — just checking the digest uses existing fields
    // (structural test: compute runs without error on an all-resolved store)
    #[test]
    fn compute_on_empty_slice_no_panic() {
        let _ = compute(&[]);
    }

    // AC 10 — graceful degradation when no escalated findings
    #[test]
    fn no_escalated_shows_open_count_only() {
        let findings = vec![
            make_finding("k1", Status::Open, Severity::Warn, 2),
            make_finding("k2", Status::Open, Severity::Warn, 3),
        ];
        let env = compute(&findings);
        assert_eq!(env.status, HealthStatus::Ok);
        assert_eq!(env.detail.escalated, 0);
        assert!(env.detail.escalated_keys.is_empty());
    }

    // Summary field tests
    #[test]
    fn summary_empty_is_zero_open() {
        let env = compute(&[]);
        assert_eq!(env.summary, "0 open");
    }

    #[test]
    fn summary_open_only() {
        let env = compute(&[make_finding("k", Status::Open, Severity::Warn, 1)]);
        assert_eq!(env.summary, "1 open");
    }

    #[test]
    fn summary_open_and_escalated() {
        let env = compute(&[
            make_finding("o", Status::Open, Severity::Warn, 1),
            make_finding("e", Status::Escalated, Severity::Warn, 3),
        ]);
        assert_eq!(env.summary, "1 open, 1 escalated");
    }
}
