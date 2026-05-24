//! tide-chart — instrument-like ASCII view of a laptop-day's rhythm.
//!
//! Phase 0 surface (see `agent/intent-card.json`):
//!   * `collect` reads ctrace-style ndjson event files under `<root>/events/`,
//!     buckets events into UTC hours, computes four per-bucket scalars
//!     (focus, fragmentation, velocity, surprise), z-scores them against a
//!     rolling 30-day baseline drawn from earlier buckets, and persists the
//!     resulting rows to a SQLite database at `<root>/tide.db`.
//!   * `chart` reads `<root>/tide.db` and renders the signals as ASCII strips
//!     (24h, 7-day, or 30-day views).

#![cfg_attr(not(test), forbid(unsafe_code))]
#![allow(
    clippy::float_arithmetic,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions,
    clippy::doc_markdown,
    clippy::module_name_repetitions,
    clippy::similar_names,
    clippy::implicit_hasher
)]

use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Timelike, Utc};
use rusqlite::{Connection, params};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Project-wide error type.
#[derive(Debug, thiserror::Error)]
pub enum TideError {
    /// I/O failure (filesystem, journal reads, etc.).
    #[error("io: {0}")]
    Io(#[from] io::Error),
    /// SQLite failure.
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// ndjson parse failure (line content kept for context).
    #[error("ndjson parse: {0}")]
    Json(#[from] serde_json::Error),
    /// Caller asked for `chart` but no database exists yet.
    #[error("no tide.db at {path:?} — run collect first")]
    NoDatabase {
        /// The path that was probed.
        path: PathBuf,
    },
}

/// Convenient alias.
pub type Result<T> = std::result::Result<T, TideError>;

/// One ctrace-style event, as written by upstream `ctrace`.
///
/// Only `ts` and `kind` are required for tide-chart's signal computations;
/// additional fields are ignored.
#[derive(Debug, Clone, Deserialize)]
pub struct Event {
    /// RFC3339 timestamp (UTC).
    pub ts: DateTime<Utc>,
    /// Event kind (e.g. "syscall", "exec", "open").
    pub kind: String,
}

/// One persisted row: signals for a single hour bucket.
#[derive(Debug, Clone)]
pub struct SignalRow {
    /// Bucket start as RFC3339 (UTC, hour-aligned).
    pub bucket_iso: String,
    /// Focus: longest run-length of same `kind` events / total events.
    pub focus: f64,
    /// Fragmentation: unique kinds / total events.
    pub fragmentation: f64,
    /// Velocity: events per minute.
    pub velocity: f64,
    /// Surprise: rarity of seen kinds vs the 30-day baseline distribution.
    pub surprise: f64,
}

/// Raw (un-z-scored) signal tuple for one bucket.
#[derive(Debug, Clone, Copy, Default)]
pub struct RawSignals {
    /// See [`SignalRow::focus`].
    pub focus: f64,
    /// See [`SignalRow::fragmentation`].
    pub fragmentation: f64,
    /// See [`SignalRow::velocity`].
    pub velocity: f64,
    /// See [`SignalRow::surprise`].
    pub surprise: f64,
}

// ---------------------------------------------------------------------------
// Reading events
// ---------------------------------------------------------------------------

/// Walk `<root>/events/` and parse every `*.ndjson` file into a flat `Vec<Event>`.
///
/// Returns an empty vec if the events directory is missing.
///
/// # Errors
/// Returns `TideError::Io` on directory/file read failures. Malformed lines are
/// skipped silently (mirrors skill-telemetry's load_events policy).
pub fn read_events(root: &Path) -> Result<Vec<Event>> {
    let events_dir = root.join("events");
    if !events_dir.exists() {
        return Ok(Vec::new());
    }
    let mut events: Vec<Event> = Vec::new();
    for entry in WalkDir::new(&events_dir).into_iter().filter_map(std::result::Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "ndjson") {
            continue;
        }
        let f = fs::File::open(path)?;
        let reader = io::BufReader::new(f);
        for line_result in reader.lines() {
            let Ok(line) = line_result else { continue };
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(ev) = serde_json::from_str::<Event>(trimmed) {
                events.push(ev);
            }
        }
    }
    events.sort_by_key(|e| e.ts);
    Ok(events)
}

// ---------------------------------------------------------------------------
// Bucketing
// ---------------------------------------------------------------------------

/// Truncate a timestamp to its hour-of-day (UTC).
#[must_use]
pub fn hour_bucket(ts: DateTime<Utc>) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(ts.year(), ts.month(), ts.day(), ts.hour(), 0, 0)
        .single()
        .unwrap_or(ts)
}

