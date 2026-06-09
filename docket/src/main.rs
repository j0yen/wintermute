//! docket — a structured ledger for standing findings.
//!
//! Provides `report`, `list`, `show`, and `resolve` subcommands backed by
//! a `SQLite` database at `$XDG_DATA_HOME/docket/docket.db`.

// Binary crate: pub vs pub(crate) visibility rules differ from library crates.
// `unreachable_pub` fires on items that won't be re-exported; suppress it.
// `redundant_pub_crate` fires on pub(crate) inside private modules; suppress it.
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    unreachable_pub,
    clippy::redundant_pub_crate
)]

use std::process;

use clap::Parser;

mod cli;
mod db;
mod digest;
mod error;
mod model;

fn main() {
    let cli = cli::Cli::parse();
    if let Err(e) = cli::run(cli) {
        eprintln!("docket: {e}");
        process::exit(1);
    }
}
