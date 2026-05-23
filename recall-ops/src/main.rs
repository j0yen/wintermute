//! recall-ops — CLI for v0.1 recall maintenance ops.

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
use recall_ops::{gc, lineage, parse_duration, stats, touch, update};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(name = "recall-ops", about = "Maintenance ops for the v0.1 recall store")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Bump recall_count + last_recalled_at for one or more memory ids.
    Touch {
        ids: Vec<String>,
        #[arg(long)]
        root: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Garbage-collect memories older than a threshold.
    Gc {
        #[arg(long)]
        older_than: String,
        #[arg(long, default_value_t = false)]
        never_recalled: bool,
        #[arg(long, default_value_t = false)]
        apply: bool,
        #[arg(long)]
        root: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Print aggregate stats over the recall store.
    Stats {
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
        #[arg(long)]
        root: Option<PathBuf>,
    },
    /// Walk supersedes chains for an id.
    Lineage {
        id: String,
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
        #[arg(long)]
        root: Option<PathBuf>,
    },
    /// Update a memory's body or confidence.
    Update {
        id: String,
        #[arg(long)]
        body: Option<String>,
        #[arg(long)]
        confidence: Option<f64>,
        #[arg(long)]
        root: Option<PathBuf>,
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

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            let _ = e.print();
            return ExitCode::from(2);
        }
    };
    match cli.command {
        Command::Touch { ids, root, format } => {
            run_touch(&ids, root.unwrap_or_else(default_root), format)
        }
        Command::Gc { older_than, never_recalled, apply, root, format } => {
            run_gc(&older_than, never_recalled, apply, root.unwrap_or_else(default_root), format)
        }
        Command::Stats { format, root } => run_stats(root.unwrap_or_else(default_root), format),
        Command::Lineage { id, format, root } => {
            run_lineage(&id, root.unwrap_or_else(default_root), format)
        }
        Command::Update { id, body, confidence, root } => {
            run_update(&id, body.as_deref(), confidence, root.unwrap_or_else(default_root))
        }
    }
}

fn ensure_dir(root: &std::path::Path) -> Option<ExitCode> {
    if root.exists() && !root.is_dir() {
        eprintln!("recall-ops: --root must be a directory: {}", root.display());
        return Some(ExitCode::from(2));
    }
    None
}

fn run_touch(ids: &[String], root: PathBuf, format: Format) -> ExitCode {
    if let Some(c) = ensure_dir(&root) { return c; }
    if ids.is_empty() {
        eprintln!("recall-ops touch: at least one id required");
        return ExitCode::from(2);
    }
    match touch(&root, ids) {
        Ok(report) => {
            match format {
                Format::Json => {
                    if let Ok(s) = serde_json::to_string(&report) {
                        println!("{s}");
                    }
                }
                Format::Text => {
                    for r in &report.results {
                        println!("touched {} → count={} at {}", r.id, r.new_count, r.last_recalled_at);
                    }
                    for u in &report.unknown {
                        println!("unknown id: {u}");
                    }
                }
            }
            if !report.unknown.is_empty() && report.touched == 0 {
                ExitCode::from(1)
            } else {
                ExitCode::from(0)
            }
        }
        Err(e) => {
            eprintln!("recall-ops touch: {e}");
            ExitCode::from(2)
        }
    }
}

fn run_gc(older_than: &str, never_recalled: bool, apply: bool, root: PathBuf, format: Format) -> ExitCode {
    if let Some(c) = ensure_dir(&root) { return c; }
    let dur = match parse_duration(older_than) {
        Some(d) => d,
        None => {
            eprintln!("recall-ops gc: bad --older-than: {older_than}");
            return ExitCode::from(2);
        }
    };
    match gc(&root, dur, never_recalled, apply) {
        Ok(report) => {
            match format {
                Format::Json => {
                    if let Ok(s) = serde_json::to_string(&report) {
                        println!("{s}");
                    }
                }
                Format::Text => {
                    if report.applied {
                        println!("deleted {} memories", report.deleted.len());
                    } else {
                        println!("dry-run: {} candidates", report.count);
                        for c in &report.candidates {
                            println!("  {} (last_seen={})", c.id, c.last_seen);
                        }
                    }
                }
            }
            if report.count == 0 { ExitCode::from(1) } else { ExitCode::from(0) }
        }
        Err(e) => {
            eprintln!("recall-ops gc: {e}");
            ExitCode::from(2)
        }
    }
}

fn run_stats(root: PathBuf, format: Format) -> ExitCode {
    if let Some(c) = ensure_dir(&root) { return c; }
    match stats(&root) {
        Ok(report) => {
            match format {
                Format::Json => {
                    if let Ok(s) = serde_json::to_string(&report) {
                        println!("{s}");
                    }
                }
                Format::Text => {
                    println!("total: {}", report.total);
                    for (k, v) in &report.by_kind { println!("  kind={k}: {v}"); }
                    for (s, v) in &report.by_subject { println!("  subject={s}: {v}"); }
                    if let Some(o) = &report.oldest_created { println!("oldest_created: {o}"); }
                    println!("embedder_ids: [{}]", report.distinct_embedder_ids.join(", "));
                }
            }
            ExitCode::from(0)
        }
        Err(e) => {
            eprintln!("recall-ops stats: {e}");
            ExitCode::from(2)
        }
    }
}

fn run_lineage(id: &str, root: PathBuf, format: Format) -> ExitCode {
    if let Some(c) = ensure_dir(&root) { return c; }
    match lineage(&root, id) {
        Ok(report) => {
            match format {
                Format::Json => {
                    if let Ok(s) = serde_json::to_string(&report) {
                        println!("{s}");
                    }
                }
                Format::Text => {
                    println!("root: {}", report.root_id);
                    println!("ancestors ({}):", report.ancestors.len());
                    for a in &report.ancestors {
                        println!("  {} [{}] {}", a.id, a.kind, a.subject);
                    }
                    println!("descendants ({}):", report.descendants.len());
                    for d in &report.descendants {
                        println!("  {} [{}] {}", d.id, d.kind, d.subject);
                    }
                }
            }
            ExitCode::from(0)
        }
        Err(e) => {
            eprintln!("recall-ops lineage: {e}");
            ExitCode::from(2)
        }
    }
}

fn run_update(id: &str, body: Option<&str>, confidence: Option<f64>, root: PathBuf) -> ExitCode {
    if let Some(c) = ensure_dir(&root) { return c; }
    if body.is_none() && confidence.is_none() {
        eprintln!("recall-ops update: at least one of --body or --confidence required");
        return ExitCode::from(2);
    }
    match update(&root, id, body, confidence) {
        Ok(true) => ExitCode::from(0),
        Ok(false) => {
            eprintln!("recall-ops update: unknown id: {id}");
            ExitCode::from(1)
        }
        Err(e) => {
            eprintln!("recall-ops update: {e}");
            ExitCode::from(2)
        }
    }
}
