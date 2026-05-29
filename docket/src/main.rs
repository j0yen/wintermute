//! docket — a structured ledger for standing findings.
//!
//! Provides `report`, `list`, `show`, and `resolve` subcommands backed by
//! a `SQLite` database at `$XDG_DATA_HOME/docket/docket.db`.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::process;

use clap::Parser;

mod cli;
mod db;
mod error;
mod model;

fn main() {
    let cli = cli::Cli::parse();
    if let Err(e) = cli::run(cli) {
        eprintln!("docket: {e}");
        process::exit(1);
    }
}
