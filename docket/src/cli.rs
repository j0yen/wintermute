//! CLI argument parsing and command dispatch for docket.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use clap::{Parser, Subcommand, ValueEnum};

use crate::db;
use crate::digest;
use crate::error::Result;

/// A structured ledger for standing findings.
#[derive(Debug, Parser)]
#[command(name = "docket", version, about, long_about = None)]
pub(crate) struct Cli {
    /// Subcommand to execute.
    #[command(subcommand)]
    pub(crate) command: Command,
}

/// Output format for list and show commands.
#[derive(Debug, Clone, ValueEnum)]
pub(crate) enum Format {
    /// Human-readable text.
    Text,
    /// JSON array / object.
    Json,
}

/// Minimum severity filter for list.
#[derive(Debug, Clone, ValueEnum)]
pub(crate) enum SeverityFilter {
    /// Include info and above.
    Info,
    /// Include warn and above.
    Warn,
    /// Include crit only.
    Crit,
}

impl SeverityFilter {
    const fn rank(&self) -> u8 {
        match self {
            Self::Info => 0,
            Self::Warn => 1,
            Self::Crit => 2,
        }
    }
}

/// Available docket subcommands.
#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Report (upsert) a finding.
    Report {
        /// Run identifier — caller-supplied opaque string (e.g. 2026-05-29.1).
        #[arg(long)]
        run: String,
        /// Stable finding key (slug).
        #[arg(long)]
        key: String,
        /// Human-readable one-liner title.
        #[arg(long)]
        title: String,
        /// Severity level (default: warn).
        #[arg(long, default_value = "warn")]
        severity: String,
        /// Typed evidence reference(s) — repeatable.
        ///
        /// Format: `<kind>:<ref>`.  Known kinds: `recall`, `journal`, `pid`,
        /// `provfs`, `commit`, `path`.  Unknown prefixes stored as `raw`.
        /// Malformed refs are never rejected — they are stored as-given.
        #[arg(long, action = clap::ArgAction::Append)]
        evidence: Vec<String>,
        /// Escalation threshold: escalate after this many consecutive runs.
        /// Overrides `DOCKET_ESCALATE_THRESHOLD` env var (default 3).
        #[arg(long)]
        escalate_threshold: Option<i64>,
        /// Opaque fingerprint for ack auto-clear (M3).
        ///
        /// If the finding is currently acked with an `--until-change` fingerprint
        /// and this value differs, the ack is automatically cleared so the finding
        /// resurfaces.
        #[arg(long)]
        fingerprint: Option<String>,
    },
    /// List findings.
    List {
        /// Show only open findings (default).
        #[arg(long, conflicts_with_all = ["resolved", "escalated", "all"])]
        open: bool,
        /// Show only resolved findings.
        #[arg(long, conflicts_with_all = ["open", "escalated", "all"])]
        resolved: bool,
        /// Show only escalated findings.
        #[arg(long, conflicts_with_all = ["open", "resolved", "all"])]
        escalated: bool,
        /// Show all findings.
        #[arg(long, conflicts_with_all = ["open", "resolved", "escalated"])]
        all: bool,
        /// Output format.
        #[arg(long, value_enum, default_value = "text")]
        format: Format,
        /// Minimum severity to include.
        #[arg(long, value_enum)]
        severity: Option<SeverityFilter>,
        /// Include acknowledged findings in the listing (shows them with an (acked) marker).
        ///
        /// By default, acked findings are hidden from open/escalated views.
        #[arg(long)]
        include_acked: bool,
    },
    /// Show a single finding's full record.
    Show {
        /// Finding key.
        key: String,
        /// Output format.
        #[arg(long, value_enum, default_value = "text")]
        format: Format,
    },
    /// Resolve a finding.
    Resolve {
        /// Finding key.
        key: String,
        /// Optional reason for resolution.
        #[arg(long)]
        reason: Option<String>,
    },
    /// Acknowledge a finding — marks it as triaged and hides it from default views.
    ///
    /// The finding remains fully tracked; `--until-change FINGERPRINT` or
    /// `--runs N` will auto-clear the ack when the condition changes.
    Ack {
        /// Finding key.
        key: String,
        /// Optional human reason for acknowledging.
        #[arg(long)]
        reason: Option<String>,
        /// Fingerprint to watch; the ack is cleared if a future `report
        /// --fingerprint FP` supplies a different value.
        #[arg(long)]
        until_change: Option<String>,
        /// Clear ack after this many additional runs (stored as run-id).
        #[arg(long)]
        runs: Option<String>,
    },
    /// Clear an acknowledgement — finding resurfaces immediately.
    Unack {
        /// Finding key.
        key: String,
    },
    /// Digest: compact rollup of open/escalated findings for banners and health checks.
    ///
    /// Text output (default): ≤3-line banner safe to inline in `SessionStart`.
    /// JSON output: `wm.health.*`-compatible envelope (matches companion-degrade shape).
    /// Acked findings are excluded from counts by default; use `--include-acked` to see all.
    Digest {
        /// Output format.
        #[arg(long, value_enum, default_value = "text")]
        format: Format,
        /// Minimum severity to include (default: include all).
        #[arg(long, value_enum)]
        severity: Option<SeverityFilter>,
        /// Include acknowledged findings in counts and listing.
        ///
        /// With this flag, acked findings are counted like open/escalated ones,
        /// and the output is byte-identical to the pre-abide digest for the same data.
        #[arg(long)]
        include_acked: bool,
    },
    /// Sweep: auto-resolve stale open/escalated findings not seen in recent runs.
    ///
    /// Marks every open/escalated finding whose `last_run` differs from <run>
    /// and whose absence spans >= `stale_after` runs as resolved(stale).
    /// Also resets `consecutive_runs` to 0 for findings with a gap (< `stale_after`).
    Sweep {
        /// Current run identifier (will be recorded in the runs ledger).
        #[arg(long)]
        run: String,
        /// Number of elapsed runs before a finding is considered stale.
        /// Overrides `DOCKET_STALE_AFTER` env var (default 3).
        #[arg(long)]
        stale_after: Option<i64>,
    },
    /// List probe-suspect findings: open findings with high report+run counts that
    /// were never resolved.  Useful for identifying probes that may be reporting
    /// phantom issues or need logic review.
    ///
    /// Excludes acked findings by default; use `--include-acked` to see them.
    /// Excludes resolved findings (always).
    Stuck {
        /// Minimum report_count threshold (default 6).
        #[arg(long, default_value = "6")]
        min_reports: i64,
        /// Minimum runs_seen threshold (default 5).
        #[arg(long, default_value = "5")]
        min_runs: i64,
        /// Output format.
        #[arg(long, value_enum, default_value = "text")]
        format: Format,
        /// Filter to findings whose key contains this substring.
        #[arg(long)]
        key: Option<String>,
        /// Include acknowledged findings (excluded by default).
        #[arg(long)]
        include_acked: bool,
    },
}

