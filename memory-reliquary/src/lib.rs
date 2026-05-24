//! memory-reliquary — compose a year's recall memories into one Markdown bundle.
//!
//! This is the Phase 1a composer: read recall memories for a calendar year,
//! filter by subject and privacy, and emit a single Markdown file with
//! top-level frontmatter and one section per memory. The output is the
//! deterministic typesetting input that Phase 1b (Typst) consumes.

#![cfg_attr(not(test), forbid(unsafe_code))]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Errors returned by the composer.
#[derive(Debug, thiserror::Error)]
pub enum ComposeError {
    /// The recall-root path does not exist or is not a directory.
    #[error("recall root not found: {0}")]
    RecallRootNotFound(PathBuf),
    /// Failed to invoke the `recall list` command.
    #[error("failed to invoke recall command {cmd:?}: {source}")]
    RecallSpawn {
        /// The command we tried to spawn.
        cmd: String,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// `recall list` exited non-zero.
    #[error("recall command {cmd:?} failed: exit {code:?} stderr={stderr}")]
    RecallExit {
        /// The command we ran.
        cmd: String,
        /// Exit code, if any.
        code: Option<i32>,
        /// Captured stderr.
        stderr: String,
    },
    /// Failed to parse `recall list --format json` output.
    #[error("failed to parse recall JSON: {0}")]
    RecallJsonParse(#[source] serde_json::Error),
    /// I/O error during file read/write.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// A single recall memory entry from `recall list --format json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallEntry {
    /// ULID identifier.
    pub id: String,
    /// Memory kind (semantic, procedural, feedback, etc.).
    pub kind: String,
    /// Subject prefix (e.g. "user", "self").
    pub subject: String,
    /// On-disk path to the Markdown body file.
    pub path: PathBuf,
    /// How many times this memory has been recalled.
    pub recall_count: u64,
    /// Last recall timestamp (RFC3339), or `None` if never recalled.
    pub last_recalled_at: Option<String>,
}

/// Options that drive a single bundle composition.
#[derive(Debug, Clone)]
pub struct ComposeOptions {
    /// Calendar year to filter memories by (e.g. 2026).
    pub year: i32,
    /// Recall data root (e.g. `~/.claude/recall`).
    pub recall_root: PathBuf,
    /// Subject prefix to filter by (default: `"user"`).
    pub subject: String,
    /// Command used to list recall entries (testing seam).
    pub recall_cmd: PathBuf,
    /// Whether to include the optional `mattered`/`dormancy_days` fields.
    pub include_recall_stats: bool,
    /// Override the `generated_at` timestamp (testing seam). `None` = now.
    pub generated_at: Option<DateTime<Utc>>,
}

/// Per-memory parsed frontmatter we care about.
#[derive(Debug, Clone)]
struct ParsedMemory {
    entry: RecallEntry,
    created_at: DateTime<Utc>,
    body: String,
}

/// Summary of a successful compose run.
#[derive(Debug, Clone, Copy)]
pub struct ComposeStats {
    /// Number of memories emitted in the bundle.
    pub memory_count: u64,
    /// Number of memories suppressed because of `private: true`.
    pub excluded_private_count: u64,
}

/// Subset of frontmatter fields we read from each memory body.
#[derive(Debug, Deserialize)]
struct FrontmatterFields {
    created_at: Option<String>,
    #[serde(default)]
    private: Option<bool>,
}

/// Outcome of parsing a single recall entry.
enum EntryOutcome {
    Keep(ParsedMemory),
    Skip,
    ExcludedPrivate,
}

/// Run a full compose, writing the bundle to `out` and warnings to `err`.
///
/// # Errors
///
/// Returns [`ComposeError::RecallRootNotFound`] when the recall-root is
/// missing or not a directory, or propagates I/O and JSON parse errors.
pub fn compose<W: Write, E: Write>(
    opts: &ComposeOptions,
    out: &mut W,
    err: &mut E,
) -> Result<ComposeStats, ComposeError> {
    // AC5: validate recall-root up front.
    if !opts.recall_root.is_dir() {
        return Err(ComposeError::RecallRootNotFound(opts.recall_root.clone()));
    }

    let entries = list_recall(opts)?;

    let mut kept: Vec<ParsedMemory> = Vec::with_capacity(entries.len());
    let mut excluded_private = 0u64;

    for entry in entries {
        match classify_entry(entry, opts, err)? {
            EntryOutcome::Keep(m) => kept.push(m),
            EntryOutcome::ExcludedPrivate => excluded_private += 1,
            EntryOutcome::Skip => {}
        }
    }

    // AC3: sort by created_at ascending, tiebreak by id.
    kept.sort_by(|a, b| {
        a.created_at
            .cmp(&b.created_at)
            .then_with(|| a.entry.id.cmp(&b.entry.id))
    });

    let mattered_tiers = if opts.include_recall_stats {
        Some(compute_mattered_tiers(&kept))
    } else {
        None
    };

    let generated_at = opts.generated_at.unwrap_or_else(Utc::now);
    let memory_count = u64::try_from(kept.len()).unwrap_or(u64::MAX);

    write_top_frontmatter(out, opts, generated_at, memory_count, excluded_private)?;
    for mem in &kept {
        write_memory_section(out, mem, mattered_tiers)?;
    }

    Ok(ComposeStats {
        memory_count,
        excluded_private_count: excluded_private,
    })
}

/// Apply subject filter, year filter, privacy and frontmatter parsing to
/// one entry. Warnings are written to `err`; the I/O error is propagated
/// only if writing to `err` itself fails.
fn classify_entry<E: Write>(
    entry: RecallEntry,
    opts: &ComposeOptions,
    err: &mut E,
) -> Result<EntryOutcome, ComposeError> {
    if !entry.subject.starts_with(&opts.subject) {
        return Ok(EntryOutcome::Skip);
    }
    let raw = match fs::read_to_string(&entry.path) {
        Ok(s) => s,
        Err(e) => {
            writeln!(
                err,
                "WARN: skipping {}: read error: {e}",
                entry.path.display()
            )?;
            return Ok(EntryOutcome::Skip);
        }
    };
    let Some((fm_text, body)) = split_frontmatter(&raw) else {
        writeln!(
            err,
            "WARN: skipping {}: frontmatter parse error: missing leading --- block",
            entry.path.display()
        )?;
        return Ok(EntryOutcome::Skip);
    };
    let fm: FrontmatterFields = match serde_yaml::from_str(fm_text) {
        Ok(v) => v,
        Err(e) => {
            writeln!(
                err,
                "WARN: skipping {}: frontmatter parse error: {e}",
                entry.path.display()
            )?;
            return Ok(EntryOutcome::Skip);
        }
    };
    if fm.private.unwrap_or(false) {
        return Ok(EntryOutcome::ExcludedPrivate);
    }
    let Some(created_at_raw) = fm.created_at else {
        writeln!(
            err,
            "WARN: skipping {}: frontmatter parse error: missing created_at",
            entry.path.display()
        )?;
        return Ok(EntryOutcome::Skip);
    };
    let created_at = match DateTime::parse_from_rfc3339(&created_at_raw) {
        Ok(dt) => dt.with_timezone(&Utc),
        Err(e) => {
            writeln!(
                err,
                "WARN: skipping {}: frontmatter parse error: invalid created_at {created_at_raw:?}: {e}",
                entry.path.display()
            )?;
            return Ok(EntryOutcome::Skip);
        }
    };
    if created_at.format("%Y").to_string() != format!("{:04}", opts.year) {
        return Ok(EntryOutcome::Skip);
    }
    Ok(EntryOutcome::Keep(ParsedMemory {
        entry,
        created_at,
        body: body.to_owned(),
    }))
}

fn write_top_frontmatter<W: Write>(
    out: &mut W,
    opts: &ComposeOptions,
    generated_at: DateTime<Utc>,
    memory_count: u64,
    excluded_private: u64,
) -> Result<(), ComposeError> {
    writeln!(out, "---")?;
    writeln!(out, "year: {}", opts.year)?;
    writeln!(out, "generated_at: \"{}\"", generated_at.to_rfc3339())?;
    writeln!(out, "memory_count: {memory_count}")?;
    writeln!(out, "excluded_private_count: {excluded_private}")?;
    writeln!(out, "recall_root: {}", opts.recall_root.display())?;
    writeln!(out, "---")?;
    Ok(())
}

fn write_memory_section<W: Write>(
    out: &mut W,
    mem: &ParsedMemory,
    mattered_tiers: Option<(u64, u64)>,
) -> Result<(), ComposeError> {
    writeln!(out)?;
    writeln!(out, "---")?;
    writeln!(out, "id: {}", mem.entry.id)?;
    writeln!(out, "kind: {}", mem.entry.kind)?;
    writeln!(out, "subject: {}", mem.entry.subject)?;
    writeln!(out, "created_at: \"{}\"", mem.created_at.to_rfc3339())?;
    writeln!(out, "recall_count: {}", mem.entry.recall_count)?;
    match &mem.entry.last_recalled_at {
        Some(s) => writeln!(out, "last_recalled_at: \"{s}\"")?,
        None => writeln!(out, "last_recalled_at: null")?,
    }
    if let Some(tiers) = mattered_tiers {
        let mattered = tier_label(tiers, mem.entry.recall_count);
        writeln!(out, "mattered: \"{mattered}\"")?;
        writeln!(out, "dormancy_days: {}", dormancy_days(mem))?;
    }
    writeln!(out, "---")?;
    writeln!(out)?;
    out.write_all(mem.body.as_bytes())?;
    if !mem.body.ends_with('\n') {
        writeln!(out)?;
    }
    Ok(())
}

/// Split a memory body into (frontmatter-text, body-text).
///
/// Returns `None` if the input does not begin with a `---` line followed by
/// a closing `---` line.
fn split_frontmatter(raw: &str) -> Option<(&str, &str)> {
    let after_open = raw
        .strip_prefix("---\n")
        .or_else(|| raw.strip_prefix("---\r\n"))?;
    let mut offset = 0usize;
    for line in after_open.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\n', '\r']);
        if trimmed == "---" {
            let fm = after_open.get(..offset)?;
            let rest = after_open.get(offset.checked_add(line.len())?..)?;
            return Some((fm, rest));
        }
        offset = offset.checked_add(line.len())?;
    }
    None
}

/// Shell out to `recall list --format json --root <root> --subject <subject>`.
fn list_recall(opts: &ComposeOptions) -> Result<Vec<RecallEntry>, ComposeError> {
    let cmd_display = opts.recall_cmd.display().to_string();
    let output = Command::new(&opts.recall_cmd)
        .arg("list")
        .arg("--format")
        .arg("json")
        .arg("--root")
        .arg(&opts.recall_root)
        .arg("--subject")
        .arg(&opts.subject)
        .arg("--limit")
        .arg("100000")
        .output()
        .map_err(|source| ComposeError::RecallSpawn {
            cmd: cmd_display.clone(),
            source,
        })?;
    if !output.status.success() {
        return Err(ComposeError::RecallExit {
            cmd: cmd_display,
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    let entries: Vec<RecallEntry> =
        serde_json::from_slice(&output.stdout).map_err(ComposeError::RecallJsonParse)?;
    Ok(entries)
}

/// AC8: compute tertile cutoffs on `recall_count` across the kept set.
///
/// Returns `(low_high_cutoff, high_low_cutoff)`: counts strictly below
/// `low_high_cutoff` are "low", counts at or above `high_low_cutoff` are
/// "high", the rest are "medium". With `<3` distinct counts we degrade
/// gracefully: identical bounds collapse the tier to "medium".
fn compute_mattered_tiers(memories: &[ParsedMemory]) -> (u64, u64) {
    if memories.is_empty() {
        return (0, 0);
    }
    let mut counts: Vec<u64> = memories.iter().map(|m| m.entry.recall_count).collect();
    counts.sort_unstable();
    let n = counts.len();
    let one_third = n / 3;
    let two_thirds = (n * 2) / 3;
    let low_high = counts.get(one_third).copied().unwrap_or(0);
    let high_low = counts.get(two_thirds).copied().unwrap_or(0);
    (low_high, high_low)
}

const fn tier_label(tiers: (u64, u64), count: u64) -> &'static str {
    let (low_max_exclusive, high_min) = tiers;
    if low_max_exclusive == high_min {
        return "medium";
    }
    if count >= high_min {
        "high"
    } else if count < low_max_exclusive {
        "low"
    } else {
        "medium"
    }
}

fn dormancy_days(mem: &ParsedMemory) -> i64 {
    let Some(last) = mem.entry.last_recalled_at.as_deref() else {
        return 0;
    };
    let Ok(last_dt) = DateTime::parse_from_rfc3339(last) else {
        return 0;
    };
    let delta = last_dt.with_timezone(&Utc) - mem.created_at;
    delta.num_days().max(0)
}

/// Default recall data root: `$RECALL_HOME` or `~/.claude/recall`.
#[must_use]
pub fn default_recall_root() -> PathBuf {
    if let Ok(v) = std::env::var("RECALL_HOME") {
        return PathBuf::from(v);
    }
    if let Ok(home) = std::env::var("HOME") {
        let mut p = PathBuf::from(home);
        p.push(".claude");
        p.push("recall");
        return p;
    }
    PathBuf::from(".")
}

/// Default path to the `recall` binary used as a testing seam.
#[must_use]
pub fn default_recall_cmd() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        let mut p = PathBuf::from(home);
        p.push(".local");
        p.push("bin");
        p.push("recall");
        return p;
    }
    PathBuf::from("recall")
}

/// Resolve a path that may begin with `~/`.
#[must_use]
pub fn expand_tilde(p: &Path) -> PathBuf {
    let Some(s) = p.to_str() else {
        return p.to_path_buf();
    };
    if let Some(rest) = s.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            let mut out = PathBuf::from(home);
            out.push(rest);
            return out;
        }
    }
    p.to_path_buf()
}
