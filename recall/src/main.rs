//! recall — local-first agentic memory CLI.

use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use recall::index::Index;
use recall::memory::{Kind, Memory, Subject};
use recall::paths;
use recall::retrieval;
use recall::store::FileStore;
use std::io::{self, Read};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "recall",
    version,
    about = "Local-first agentic memory: file-backed memories with a keyword/FTS5 index."
)]
struct Cli {
    /// Override the recall data root (default: `$RECALL_HOME` or `~/.claude/recall`).
    #[arg(long, global = true)]
    root: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create the data dir and initialize the `SQLite` index.
    Init,

    /// Write a new memory. Body is read from --body, --file, or stdin.
    Write {
        /// Memory kind: episodic | semantic | procedural | reflective
        #[arg(long, default_value = "semantic")]
        kind: String,
        /// Subject: user | self | project:<slug> | tool:<name>
        #[arg(long, default_value = "user")]
        subject: String,
        /// Inline body text. If absent, read from --file or stdin.
        #[arg(long)]
        body: Option<String>,
        /// Read body from this file.
        #[arg(long)]
        file: Option<PathBuf>,
        /// Set confidence in [0,1].
        #[arg(long)]
        confidence: Option<f64>,
        /// Mark another memory id as superseded by this one (repeatable).
        #[arg(long)]
        supersedes: Vec<String>,
    },

    /// Search memories by keyword. Prints a ranked JSON array.
    Query {
        /// The query text.
        query: String,
        /// Max results.
        #[arg(long, default_value_t = 5)]
        limit: usize,
        /// Output format: json | text
        #[arg(long, default_value = "text")]
        format: String,
        /// If set, also bump `recall_count` and `last_recalled_at` on each hit.
        #[arg(long)]
        touch: bool,
    },

    /// List memories (newest first). Optionally filter by subject prefix.
    List {
        /// Subject prefix, e.g. `project:` or `user`.
        #[arg(long)]
        subject: Option<String>,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },

    /// Show a single memory's Markdown content by id.
    Show {
        id: String,
    },

    /// Delete a memory by id (removes the file and the index row).
    Delete {
        id: String,
    },

    /// Wipe the `SQLite` index and rebuild it from the files on disk.
    Reindex,

    /// Print where recall is reading from.
    Where,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = match cli.root {
        Some(r) => r,
        None => paths::root()?,
    };

    match cli.command {
        Command::Init => cmd_init(&root),
        Command::Write {
            kind,
            subject,
            body,
            file,
            confidence,
            supersedes,
        } => cmd_write(&root, &kind, &subject, body, file, confidence, supersedes),
        Command::Query {
            query,
            limit,
            format,
            touch,
        } => cmd_query(&root, &query, limit, &format, touch),
        Command::List { subject, limit } => cmd_list(&root, subject.as_deref(), limit),
        Command::Show { id } => cmd_show(&root, &id),
        Command::Delete { id } => cmd_delete(&root, &id),
        Command::Reindex => cmd_reindex(&root),
        Command::Where => {
            println!("{}", root.display());
            Ok(())
        }
    }
}

fn cmd_init(root: &std::path::Path) -> Result<()> {
    let _store = FileStore::open(root.to_path_buf())?;
    let _idx = Index::open(&paths::index_db(root))?;
    println!("initialized {}", root.display());
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_write(
    root: &std::path::Path,
    kind: &str,
    subject: &str,
    body: Option<String>,
    file: Option<PathBuf>,
    confidence: Option<f64>,
    supersedes: Vec<String>,
) -> Result<()> {
    let body_text = read_body(body, file)?;
    if body_text.trim().is_empty() {
        return Err(anyhow!("body is empty"));
    }
    let store = FileStore::open(root.to_path_buf())?;
    let idx = Index::open(&paths::index_db(root))?;
    let kind: Kind = kind.parse()?;
    let mut mem = Memory::new(kind, Subject(subject.to_string()), body_text);
    if let Some(c) = confidence {
        mem.front.confidence = c.clamp(0.0, 1.0);
    }
    mem.front.supersedes = supersedes;
    let path = store.write(&mem)?;
    idx.upsert(&mem, &path)?;
    println!("{}", mem.front.id);
    Ok(())
}

fn read_body(body: Option<String>, file: Option<PathBuf>) -> Result<String> {
    if let Some(b) = body {
        return Ok(b);
    }
    if let Some(p) = file {
        return std::fs::read_to_string(&p)
            .with_context(|| format!("read body file {}", p.display()));
    }
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf).context("read stdin")?;
    Ok(buf)
}

fn cmd_query(
    root: &std::path::Path,
    query: &str,
    limit: usize,
    format: &str,
    touch: bool,
) -> Result<()> {
    let idx = Index::open(&paths::index_db(root))?;
    let hits = retrieval::search(&idx, query, limit)?;
    if touch {
        for r in &hits {
            idx.touch_recall(&r.hit.id)?;
        }
    }
    if format == "json" {
        let arr: Vec<serde_json::Value> = hits
            .iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.hit.id,
                    "kind": r.hit.kind,
                    "subject": r.hit.subject,
                    "path": r.hit.path.to_string_lossy(),
                    "snippet": r.hit.snippet,
                    "score": r.score,
                    "confidence": r.hit.confidence,
                    "recall_count": r.hit.recall_count,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr)?);
    } else {
        if hits.is_empty() {
            println!("(no matches)");
        }
        for r in hits {
            println!(
                "{}  [{}/{}]  score={:.3} recalls={}",
                &r.hit.id, r.hit.kind, r.hit.subject, r.score, r.hit.recall_count
            );
            println!("  {}", r.hit.snippet);
        }
    }
    Ok(())
}

fn cmd_list(
    root: &std::path::Path,
    subject: Option<&str>,
    limit: usize,
) -> Result<()> {
    let idx = Index::open(&paths::index_db(root))?;
    let hits = idx.list(subject, limit)?;
    if hits.is_empty() {
        println!("(no memories)");
    }
    for h in hits {
        println!("{}  [{}/{}]  recalls={}", h.id, h.kind, h.subject, h.recall_count);
    }
    Ok(())
}

fn cmd_show(root: &std::path::Path, id: &str) -> Result<()> {
    let store = FileStore::open(root.to_path_buf())?;
    let (mem, path) = store.find_by_id(id)?;
    println!("# {}", path.display());
    println!("{}", mem.to_markdown()?);
    Ok(())
}

fn cmd_delete(root: &std::path::Path, id: &str) -> Result<()> {
    let store = FileStore::open(root.to_path_buf())?;
    let idx = Index::open(&paths::index_db(root))?;
    let (_, path) = store.find_by_id(id)?;
    store.delete(&path)?;
    idx.remove(id)?;
    println!("deleted {id}");
    Ok(())
}

fn cmd_reindex(root: &std::path::Path) -> Result<()> {
    let store = FileStore::open(root.to_path_buf())?;
    let idx = Index::open(&paths::index_db(root))?;
    let it = store.iter_all().filter_map(|r| match r {
        Ok(v) => Some(v),
        Err(e) => {
            eprintln!("skip: {e:#}");
            None
        }
    });
    let n = idx.rebuild_from(it)?;
    println!("indexed {n} memories");
    Ok(())
}
