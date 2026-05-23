//! spool — skill-telemetry CLI.

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::float_arithmetic,
    clippy::indexing_slicing,
    clippy::doc_markdown,
    clippy::redundant_closure_for_method_calls
)]

use chrono::Utc;
use clap::{Parser, Subcommand};
use skill_telemetry::{
    LogEndResult, RankEntry, load_events, log_end, log_start, parse_duration, rank, stale_skills,
};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(name = "spool", about = "Skill-telemetry: log + rank skill invocations")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Log a start or end event.
    Log {
        #[command(subcommand)]
        action: LogAction,
    },
    /// Print per-skill invocation ranking.
    Rank {
        /// Output format
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
        /// Override default spool root (~/.claude/spool/)
        #[arg(long)]
        root: Option<PathBuf>,
    },
    /// Report stale skills (no events within window).
    Report {
        /// Window like 30d / 7d / 12h
        #[arg(long)]
        stale: Option<String>,
        /// Override default spool root
        #[arg(long)]
        root: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
enum LogAction {
    /// Emit a start event; prints the generated invocation_id.
    Start {
        /// Skill name (e.g. self-review)
        skill: String,
        /// Optional args text (truncated to 200 chars)
        #[arg(long)]
        args: Option<String>,
        /// Override default spool root
        #[arg(long)]
        root: Option<PathBuf>,
    },
    /// Emit an end event for a prior start.
    End {
        /// The invocation_id printed by `log start`
        invocation_id: String,
        /// ok | error | interrupted
        #[arg(long)]
        outcome: String,
        /// Override default spool root
        #[arg(long)]
        root: Option<PathBuf>,
    },
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Text,
    Json,
}

fn default_root() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(format!("{home}/.claude/spool"))
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            let _ = e.print();
            return ExitCode::from(2);
        }
    };
    match cli.command {
        Command::Log { action } => match action {
            LogAction::Start { skill, args, root } => run_log_start(&skill, args.as_deref(), root),
            LogAction::End { invocation_id, outcome, root } => {
                run_log_end(&invocation_id, &outcome, root)
            }
        },
        Command::Rank { format, root } => run_rank(format, root),
        Command::Report { stale, root } => run_report(stale, root),
    }
}

fn run_log_start(skill: &str, args: Option<&str>, root: Option<PathBuf>) -> ExitCode {
    let root = root.unwrap_or_else(default_root);
    match log_start(&root, skill, args) {
        Ok(id) => {
            println!("{id}");
            ExitCode::from(0)
        }
        Err(e) => {
            eprintln!("spool: log start failed: {e}");
            ExitCode::from(2)
        }
    }
}

fn run_log_end(invocation_id: &str, outcome: &str, root: Option<PathBuf>) -> ExitCode {
    if !matches!(outcome, "ok" | "error" | "interrupted") {
        eprintln!("spool: --outcome must be one of: ok | error | interrupted");
        return ExitCode::from(2);
    }
    let root = root.unwrap_or_else(default_root);
    match log_end(&root, invocation_id, outcome) {
        Ok(LogEndResult::Paired { duration_ms: _ }) => ExitCode::from(0),
        Ok(LogEndResult::Orphaned) => {
            eprintln!("spool: orphaned end (no matching start for {invocation_id})");
            ExitCode::from(1)
        }
        Err(e) => {
            eprintln!("spool: log end failed: {e}");
            ExitCode::from(2)
        }
    }
}

fn run_rank(format: Format, root: Option<PathBuf>) -> ExitCode {
    let root = root.unwrap_or_else(default_root);
    let events = match load_events(&root) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("spool rank: {e}");
            return ExitCode::from(2);
        }
    };
    let rows = rank(&events);
    emit_rank(&rows, format);
    ExitCode::from(0)
}

fn emit_rank(rows: &[RankEntry], format: Format) {
    match format {
        Format::Json => {
            if let Ok(s) = serde_json::to_string(rows) {
                println!("{s}");
            }
        }
        Format::Text => {
            println!("{:<24} {:>11}  {:>10}  last_fired", "skill", "invocations", "mean_ms");
            for r in rows {
                let mean = r.mean_ms.map_or("—".to_string(), |m| m.to_string());
                let last = r.last_fired.map_or("never".to_string(), |t| t.to_rfc3339());
                println!("{:<24} {:>11}  {:>10}  {}", r.skill, r.invocations, mean, last);
            }
        }
    }
}

fn run_report(stale: Option<String>, root: Option<PathBuf>) -> ExitCode {
    let Some(window_str) = stale else {
        eprintln!("spool report: pass --stale <duration>");
        return ExitCode::from(2);
    };
    let Some(window) = parse_duration(&window_str) else {
        eprintln!("spool report: bad duration: {window_str}");
        return ExitCode::from(2);
    };
    let root = root.unwrap_or_else(default_root);
    let events = match load_events(&root) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("spool report: {e}");
            return ExitCode::from(2);
        }
    };
    let now = Utc::now();
    let stale_list = stale_skills(&events, now, window);
    for skill in stale_list {
        println!("{skill}");
    }
    ExitCode::from(0)
}
