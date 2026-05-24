//! `self-portrait` CLI entry point.
//!
//! Subcommand `extract --file <repo-relative-path> [--repo <dir>] [--collapse-trivial]`
//! prints a single JSON [`Extract`] object on stdout describing every
//! commit that touched the named file. Exit codes:
//!
//! * `0` — success.
//! * `2` — `--repo` is not a git repository.
//! * `3` — the file never existed under any name in HEAD's history.
//! * `1` — any other error.

#![cfg_attr(not(test), forbid(unsafe_code))]
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc
)]

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use self_portrait::{Error, ExtractOptions, extract};

#[derive(Debug, Parser)]
#[command(
    name = "self-portrait",
    version,
    about = "Extract a single file's git history as structured JSON"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Walk a single file's history and emit per-commit records.
    Extract {
        /// Repository-relative path of the file to walk.
        #[arg(long)]
        file: String,
        /// Path to the git repository (defaults to current directory).
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Collapse runs of commits with empty diffs into a single
        /// synthetic `trivial` entry.
        #[arg(long)]
        collapse_trivial: bool,
    },
}

fn run() -> Result<(), Error> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Extract {
            file,
            repo,
            collapse_trivial,
        } => {
            let opts = ExtractOptions { collapse_trivial };
            let report = extract(&repo, &file, &opts)?;
            let json = serde_json::to_string(&report)?;
            println!("{json}");
            Ok(())
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(Error::NotARepository { path }) => {
            eprintln!("{}: not a git repository", path.display());
            ExitCode::from(2)
        }
        Err(Error::NoCommitsFound { path }) => {
            eprintln!("{}: no commits found", path.display());
            ExitCode::from(3)
        }
        Err(err) => {
            eprintln!("self-portrait: {err}");
            ExitCode::from(1)
        }
    }
}
