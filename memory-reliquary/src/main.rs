//! `reliquary` — annual recall-memory book composer (Phase 1a).
//!
//! Reads recall memories for a calendar year and writes a single Markdown
//! bundle with top-level frontmatter and one section per memory.

#![cfg_attr(not(test), forbid(unsafe_code))]
#![allow(clippy::print_stderr)]

use std::fs::File;
use std::io::{BufWriter, Write, stderr, stdout};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use memory_reliquary::{
    ComposeError, ComposeOptions, compose, default_recall_cmd, default_recall_root, expand_tilde,
};

/// CLI entry point.
#[derive(Parser, Debug)]
#[command(name = "reliquary", about = "Compose a year of recall memories into one Markdown bundle.", version)]
struct Cli {
    /// Subcommand to run.
    #[command(subcommand)]
    cmd: Cmd,
}

/// Subcommands.
#[derive(Subcommand, Debug)]
enum Cmd {
    /// Compose the bundle for a calendar year.
    Compose(ComposeArgs),
}

/// Arguments to `reliquary compose`.
#[derive(clap::Args, Debug)]
struct ComposeArgs {
    /// Calendar year (e.g. 2026).
    #[arg(long)]
    year: i32,
    /// Recall data root. Default: `$RECALL_HOME` or `~/.claude/recall`.
    #[arg(long)]
    recall_root: Option<PathBuf>,
    /// Output bundle path. Default: stdout.
    #[arg(long)]
    out: Option<PathBuf>,
    /// Subject prefix to filter by. Default: "user".
    #[arg(long, default_value = "user")]
    subject: String,
    /// Path to the `recall` binary (testing seam).
    #[arg(long)]
    recall_cmd: Option<PathBuf>,
    /// Add per-memory `mattered` + `dormancy_days` fields.
    #[arg(long, default_value_t = false)]
    include_recall_stats: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Compose(args) => run_compose(args),
    }
}

fn run_compose(args: ComposeArgs) -> ExitCode {
    let recall_root = args
        .recall_root
        .map_or_else(default_recall_root, |p| expand_tilde(&p));
    let recall_cmd = args
        .recall_cmd
        .map_or_else(default_recall_cmd, |p| expand_tilde(&p));

    let opts = ComposeOptions {
        year: args.year,
        recall_root,
        subject: args.subject,
        recall_cmd,
        include_recall_stats: args.include_recall_stats,
        generated_at: None,
    };

    // Acquire output writer.
    let stdout_handle = stdout();
    let mut err_handle = stderr();
    let result = if let Some(path) = args.out {
        match File::create(&path) {
            Ok(f) => {
                let mut buf = BufWriter::new(f);
                let r = compose(&opts, &mut buf, &mut err_handle);
                if let Err(e) = buf.flush() {
                    let _ = writeln!(err_handle, "ERROR: failed to flush bundle: {e}");
                    return ExitCode::from(1);
                }
                r
            }
            Err(e) => {
                let _ = writeln!(err_handle, "ERROR: failed to create {}: {e}", path.display());
                return ExitCode::from(1);
            }
        }
    } else {
        let mut buf = BufWriter::new(stdout_handle.lock());
        let r = compose(&opts, &mut buf, &mut err_handle);
        if let Err(e) = buf.flush() {
            let _ = writeln!(err_handle, "ERROR: failed to flush bundle: {e}");
            return ExitCode::from(1);
        }
        r
    };

    match result {
        Ok(_) => ExitCode::from(0),
        Err(ComposeError::RecallRootNotFound(p)) => {
            let _ = writeln!(err_handle, "ERROR: recall root not found: {}", p.display());
            ExitCode::from(2)
        }
        Err(e) => {
            let _ = writeln!(err_handle, "ERROR: {e}");
            ExitCode::from(1)
        }
    }
}
