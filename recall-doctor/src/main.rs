//! recall-doctor — health-check CLI for the v0.1 recall memory store.

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::doc_markdown,
    clippy::redundant_closure_for_method_calls
)]

use clap::{Parser, ValueEnum};
use recall_doctor::{doctor, invoke_reindex, render_json, render_text};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(name = "recall-doctor", about = "Divergence report for the recall memory store")]
struct Cli {
    /// Recall data root (default: ~/.claude/recall)
    #[arg(long)]
    root: Option<PathBuf>,
    /// Output format
    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,
    /// Attempt to fix divergence by invoking `recall reindex` (requires recall on PATH).
    #[arg(long, default_value_t = false)]
    fix: bool,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Text,
    Json,
}

fn default_root() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(format!("{home}/.claude/recall"))
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            let _ = e.print();
            return ExitCode::from(2);
        }
    };
    let root = cli.root.unwrap_or_else(default_root);
    if root.exists() && !root.is_dir() {
        eprintln!("recall-doctor: --root must be a directory: {}", root.display());
        return ExitCode::from(2);
    }
    let report = match doctor(&root) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("recall-doctor: {e}");
            return ExitCode::from(2);
        }
    };

    match cli.format {
        Format::Json => match render_json(&report) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("recall-doctor: render: {e}");
                return ExitCode::from(2);
            }
        },
        Format::Text => print!("{}", render_text(&report)),
    }

    let clean = report.is_clean();
    if cli.fix && !clean {
        match invoke_reindex(&root) {
            Ok(status) if status.success() => {
                eprintln!("recall-doctor: ran `recall reindex` successfully");
                return ExitCode::from(0);
            }
            Ok(_) => {
                eprintln!("recall-doctor: `recall reindex` exited non-zero");
                return ExitCode::from(1);
            }
            Err(e) => {
                eprintln!("recall-doctor: cannot invoke `recall`: {e}");
                return ExitCode::from(1);
            }
        }
    }

    if clean {
        ExitCode::from(0)
    } else {
        ExitCode::from(1)
    }
}
