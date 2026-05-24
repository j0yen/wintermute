//! `letter` — curation CLI for letters-we-never-sent.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use clap::Parser;
use letters_we_never_sent::cli::{self, Cli, Command};
use letters_we_never_sent::state::State;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match dispatch(cli.command) {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(e.exit_code())
        }
    }
}

fn dispatch(cmd: Command) -> letters_we_never_sent::LetterResult<u8> {
    match cmd {
        Command::List(a) => {
            let s = cli::run_list(a)?;
            print!("{s}");
            Ok(0)
        }
        Command::Stats(a) => {
            let s = cli::run_stats(a)?;
            print!("{s}");
            Ok(0)
        }
        Command::Accept(a) => {
            cli::run_mutate(a, State::Accepted)?;
            Ok(0)
        }
        Command::Decline(a) => {
            cli::run_mutate(a, State::Declined)?;
            Ok(0)
        }
        Command::MarkSendReal(a) => {
            cli::run_mutate(a, State::SendReal)?;
            Ok(0)
        }
        Command::Open(a) => {
            let c = cli::run_open(a)?;
            let clamped = c.clamp(0, 255);
            Ok(u8::try_from(clamped).unwrap_or(1))
        }
    }
}
