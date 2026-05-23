//! skill-telemetry — per-skill invocation log (a.k.a. spool).
//!
//! See PRD-skill-telemetry.md. Phase 0 ships:
//!   - log start/end (append to monthly JSONL bucket)
//!   - rank (skill → invocation count, mean duration, last fired)
//!   - report --stale (skills with no events in window)

#![cfg_attr(not(test), forbid(unsafe_code))]
#![allow(
    clippy::trivially_copy_pass_by_ref,
    clippy::option_if_let_else,
    clippy::single_match_else,
    clippy::type_complexity
)]

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, BufRead, Write as _};
use std::path::{Path, PathBuf};

/// Maximum length (chars) for the `args` field before truncation.
pub const ARGS_MAX_LEN: usize = 200;

/// Tagged enum for one journal line.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "lowercase")]
pub enum SpoolEvent {
    /// Skill invocation start.
    Start(StartEvent),
    /// Skill invocation end.
    End(EndEvent),
}

/// Start-event payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartEvent {
    /// Skill name (e.g. "self-review").
    pub skill: String,
    /// 26-char ULID, lexicographically sortable.
    pub invocation_id: String,
    /// RFC3339 timestamp the invocation started.
    pub started_at: DateTime<Utc>,
    /// Optional truncated args text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<String>,
    /// True iff args was truncated at `ARGS_MAX_LEN`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub args_truncated: bool,
}

/// End-event payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndEvent {
    /// Invocation id paired with a prior `start` event.
    pub invocation_id: String,
    /// RFC3339 timestamp the invocation ended.
    pub ended_at: DateTime<Utc>,
    /// Wall duration ms (None when no matching start was found).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// Caller-reported outcome: "ok" | "error" | "interrupted".
    pub outcome: String,
}

const fn is_false(b: &bool) -> bool {
    !*b
}

/// Append a `start` event. Returns the generated ULID.
///
/// # Errors
/// Returns `io::Error` if the journal directory can't be created or the file can't be written.
pub fn log_start(root: &Path, skill: &str, args: Option<&str>) -> io::Result<String> {
    fs::create_dir_all(root)?;
    let now = Utc::now();
    let id = ulid::Ulid::new().to_string();
    let (truncated_args, was_truncated) = truncate_args(args);
    let event = SpoolEvent::Start(StartEvent {
        skill: skill.to_string(),
        invocation_id: id.clone(),
        started_at: now,
        args: truncated_args,
        args_truncated: was_truncated,
    });
    append_event(root, now, &event)?;
    Ok(id)
}

/// Outcome of `log_end`.
#[derive(Debug)]
pub enum LogEndResult {
    /// Matching start was found; duration computed.
    Paired {
        /// Wall duration in ms from start → end.
        duration_ms: u64,
    },
    /// No matching start in journal; orphaned end was written anyway.
    Orphaned,
}

/// Append an `end` event matched to an invocation id.
///
/// # Errors
/// Returns `io::Error` if the journal can't be read or written.
pub fn log_end(root: &Path, invocation_id: &str, outcome: &str) -> io::Result<LogEndResult> {
    fs::create_dir_all(root)?;
    let now = Utc::now();
    let start = find_start(root, invocation_id)?;
    let duration_ms = start.as_ref().map(|s| {
        let delta = now - s.started_at;
        delta.num_milliseconds().max(0).unsigned_abs()
    });
    let event = SpoolEvent::End(EndEvent {
        invocation_id: invocation_id.to_string(),
        ended_at: now,
        duration_ms,
        outcome: outcome.to_string(),
    });
    append_event(root, now, &event)?;
    Ok(if start.is_some() {
        LogEndResult::Paired { duration_ms: duration_ms.unwrap_or(0) }
    } else {
        LogEndResult::Orphaned
    })
}

fn truncate_args(args: Option<&str>) -> (Option<String>, bool) {
    match args {
        None => (None, false),
        Some(s) => {
            if s.chars().count() <= ARGS_MAX_LEN {
                (Some(s.to_string()), false)
            } else {
                let mut truncated: String = s.chars().take(ARGS_MAX_LEN - 1).collect();
                truncated.push('…');
                (Some(truncated), true)
            }
        }
    }
}

fn append_event(root: &Path, now: DateTime<Utc>, event: &SpoolEvent) -> io::Result<()> {
    let bucket = month_bucket_path(root, now);
    if let Some(parent) = bucket.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut f = fs::OpenOptions::new().create(true).append(true).open(&bucket)?;
    let json = serde_json::to_string(event).map_err(io::Error::other)?;
    writeln!(f, "{json}")?;
    Ok(())
}

fn month_bucket_path(root: &Path, now: DateTime<Utc>) -> PathBuf {
    root.join(format!("{}.jsonl", now.format("%Y-%m")))
}