/// Group events by hour bucket. Order: ascending by bucket time.
#[must_use]
pub fn bucket_by_hour(events: &[Event]) -> Vec<(DateTime<Utc>, Vec<Event>)> {
    let mut map: BTreeMap<DateTime<Utc>, Vec<Event>> = BTreeMap::new();
    for ev in events {
        let bucket = hour_bucket(ev.ts);
        map.entry(bucket).or_default().push(ev.clone());
    }
    map.into_iter().collect()
}

// ---------------------------------------------------------------------------
// Raw signal computation
// ---------------------------------------------------------------------------

/// Compute the four raw scalars for a single bucket's events.
#[must_use]
pub fn raw_signals_for_bucket(
    events: &[Event],
    baseline_kind_freq: &HashMap<String, f64>,
) -> RawSignals {
    let total = events.len();
    if total == 0 {
        return RawSignals::default();
    }
    let total_f = total as f64;

    // focus: longest run-length of same kind / total.
    let mut longest_run: usize = 1;
    let mut current_run: usize = 1;
    for window in events.windows(2) {
        if let [a, b] = window {
            if a.kind == b.kind {
                current_run += 1;
                if current_run > longest_run {
                    longest_run = current_run;
                }
            } else {
                current_run = 1;
            }
        }
    }
    let focus = longest_run as f64 / total_f;

    // fragmentation: unique kinds / total.
    let mut kinds_seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for ev in events {
        kinds_seen.insert(ev.kind.as_str());
    }
    let fragmentation = kinds_seen.len() as f64 / total_f;

    // velocity: events per minute = total / 60.
    let velocity = total_f / 60.0;

    // surprise: mean(-log(p(kind))) over events, where p(kind) comes from baseline.
    // If baseline is empty OR kind unseen, use a smoothing floor of 1/(total_baseline+1).
    let baseline_total: f64 = baseline_kind_freq.values().sum();
    let denom = baseline_total + 1.0;
    let mut acc = 0.0;
    for ev in events {
        let raw = baseline_kind_freq.get(&ev.kind).copied().unwrap_or(0.0);
        let p = (raw + 1.0) / denom; // add-one smoothing
        acc += -p.ln();
    }
    let surprise = acc / total_f;

    RawSignals { focus, fragmentation, velocity, surprise }
}

// ---------------------------------------------------------------------------
// Z-scoring
// ---------------------------------------------------------------------------

/// Z-score `x` against the population `baseline`. Returns 0.0 if baseline is
/// empty or has zero variance — by design, since calibration during a cold
/// start should not blow up to ±inf.
#[must_use]
pub fn z_score(x: f64, baseline: &[f64]) -> f64 {
    if baseline.is_empty() {
        return 0.0;
    }
    let n = baseline.len() as f64;
    let mean: f64 = baseline.iter().sum::<f64>() / n;
    let var: f64 = baseline.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    let sd = var.sqrt();
    if sd == 0.0 {
        return 0.0;
    }
    (x - mean) / sd
}

// ---------------------------------------------------------------------------
// Collection pipeline
// ---------------------------------------------------------------------------

