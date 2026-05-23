//! recall-io — CLI for export + import of the v0.1 recall store.

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::doc_markdown,
    clippy::redundant_closure_for_method_calls,
    clippy::needless_pass_by_value,
    clippy::manual_let_else,
    clippy::single_match_else
)]

use clap::{Parser, Subcommand, ValueEnum};
use recall_io::{export, import};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(name = "recall-io", about = "Export + import for the v0.1 recall store (NDJSON)")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Emit every memory to stdout as NDJSON (one per line).
    Export {
        #[arg(long)]
        root: Option<PathBuf>,
    },
    /// Read NDJSON from a file (or `-` for stdin) and add to the store.
    Import {
        /// Path to the .jsonl file, or `-` to read stdin.
        input: String,
        #[arg(long, default_value_t = false)]
        replace: bool,
        #[arg(long)]
        root: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
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

fn ensure_dir(root: &std::path::Path) -> Option<ExitCode> {
    if root.exists() && !root.is_dir() {
        eprintln!("recall-io: --root must be a directory: {}", root.display());
        return Some(ExitCode::from(2));
    }
    None
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
        Command::Export { root } => run_export(root.unwrap_or_else(default_root)),
        Command::Import { input, replace, root, format } => {
            run_import(&input, replace, root.unwrap_or_else(default_root), format)
        }
    }
}

fn run_export(root: PathBuf) -> ExitCode {
    if let Some(c) = ensure_dir(&root) { return c; }
    let mut stdout = std::io::stdout().lock();
    match export(&root, &mut stdout) {
        Ok(_) => ExitCode::from(0),
        Err(e) => {
            eprintln!("recall-io export: {e}");
            ExitCode::from(2)
        }
    }
}

fn run_import(input: &str, replace: bool, root: PathBuf, format: Format) -> ExitCode {
    if let Some(c) = ensure_dir(&root) { return c; }
    let result = if input == "-" {
        let stdin = std::io::stdin().lock();
        import(&root, stdin, replace)
    } else {
        let f = match std::fs::File::open(input) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("recall-io import: cannot open {input}: {e}");
                return ExitCode::from(2);
            }
        };
        import(&root, f, replace)
    };
    match result {
        Ok(summary) => {
            match format {
                Format::Json => {
                    if let Ok(s) = serde_json::to_string(&summary) {
                        println!("{s}");
                    }
                }
                Format::Text => {
                    println!(
                        "imported={} skipped={} errors={}",
                        summary.imported, summary.skipped, summary.errors
                    );
                }
            }
            if summary.imported == 0 && summary.errors > 0 && summary.skipped == 0 {
                ExitCode::from(1)
            } else {
                ExitCode::from(0)
            }
        }
        Err(e) => {
            eprintln!("recall-io import: {e}");
            ExitCode::from(2)
        }
    }
}
