//! transcript — FTS5 index over Claude Code session JSONLs.

use anyhow::Result;
use clap::{Parser, Subcommand};
use session_index::index::Index;
use session_index::{paths, reindex};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "transcript",
    version,
    about = "FTS5 index over ~/.claude/projects/*.jsonl session traces."
)]
struct Cli {
    /// Override the transcript data root (default: `$TRANSCRIPT_HOME` or
    /// `~/.claude/transcript`).
    #[arg(long, global = true)]
    root: Option<PathBuf>,

    /// Override the projects-root walked at reindex time (default:
    /// `$CLAUDE_PROJECTS` or `~/.claude/projects`).
    #[arg(long, global = true)]
    projects: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create the data dir and initialize the `SQLite` index.
    Init,
    /// Drop the index and rebuild from JSONLs on disk.
    Reindex,
    /// Keyword search across all indexed turns. Newest first.
    Query {
        query: String,
        #[arg(long, default_value_t = 10)]
        limit: usize,
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// List indexed sessions, newest first.
    ListSessions {
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Summary counts.
    Stats {
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Print where transcript reads and writes.
    Where,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = match cli.root {
        Some(r) => r,
        None => paths::root()?,
    };
    let projects = match cli.projects {
        Some(p) => p,
        None => paths::projects_root()?,
    };

    match cli.command {
        Command::Init => cmd_init(&root),
        Command::Reindex => cmd_reindex(&root, &projects),
        Command::Query {
            query,
            limit,
            format,
        } => cmd_query(&root, &query, limit, &format),
        Command::ListSessions { limit, format } => cmd_list_sessions(&root, limit, &format),
        Command::Stats { format } => cmd_stats(&root, &format),
        Command::Where => {
            cmd_where(&root, &projects);
            Ok(())
        }
    }
}

fn cmd_init(root: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(paths::index_dir(root))?;
    let _ = Index::open(&paths::index_db(root))?;
    println!("initialized {}", root.display());
    Ok(())
}

fn cmd_reindex(root: &std::path::Path, projects: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(paths::index_dir(root))?;
    let mut idx = Index::open(&paths::index_db(root))?;
    idx.truncate()?;
    let report = reindex::run(&mut idx, projects)?;
    idx.set_state(
        "last_indexed_at",
        &std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs().to_string())
            .unwrap_or_default(),
    )?;
    println!(
        "indexed {} rows from {} files (skipped {} unparseable lines)",
        report.rows, report.files, report.skipped_lines
    );
    Ok(())
}

fn cmd_query(root: &std::path::Path, q: &str, limit: usize, format: &str) -> Result<()> {
    let idx = Index::open(&paths::index_db(root))?;
    let hits = idx.query(q, limit)?;
    if format == "json" {
        println!("{}", serde_json::to_string_pretty(&hits)?);
    } else {
        if hits.is_empty() {
            println!("(no matches)");
        }
        for h in &hits {
            let ts = h.ts.as_deref().unwrap_or("?");
            let tool = h
                .tool_name
                .as_deref()
                .map(|t| format!(" {t}"))
                .unwrap_or_default();
            println!(
                "[{ts}] {role}{tool} {sid}#{turn}\n  {text}",
                role = h.role,
                sid = short_id(&h.session_id),
                turn = h.turn_index,
                text = one_line(&h.text),
            );
        }
    }
    Ok(())
}

fn cmd_list_sessions(root: &std::path::Path, limit: usize, format: &str) -> Result<()> {
    let idx = Index::open(&paths::index_db(root))?;
    let sessions = idx.list_sessions(limit)?;
    if format == "json" {
        println!("{}", serde_json::to_string_pretty(&sessions)?);
    } else {
        for s in &sessions {
            let last = s.last_ts.as_deref().unwrap_or("?");
            println!(
                "{last}  {sid}  rows={n}  project={p}",
                sid = short_id(&s.session_id),
                n = s.row_count,
                p = s.project_dir,
            );
        }
    }
    Ok(())
}

fn cmd_stats(root: &std::path::Path, format: &str) -> Result<()> {
    let idx = Index::open(&paths::index_db(root))?;
    let stats = idx.stats()?;
    if format == "json" {
        println!("{}", serde_json::to_string_pretty(&stats)?);
    } else {
        println!(
            "sessions={s} rows={r} projects={p} schema_version={v}",
            s = stats.session_count,
            r = stats.row_count,
            p = stats.project_count,
            v = stats.schema_version,
        );
    }
    Ok(())
}

fn cmd_where(root: &std::path::Path, projects: &std::path::Path) {
    println!("root     {}", root.display());
    println!("db       {}", paths::index_db(root).display());
    println!("projects {}", projects.display());
}

fn short_id(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}

fn one_line(s: &str) -> String {
    let mut out = String::with_capacity(s.len().min(160));
    for ch in s.chars().take(160) {
        if ch == '\n' || ch == '\r' {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    if s.chars().count() > 160 {
        out.push('…');
    }
    out
}
