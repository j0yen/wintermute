//! `repo-as-landscape` CLI entry point.
//!
//! Subcommand `walk --repo <git-dir>` prints a JSON [`Landscape`] on
//! stdout. With `--summary`, prints a one-line-per-language summary on
//! stderr first. With `--coalesce-below <N>`, collapses tiny
//! directories into synthetic entries.

#![cfg_attr(not(test), forbid(unsafe_code))]
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc
)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use repo_as_landscape::{Error, Landscape, WalkOptions, walk_repo};

#[derive(Debug, Parser)]
#[command(name = "repo-as-landscape", version, about = "Extract topographic primitives from a git repo")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Walk the HEAD tree and emit per-file landscape JSON.
    Walk {
        /// Path to the git repository.
        #[arg(long)]
        repo: PathBuf,
        /// Also print a per-language summary on stderr.
        #[arg(long)]
        summary: bool,
        /// Collapse directories with fewer than N tracked files.
        #[arg(long, value_name = "N")]
        coalesce_below: Option<u32>,
    },
}

fn run() -> Result<(), Error> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Walk {
            repo,
            summary,
            coalesce_below,
        } => {
            let opts = WalkOptions { coalesce_below };
            let landscape = walk_repo(&repo, &opts)?;
            if summary {
                print_summary_stderr(&landscape);
            }
            let json = serde_json::to_string(&landscape)?;
            println!("{json}");
            Ok(())
        }
    }
}

fn print_summary_stderr(landscape: &Landscape) {
    let mut by_lang: BTreeMap<&str, (u32, u32)> = BTreeMap::new();
    for f in &landscape.files {
        let e = by_lang.entry(f.language.as_str()).or_insert((0, 0));
        e.0 = e.0.saturating_add(1);
        e.1 = e.1.saturating_add(f.commit_count);
    }
    for (lang, (file_count, commit_total)) in by_lang {
        eprintln!("{lang}: {file_count} files, {commit_total} commits");
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(Error::NotARepository { path }) => {
            eprintln!("{}: not a git repository", path.display());
            ExitCode::from(2)
        }
        Err(err) => {
            eprintln!("repo-as-landscape: {err}");
            ExitCode::from(1)
        }
    }
}
