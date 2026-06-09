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
    /// Digest: compact rollup of open/escalated findings for banners and health checks.
    ///
    /// Text output (default): ≤3-line banner safe to inline in `SessionStart`.
    /// JSON output: `wm.health.*`-compatible envelope (matches companion-degrade shape).
    Digest {
        /// Output format.
        #[arg(long, value_enum, default_value = "text")]
        format: Format,
        /// Minimum severity to include (default: include all).
        #[arg(long, value_enum)]
        severity: Option<SeverityFilter>,
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
        } => {
            let threshold = escalate_threshold.unwrap_or_else(escalate_threshold_default);
            let conn = db::open()?;
            // Pass None for the legacy single-line evidence column; typed refs go
            // into the `evidence` table via insert_evidence.
            db::report(&conn, &run, &key, &title, &severity, None, threshold)?;
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
            let findings = db::list(&conn, status_filter, min_sev)?;
            match format {
                Format::Json => {
                    println!("{}", serde_json::to_string_pretty(&findings)?);
                }
                Format::Text => {
                    for f in &findings {
                        println!("{}", f.format_text());
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
        Command::Digest { format, severity } => {
            let min_sev = severity.as_ref().map(SeverityFilter::rank);
            let conn = db::open()?;
            let findings = db::list_active(&conn, min_sev)?;
            let env = digest::compute(&findings);
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
    }
    Ok(())
}