fn find_start(root: &Path, invocation_id: &str) -> io::Result<Option<StartEvent>> {
    for event in load_events(root)? {
        if let SpoolEvent::Start(s) = event {
            if s.invocation_id == invocation_id {
                return Ok(Some(s));
            }
        }
    }
    Ok(None)
}

/// Load every event from every monthly bucket under `root`.
///
/// Returns an empty vec if `root` doesn't exist (no events yet).
///
/// # Errors
/// Returns `io::Error` if reading the directory or any file fails.
pub fn load_events(root: &Path) -> io::Result<Vec<SpoolEvent>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut events = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(root)?
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
        .collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let f = fs::File::open(entry.path())?;
        let reader = io::BufReader::new(f);
        for line_result in reader.lines() {
            let Ok(line) = line_result else { continue };
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<SpoolEvent>(&line) {
                Ok(event) => events.push(event),
                Err(_) => {
                    // Malformed line — skip silently in this slice; could surface in stderr later.
                }
            }
        }
    }
    Ok(events)
}

/// One row in the `rank` output.
#[derive(Debug, Clone, Serialize)]
pub struct RankEntry {
    /// Skill name.
    pub skill: String,
    /// Total number of start events seen.
    pub invocations: u64,
    /// Mean duration across paired (start, end) pairs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean_ms: Option<u64>,
    /// Most recent start RFC3339, or None if no events.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_fired: Option<DateTime<Utc>>,
}

/// Compute per-skill rank rows from a list of events.
#[must_use]
pub fn rank(events: &[SpoolEvent]) -> Vec<RankEntry> {
    // Map invocation_id → (skill, started_at, optional ended_at, optional duration_ms).
    let mut starts: BTreeMap<String, (String, DateTime<Utc>)> = BTreeMap::new();
    let mut ends: BTreeMap<String, u64> = BTreeMap::new();
    for event in events {
        match event {
            SpoolEvent::Start(s) => {
                starts.insert(s.invocation_id.clone(), (s.skill.clone(), s.started_at));
            }
            SpoolEvent::End(e) => {
                if let Some(d) = e.duration_ms {
                    ends.insert(e.invocation_id.clone(), d);
                }
            }
        }
    }
    let mut by_skill: BTreeMap<String, (u64, u64, u64, Option<DateTime<Utc>>)> = BTreeMap::new();
    // value: (invocations, total_ms_seen, paired_count, last_fired)
    for (id, (skill, started_at)) in &starts {
        let entry = by_skill.entry(skill.clone()).or_insert((0, 0, 0, None));
        entry.0 += 1;
        if let Some(d) = ends.get(id) {
            entry.1 += d;
            entry.2 += 1;
        }
        entry.3 = match entry.3 {
            Some(prev) if prev > *started_at => Some(prev),
            _ => Some(*started_at),
        };
    }
    let mut rows: Vec<RankEntry> = by_skill
        .into_iter()
        .map(|(skill, (invocations, total_ms, paired, last_fired))| RankEntry {
            skill,
            invocations,
            mean_ms: if paired > 0 { Some(total_ms / paired) } else { None },
            last_fired,
        })
        .collect();
    rows.sort_by(|a, b| {
        b.invocations.cmp(&a.invocations).then_with(|| a.skill.cmp(&b.skill))
    });
    rows
}

/// Find skills with no `start` events within the given window.
#[must_use]
pub fn stale_skills(events: &[SpoolEvent], now: DateTime<Utc>, window: Duration) -> Vec<String> {
    let mut last_fired_by_skill: BTreeMap<String, DateTime<Utc>> = BTreeMap::new();
    for event in events {
        if let SpoolEvent::Start(s) = event {
            let entry = last_fired_by_skill.entry(s.skill.clone()).or_insert(s.started_at);
            if *entry < s.started_at {
                *entry = s.started_at;
            }
        }
    }
    let cutoff = now - window;
    let mut stale: Vec<String> = last_fired_by_skill
        .into_iter()
        .filter(|(_, last)| *last < cutoff)
        .map(|(skill, _)| skill)
        .collect();
    stale.sort();
    stale
}

/// Parse a duration like "30d" / "7d" / "12h" / "30m".
#[must_use]
pub fn parse_duration(s: &str) -> Option<Duration> {
    let trimmed = s.trim();
    if trimmed.len() < 2 {
        return None;
    }
    let (num_part, suffix) = trimmed.split_at(trimmed.len() - 1);
    let n: i64 = num_part.parse().ok()?;
    match suffix {
        "d" => Some(Duration::days(n)),
        "h" => Some(Duration::hours(n)),
        "m" => Some(Duration::minutes(n)),
        _ => None,
    }
}
