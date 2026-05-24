//! morsel-bake — CLI entry point.
//!
//! Parses flags, reads the safetensors input, hands the bytes to
//! [`morsel_bake::bake`], and writes the rendered Rust source to
//! `--out`. Exit codes are dictated by the acceptance contract — see
//! [`morsel_bake::BakeError::exit_code`].

#![cfg_attr(not(test), forbid(unsafe_code))]
#![allow(clippy::print_stderr)] // CLI tool — stderr is the user channel.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use morsel_bake::{BakeError, BakeOptions, bake};

/// CLI args, parsed by clap-derive.
#[derive(Debug, Parser)]
#[command(
    name = "morsel-bake",
    version,
    about = "Bake safetensors weights into a standalone Rust source file."
)]
struct Args {
    /// Input safetensors file.
    #[arg(long = "in", value_name = "PATH")]
    input: PathBuf,

    /// Architecture name (baked into `ARCH_FINGERPRINT`).
    #[arg(long = "arch", value_name = "NAME")]
    arch: String,

    /// Output Rust source file.
    #[arg(long = "out", value_name = "PATH")]
    output: PathBuf,

    /// Overwrite the output file if it already exists.
    #[arg(long = "force", default_value_t = false)]
    force: bool,

    /// Override the doc-header timestamp (RFC3339). Hidden flag for
    /// reproducible builds and AC8's stable-rebake comparison.
    #[arg(long = "source-stamp", value_name = "RFC3339", hide = true)]
    source_stamp: Option<String>,
}

fn main() -> ExitCode {
    let args = Args::parse();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // Always emit a single-line stderr description; tests
            // grep for substrings.
            eprintln!("{e}");
            ExitCode::from(e.exit_code())
        }
    }
}

fn run(args: &Args) -> Result<(), BakeError> {
    // AC7: distinguish "no such file" from other I/O errors so the
    // user gets exit 2 + the documented message.
    if !args.input.exists() {
        return Err(BakeError::InputMissing(
            args.input.display().to_string(),
        ));
    }

    if args.output.exists() && !args.force {
        return Err(BakeError::OutputExists(
            args.output.display().to_string(),
        ));
    }

    let bytes = std::fs::read(&args.input)?;
    let basename = basename_of(&args.input);

    let opts = BakeOptions {
        arch: &args.arch,
        input_basename: &basename,
        source_stamp: args.source_stamp.as_deref(),
    };
    let out = bake(&bytes, &opts)?;

    // Write atomically-ish: create+truncate (or open w/ truncate).
    let mut f = std::fs::File::create(&args.output)?;
    f.write_all(out.source.as_bytes())?;
    f.flush()?;
    Ok(())
}

/// Best-effort basename extractor; falls back to the full display if
/// the path has no final component.
fn basename_of(p: &Path) -> String {
    p.file_name().map_or_else(
        || p.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}
