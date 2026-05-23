//! skill — manifest validator CLI.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(name = "skill", about = "Skill manifest validator")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Validate a SKILL.md (by path or bare skill name)
    Validate {
        /// Path to SKILL.md, or a bare skill name resolved against ~/.claude/skills/<name>/SKILL.md.
        input: String,

        /// Output format
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Text,
    Json,
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
        Command::Validate { input, format } => run_validate(&input, format),
    }
}

fn run_validate(input: &str, format: Format) -> ExitCode {
    let path = resolve_input(input);
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(err) => {
            if format == Format::Json {
                emit_invocation_error_json(&path, &err.to_string());
            } else {
                eprintln!("skill: cannot read {}: {}", path.display(), err);
            }
            return ExitCode::from(2);
        }
    };

    let result = skill_manifest::validate(&content);
    emit_result(&result, format);

    match result.verdict {
        skill_manifest::Verdict::Pass => ExitCode::from(0),
        skill_manifest::Verdict::Fail => ExitCode::from(1),
        skill_manifest::Verdict::Error => ExitCode::from(2),
    }
}

/// Resolve a CLI input to a SKILL.md path.
fn resolve_input(input: &str) -> PathBuf {
    if input.contains('/') || input.ends_with(".md") {
        PathBuf::from(input)
    } else {
        let home = std::env::var("HOME").unwrap_or_default();
        PathBuf::from(format!("{home}/.claude/skills/{input}/SKILL.md"))
    }
}

fn emit_result(result: &skill_manifest::ValidationOutput, format: Format) {
    match format {
        Format::Json => {
            if let Ok(s) = serde_json::to_string(result) {
                println!("{s}");
            }
        }
        Format::Text => match result.verdict {
            skill_manifest::Verdict::Pass => {
                if result.manifest_present {
                    println!("ok: manifest validates");
                } else {
                    println!("ok: no manifest block (backwards-compatible)");
                }
            }
            skill_manifest::Verdict::Fail => {
                println!("fail:");
                for e in &result.errors {
                    println!("  {} at {}: {}", e.id, e.path, e.message);
                }
            }
            skill_manifest::Verdict::Error => {
                println!("error");
            }
        },
    }
}

fn emit_invocation_error_json(path: &std::path::Path, message: &str) {
    let payload = serde_json::json!({
        "verdict": "error",
        "errors": [{
            "id": "invocation_error",
            "path": path.display().to_string(),
            "message": message,
        }]
    });
    if let Ok(s) = serde_json::to_string(&payload) {
        println!("{s}");
    }
}