/// End-to-end `collect` pipeline.
///
/// Reads events, buckets them, computes raw signals, z-scores against baseline
/// drawn from buckets older than the most-recent-24h window, persists to
/// `<root>/tide.db` (table `signals`). Idempotent via `INSERT OR REPLACE`.
///
/// `now` is plumbed for deterministic testing.
///
/// # Errors
/// Returns `TideError::Io` / `TideError::Sqlite` on IO or DB failures.
pub fn collect(root: &Path, now: DateTime<Utc>) -> Result<usize> {
    fs::create_dir_all(root)?;
    let events = read_events(root)?;
    let buckets = bucket_by_hour(&events);

    // 24h cutoff: any bucket whose hour-start is within [now-24h, now] is in the
    // "current" window. Earlier buckets feed the baseline distribution.
    let now_hour = hour_bucket(now);
    let cutoff_24h = now_hour - Duration::hours(24);
    let cutoff_30d = now_hour - Duration::days(30);

    // Baseline event-kind frequency (count over events, 30d window before now).
    let mut baseline_kind_freq: HashMap<String, f64> = HashMap::new();
    for ev in &events {
        let bh = hour_bucket(ev.ts);
        if bh >= cutoff_30d && bh < cutoff_24h {
            *baseline_kind_freq.entry(ev.kind.clone()).or_insert(0.0) += 1.0;
        }
    }

    // Per-bucket raw signals for baseline buckets (used to z-score current 24h).
    let mut baseline_focus: Vec<f64> = Vec::new();
    let mut baseline_frag: Vec<f64> = Vec::new();
    let mut baseline_vel: Vec<f64> = Vec::new();
    let mut baseline_surp: Vec<f64> = Vec::new();
    let mut current_rows: Vec<(DateTime<Utc>, RawSignals)> = Vec::new();

    for (bh, evs) in &buckets {
        let raw = raw_signals_for_bucket(evs, &baseline_kind_freq);
        if *bh >= cutoff_30d && *bh < cutoff_24h {
            baseline_focus.push(raw.focus);
            baseline_frag.push(raw.fragmentation);
            baseline_vel.push(raw.velocity);
            baseline_surp.push(raw.surprise);
        } else if *bh >= cutoff_24h && *bh <= now_hour {
            current_rows.push((*bh, raw));
        }
    }

    // Open / migrate db.
    let db_path = root.join("tide.db");
    let store = Store::open(&db_path)?;
    store.migrate()?;

    let mut written = 0usize;
    for (bh, raw) in &current_rows {
        let row = SignalRow {
            bucket_iso: bh.to_rfc3339(),
            focus: z_score(raw.focus, &baseline_focus),
            fragmentation: z_score(raw.fragmentation, &baseline_frag),
            velocity: z_score(raw.velocity, &baseline_vel),
            surprise: z_score(raw.surprise, &baseline_surp),
        };
        store.upsert(&row)?;
        written += 1;
    }
    Ok(written)
}

// ---------------------------------------------------------------------------
// SQLite store
// ---------------------------------------------------------------------------

