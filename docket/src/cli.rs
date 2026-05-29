//! CLI argument parsing and command dispatch for docket.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use clap::{Parser, Subcommand, ValueEnum};

use crate::db;
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
        /// Optional evidence reference (raw string, stored for future docket-evidence).
        #[arg(long)]
        evidence: Option<String>,
    },
    /// List findings.
    List {
        /// Show only open findings (default).
        #[arg(long, conflicts_with_all = ["resolved", "all"])]
        open: bool,
        /// Show only resolved findings.
        #[arg(long, conflicts_with_all = ["open", "all"])]
        resolved: bool,
        /// Show all findings.
        #[arg(long, conflicts_with_all = ["open", "resolved"])]
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
}

/// Run the CLI command.
///
/// # Errors
///
/// Returns an error if the database operation fails or a key is not found.
pub(crate) fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Report {
            run,
            key,
            title,
            severity,
            evidence,
        } => {
            let conn = db::open()?;
            db::report(&conn, &run, &key, &title, &severity, evidence.as_deref())?;
        }
        Command::List {
            open: _open,
            resolved,
            all,
            format,
            severity,
        } => {
            let status_filter = if resolved {
                "resolved"
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
    }
    Ok(())
}