/// Resolve `DOCKET_ESCALATE_THRESHOLD` env var, defaulting to 3.
fn escalate_threshold_default() -> i64 {
    std::env::var("DOCKET_ESCALATE_THRESHOLD")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(3)
}

/// Resolve `DOCKET_STALE_AFTER` env var, defaulting to 3.
fn stale_after_default() -> i64 {
    std::env::var("DOCKET_STALE_AFTER")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(3)
}

/// Run the CLI command.
///
/// # Errors
///
/// Returns an error if the database operation fails or a key is not found.
#[allow(clippy::too_many_lines)]
pub(crate) fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Report {
            run,
            key,
            title,
            severity,
            evidence,
            escalate_threshold,
            fingerprint,
        } => {
            let threshold = escalate_threshold.unwrap_or_else(escalate_threshold_default);
            let conn = db::open()?;
            // Pass None for the legacy single-line evidence column; typed refs go
            // into the `evidence` table via insert_evidence.
            db::report(
                &conn,
                &run,
                &key,
                &title,
                &severity,
                None,
                threshold,
                fingerprint.as_deref(),
            )?;
            // Append typed evidence refs (M2) — lenient, never fails for bad syntax.
            if !evidence.is_empty() {
                db::insert_evidence(&conn, &key, &run, &evidence)?;
            }
        }
        Command::List {
            open: _open,
            resolved,
            escalated,
            all,
            format,
            severity,
            include_acked,
        } => {
            let status_filter = if resolved {
                "resolved"
            } else if escalated {
                "escalated"
            } else if all {
                "all"
            } else {
                // Default: open (handles both explicit --open and the default)
                "open"
            };
            let min_sev = severity.as_ref().map(SeverityFilter::rank);
            let conn = db::open()?;
            let findings = db::list(&conn, status_filter, min_sev, include_acked)?;
            match format {
                Format::Json => {
                    println!("{}", serde_json::to_string_pretty(&findings)?);
                }
                Format::Text => {
                    for f in &findings {
                        let text = f.format_text();
                        if f.is_acked() {
                            println!("{text} (acked)");
                        } else {
                            println!("{text}");
                        }
                    }
                }
            }
        }
        Command::Show { key, format } => {
            let conn = db::open()?;
            let finding = db::show(&conn, &key)?;
            match format {
                Format::Json => {
                    println!("{}", serde_json::to_string_pretty(&finding)?);
                }
                Format::Text => {
                    println!("{}", finding.format_text());
                }
            }
        }
        Command::Resolve { key, reason } => {
            let conn = db::open()?;
            db::resolve(&conn, &key, reason.as_deref())?;
        }
        Command::Ack {
            key,
            reason,
            until_change,
            runs: _runs,
        } => {
            let conn = db::open()?;
            db::ack(&conn, &key, reason.as_deref(), until_change.as_deref(), None)?;
            eprintln!("docket: acked '{key}'");
        }
        Command::Unack { key } => {
            let conn = db::open()?;
            let was_acked = db::unack(&conn, &key)?;
            if was_acked {
                eprintln!("docket: unacked '{key}'");
            } else {
                eprintln!("docket: '{key}' was not acked (no-op)");
            }
        }
        Command::Digest {
            format,
            severity,
            include_acked,
        } => {
            let min_sev = severity.as_ref().map(SeverityFilter::rank);
            let conn = db::open()?;
            let env = if include_acked {
                // --include-acked: pass all findings as non-acked; acked slice empty.
                let findings = db::list_active(&conn, min_sev, true)?;
                digest::compute(&findings, &[])
            } else {
                // Default: split into acked and non-acked.
                let non_acked = db::list_active(&conn, min_sev, false)?;
                // Fetch acked findings for the tally (separate query).
                let all_active = db::list_active(&conn, min_sev, true)?;
                let acked: Vec<_> = all_active.into_iter().filter(crate::model::Finding::is_acked).collect();
                digest::compute(&non_acked, &acked)
            };
            match format {
                Format::Json => {
                    println!("{}", serde_json::to_string_pretty(&env)?);
                }
                Format::Text => {
                    println!("{}", digest::format_text(&env));
                }
            }
        }
        Command::Sweep { run, stale_after } => {
            let stale = stale_after.unwrap_or_else(stale_after_default);
            let conn = db::open()?;
            let result = db::sweep(&conn, &run, stale)?;
            eprintln!(
                "sweep complete: resolved={} streak_reset={}",
                result.resolved, result.streak_reset
            );
        }
        Command::Stuck {
            min_reports,
            min_runs,
            format,
            key,
            include_acked,
        } => {
            let conn = db::open()?;
            let findings = db::stuck(&conn, min_reports, min_runs, key.as_deref(), include_acked)?;
            if findings.is_empty() {
                println!("no probe-suspect findings");
                return Ok(());
            }
            match format {
                Format::Json => {
                    let rows: Vec<serde_json::Value> = findings
                        .iter()
                        .map(|f| {
                            let reason = format!(
                                "reported {report_count}\u{00d7} over {runs_seen} runs, \
                                 never resolved \u{2014} verify the probe matches reality \
                                 before re-parking",
                                report_count = f.report_count,
                                runs_seen = f.runs_seen,
                            );
                            serde_json::json!({
                                "key": f.key,
                                "title": f.title,
                                "report_count": f.report_count,
                                "runs_seen": f.runs_seen,
                                "consecutive_runs": f.consecutive_runs,
                                "first_seen": f.first_seen,
                                "last_seen": f.last_seen,
                                "suspect_reason": reason,
                            })
                        })
                        .collect();
                    println!("{}", serde_json::to_string_pretty(&rows)?);
                }
                Format::Text => {
                    for f in &findings {
                        let ack_marker = if f.is_acked() { " (acked)" } else { "" };
                        println!(
                            "[suspect] {} — {} reports / {} runs{ack_marker}\n  {}",
                            f.key, f.report_count, f.runs_seen, f.title,
                        );
                    }
                }
            }
        }
    }
    Ok(())
}