/// Thin wrapper over rusqlite::Connection for the `signals` table.
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Open (or create) the database at `path`.
    ///
    /// # Errors
    /// Returns `TideError::Sqlite` if the file can't be opened.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        Ok(Self { conn })
    }

    /// Ensure the `signals` table exists.
    ///
    /// # Errors
    /// Returns `TideError::Sqlite` on DDL failures.
    pub fn migrate(&self) -> Result<()> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS signals (
                bucket_iso     TEXT PRIMARY KEY,
                focus          REAL NOT NULL,
                fragmentation  REAL NOT NULL,
                velocity       REAL NOT NULL,
                surprise       REAL NOT NULL
            )",
            [],
        )?;
        Ok(())
    }

    /// Insert or replace a row keyed on bucket_iso.
    ///
    /// # Errors
    /// Returns `TideError::Sqlite` on insert failures.
    pub fn upsert(&self, row: &SignalRow) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO signals (bucket_iso, focus, fragmentation, velocity, surprise)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                row.bucket_iso,
                row.focus,
                row.fragmentation,
                row.velocity,
                row.surprise,
            ],
        )?;
        Ok(())
    }

    /// Fetch all rows ordered by `bucket_iso` ascending.
    ///
    /// # Errors
    /// Returns `TideError::Sqlite` on query failures.
    pub fn all_rows(&self) -> Result<Vec<SignalRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT bucket_iso, focus, fragmentation, velocity, surprise
             FROM signals ORDER BY bucket_iso ASC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(SignalRow {
                bucket_iso: r.get::<_, String>(0)?,
                focus: r.get::<_, f64>(1)?,
                fragmentation: r.get::<_, f64>(2)?,
                velocity: r.get::<_, f64>(3)?,
                surprise: r.get::<_, f64>(4)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

const BLOCKS: [char; 8] = ['\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}', '\u{2588}'];

/// Map a normalized value in [0,1] to one of the eight block characters.
#[must_use]
pub fn intensity_char(norm: f64) -> char {
    let clamped = if norm.is_nan() {
        0.0
    } else {
        norm.clamp(0.0, 1.0)
    };
    let idx = (clamped * 7.999).floor() as usize;
    let idx = idx.min(7);
    BLOCKS.get(idx).copied().unwrap_or(BLOCKS[0])
}

/// Normalize a slice of values to `[0,1]` via min-max. Returns a vec of the
/// same length. If all values equal, returns 0.5 for every entry.
#[must_use]
pub fn normalize(xs: &[f64]) -> Vec<f64> {
    if xs.is_empty() {
        return Vec::new();
    }
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for &x in xs {
        if x.is_finite() {
            if x < lo {
                lo = x;
            }
            if x > hi {
                hi = x;
            }
        }
    }
    if !lo.is_finite() || !hi.is_finite() || (hi - lo).abs() < f64::EPSILON {
        return xs.iter().map(|_| 0.5).collect();
    }
    xs.iter()
        .map(|&x| if x.is_finite() { (x - lo) / (hi - lo) } else { 0.0 })
        .collect()
}

/// Per-signal short labels (≤8 chars), left-padded to 8 in the renderer.
pub const SIGNAL_LABELS: [&str; 4] = ["focus", "frag", "velocity", "surprise"];

/// Render the 24-column `--today` view from rows.
///
/// Bucket each row by its UTC hour (0..24), keep the most recent value per
/// hour-of-day from the given rows (assumed already sorted ascending), then
/// per-row min-max normalize and emit `▁..█` cells.
#[must_use]
pub fn render_today(rows: &[SignalRow], now: DateTime<Utc>) -> String {
    let now_hour = hour_bucket(now);
    let cutoff = now_hour - Duration::hours(23);
    // Build 24 hour-slots: [cutoff, cutoff+1h, ..., cutoff+23h == now_hour].
    let mut hours: Vec<DateTime<Utc>> = (0..24).map(|i| cutoff + Duration::hours(i)).collect();
    hours.sort();

    // Map bucket_iso → row.
    let mut by_iso: HashMap<String, &SignalRow> = HashMap::new();
    for r in rows {
        by_iso.insert(r.bucket_iso.clone(), r);
    }

    let mut focus_v = vec![f64::NAN; 24];
    let mut frag_v = vec![f64::NAN; 24];
    let mut vel_v = vec![f64::NAN; 24];
    let mut surp_v = vec![f64::NAN; 24];

    for (i, h) in hours.iter().enumerate() {
        let iso = h.to_rfc3339();
        if let Some(r) = by_iso.get(&iso) {
            if let Some(slot) = focus_v.get_mut(i) {
                *slot = r.focus;
            }
            if let Some(slot) = frag_v.get_mut(i) {
                *slot = r.fragmentation;
            }
            if let Some(slot) = vel_v.get_mut(i) {
                *slot = r.velocity;
            }
            if let Some(slot) = surp_v.get_mut(i) {
                *slot = r.surprise;
            }
        }
    }

    let rows_data = [focus_v, frag_v, vel_v, surp_v];
    let mut out = String::new();
    for (lbl, data) in SIGNAL_LABELS.iter().zip(rows_data.iter()) {
        let normed = normalize(data);
        out.push_str(&format!("{lbl:>8} "));
        for &n in &normed {
            out.push(intensity_char(n));
        }
        out.push('\n');
    }
    out
}

/// Render the 7-cell-per-row `--7day` view from rows.
#[must_use]
pub fn render_7day(rows: &[SignalRow], now: DateTime<Utc>) -> String {
    let today = now.date_naive();
    let days: Vec<NaiveDate> = (0..7)
        .rev()
        .filter_map(|i| today.checked_sub_signed(Duration::days(i)))
        .collect();

    // Group rows by date, computing daily means per signal.
    let mut by_date: BTreeMap<NaiveDate, (f64, f64, f64, f64, usize)> = BTreeMap::new();
    for r in rows {
        if let Ok(dt) = DateTime::parse_from_rfc3339(&r.bucket_iso) {
            let date = dt.naive_utc().date();
            let entry = by_date.entry(date).or_insert((0.0, 0.0, 0.0, 0.0, 0));
            entry.0 += r.focus;
            entry.1 += r.fragmentation;
            entry.2 += r.velocity;
            entry.3 += r.surprise;
            entry.4 += 1;
        }
    }

    let mut focus_v = vec![f64::NAN; days.len()];
    let mut frag_v = vec![f64::NAN; days.len()];
    let mut vel_v = vec![f64::NAN; days.len()];
    let mut surp_v = vec![f64::NAN; days.len()];

    for (i, d) in days.iter().enumerate() {
        if let Some(entry) = by_date.get(d) {
            let n = entry.4 as f64;
            if n > 0.0 {
                if let Some(slot) = focus_v.get_mut(i) {
                    *slot = entry.0 / n;
                }
                if let Some(slot) = frag_v.get_mut(i) {
                    *slot = entry.1 / n;
                }
                if let Some(slot) = vel_v.get_mut(i) {
                    *slot = entry.2 / n;
                }
                if let Some(slot) = surp_v.get_mut(i) {
                    *slot = entry.3 / n;
                }
            }
        }
    }

    let rows_data = [focus_v, frag_v, vel_v, surp_v];
    let mut out = String::new();
    for (lbl, data) in SIGNAL_LABELS.iter().zip(rows_data.iter()) {
        let normed = normalize(data);
        out.push_str(&format!("{lbl:>8} "));
        for &n in &normed {
            out.push(intensity_char(n));
        }
        out.push('\n');
    }
    out
}

/// Render the single-line 30-cell `--30day` heat strip.
#[must_use]
pub fn render_30day(rows: &[SignalRow], now: DateTime<Utc>) -> String {
    let today = now.date_naive();
    let days: Vec<NaiveDate> = (0..30)
        .rev()
        .filter_map(|i| today.checked_sub_signed(Duration::days(i)))
        .collect();

    // Daily aggregate intensity = mean of |focus|+|frag|+|vel|+|surp| over hours.
    let mut by_date: BTreeMap<NaiveDate, (f64, usize)> = BTreeMap::new();
    for r in rows {
        if let Ok(dt) = DateTime::parse_from_rfc3339(&r.bucket_iso) {
            let date = dt.naive_utc().date();
            let agg = r.focus.abs() + r.fragmentation.abs() + r.velocity.abs() + r.surprise.abs();
            let entry = by_date.entry(date).or_insert((0.0, 0));
            entry.0 += agg;
            entry.1 += 1;
        }
    }

    let mut data = vec![f64::NAN; days.len()];
    for (i, d) in days.iter().enumerate() {
        if let Some(entry) = by_date.get(d) {
            let n = entry.1 as f64;
            if n > 0.0 {
                if let Some(slot) = data.get_mut(i) {
                    *slot = entry.0 / n;
                }
            }
        }
    }
    let normed = normalize(&data);
    let mut out = String::new();
    for &n in &normed {
        out.push(intensity_char(n));
    }
    out.push('\n');
    out
}

/// Resolve the default root: `$XDG_DATA_HOME/tide-chart`, fallback
/// `~/.local/share/tide-chart`.
#[must_use]
pub fn default_root() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("tide-chart");
        }
    }
    if let Some(home) = dirs::home_dir() {
        return home.join(".local").join("share").join("tide-chart");
    }
    PathBuf::from(".").join("tide-chart")
}
