//! tide-chart CLI — `collect` + `chart` subcommands.
//!
//! See `src/lib.rs` for the underlying library surface.

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::doc_markdown
)]

use chrono::Utc;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;
use tide_chart::{
    Store, collect, default_root, render_30day, render_7day, render_today,
};

#[derive(Parser, Debug)]
#[command(name = "tide-chart", about = "Instrument-like ASCII view of a day's rhythm")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Read events from <root>/events/ and write z-scored signals to <root>/tide.db.
    Collect {
        /// Override default root (default: $XDG_DATA_HOME/tide-chart).
        #[arg(long)]
        root: Option<PathBuf>,
    },
    /// Render the ASCII chart from <root>/tide.db to stdout.
    Chart {
        /// Today (last 24 hours, 4 lines x 24 cols).
        #[arg(long, conflicts_with_all = ["seven_day", "thirty_day"])]
        today: bool,
        /// 7-day strip (4 lines x 7 cols, daily means).
        #[arg(long = "7day", conflicts_with_all = ["today", "thirty_day"])]
        seven_day: bool,
        /// 30-day heat strip (1 line x 30 cells).
        #[arg(long = "30day", conflicts_with_all = ["today", "seven_day"])]
        thirty_day: bool,
        /// Override default root.
        #[arg(long)]
        root: Option<PathBuf>,
    },
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
        Command::Collect { root } => run_collect(root),
        Command::Chart { today, seven_day, thirty_day, root } => {
            run_chart(today, seven_day, thirty_day, root)
        }
    }
}

fn run_collect(root: Option<PathBuf>) -> ExitCode {
    let root = root.unwrap_or_else(default_root);
    match collect(&root, Utc::now()) {
        Ok(_) => ExitCode::from(0),
        Err(e) => {
            eprintln!("tide-chart collect: {e}");
            ExitCode::from(2)
        }
    }
}

fn run_chart(
    today: bool,
    seven_day: bool,
    thirty_day: bool,
    root: Option<PathBuf>,
) -> ExitCode {
    let root = root.unwrap_or_else(default_root);
    let db_path = root.join("tide.db");
    if !db_path.exists() {
        eprintln!(
            "tide-chart chart: no database at {} — run collect first",
            db_path.display()
        );
        return ExitCode::from(2);
    }
    let store = match Store::open(&db_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("tide-chart chart: open {}: {e}", db_path.display());
            return ExitCode::from(2);
        }
    };
    if let Err(e) = store.migrate() {
        eprintln!("tide-chart chart: migrate: {e}");
        return ExitCode::from(2);
    }
    let rows = match store.all_rows() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("tide-chart chart: query: {e}");
            return ExitCode::from(2);
        }
    };
    let now = Utc::now();
    let out = if thirty_day {
        render_30day(&rows, now)
    } else if seven_day {
        render_7day(&rows, now)
    } else {
        // default + --today both map to the 24h view
        let _ = today;
        render_today(&rows, now)
    };
    print!("{out}");
    ExitCode::from(0)
}
