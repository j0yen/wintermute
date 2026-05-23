//! apipe — agent-pipe runtime CLI.

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::float_arithmetic,
    clippy::indexing_slicing,
    clippy::doc_markdown,
    clippy::redundant_closure_for_method_calls
)]

use agent_pipe::{LineOutcome, ParsedLine, get_field, parse_and_validate, schema_for_kind, KNOWN_KINDS};
use clap::{Parser, Subcommand};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::io::{BufRead, Write};
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(name = "apipe", about = "Agent-pipe runtime: streaming NDJSON record pipeline")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Identity filter; validates each line, drops malformed records.
    Pass,
    /// Keep top N records by score (or --by <field>).
    Top {
        /// How many records to keep.
        n: usize,
        /// Field name to rank by (default: score).
        #[arg(long, default_value = "score")]
        by: String,
    },
    /// Sort all records by a field (loads into memory).
    Sort {
        /// Field name to sort by.
        #[arg(short = 'k', long)]
        key: String,
        /// Sort descending.
        #[arg(long, default_value_t = false)]
        desc: bool,
    },
    /// Filter records by a simple predicate `<field> <op> <literal>`.
    Filter {
        /// Predicate expression, e.g. 'score > 0.8' or 'kind == "recall_hit"'.
        expr: String,
    },
    /// Print known record kinds, or the JSON Schema for a specific kind.
    Schema {
        /// Print the JSON Schema for this kind.
        #[arg(long)]
        kind: Option<String>,
    },
    /// Render records as a human-readable table (sink stage).
    Pretty,
    /// Extract path-like fields and emit as bare strings.
    ToPaths,
    /// Inverse of to-paths: wrap bare path strings as file_event records.
    FromPaths,
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
        Command::Pass => run_pass(),
        Command::Top { n, by } => run_top(n, &by),
        Command::Sort { key, desc } => run_sort(&key, desc),
        Command::Filter { expr } => run_filter(&expr),
        Command::Schema { kind } => run_schema(kind.as_deref()),
        Command::Pretty => run_pretty(),
        Command::ToPaths => run_to_paths(),
        Command::FromPaths => run_from_paths(),
    }
}

fn read_lines() -> impl Iterator<Item = (usize, std::io::Result<String>)> {
    std::io::stdin().lock().lines().enumerate().map(|(i, r)| (i + 1, r))
}

fn run_pass() -> ExitCode {
    let mut stdout = std::io::stdout().lock();
    let mut emitted: u64 = 0;
    for (line_no, read_result) in read_lines() {
        let Ok(line) = read_result else { continue };
        if line.is_empty() {
            continue;
        }
        match parse_and_validate(&line) {
            LineOutcome::Ok(p) => {
                let _ = writeln!(stdout, "{}", p.original);
                emitted += 1;
            }
            LineOutcome::Malformed { line: orig, error } => {
                eprintln!("apipe: line {line_no} dropped (malformed): {error}");
                let _ = writeln!(std::io::stderr(), "  {orig}");
            }
            LineOutcome::MissingField { line: orig, field } => {
                eprintln!("apipe: line {line_no} dropped (missing required field: {field})");
                let _ = writeln!(std::io::stderr(), "  {orig}");
            }
        }
    }
    if emitted == 0 {
        ExitCode::from(1)
    } else {
        ExitCode::from(0)
    }
}

fn collect_parsed() -> (Vec<ParsedLine>, bool) {
    let mut records = Vec::new();
    let mut any_input = false;
    for (line_no, read_result) in read_lines() {
        let Ok(line) = read_result else { continue };
        if line.is_empty() {
            continue;
        }
        any_input = true;
        match parse_and_validate(&line) {
            LineOutcome::Ok(p) => records.push(p),
            LineOutcome::Malformed { line: orig, error } => {
                eprintln!("apipe: line {line_no} dropped (malformed): {error}\n  {orig}");
            }
            LineOutcome::MissingField { line: orig, field } => {
                eprintln!("apipe: line {line_no} dropped (missing required field: {field})\n  {orig}");
            }
        }
    }
    (records, any_input)
}

fn run_top(n: usize, by: &str) -> ExitCode {
    let (records, any_input) = collect_parsed();
    if !any_input {
        return ExitCode::from(0);
    }

    // Stable ordering: keep input order for ties by tagging with original index.
    let mut indexed: Vec<(usize, ParsedLine)> = records.into_iter().enumerate().collect();
    indexed.sort_by(|(ia, a), (ib, b)| {
        let key_a = get_field(&a.value, by).and_then(Value::as_f64).unwrap_or(f64::NEG_INFINITY);
        let key_b = get_field(&b.value, by).and_then(Value::as_f64).unwrap_or(f64::NEG_INFINITY);
        // Descending order, then input order for ties.
        key_b.partial_cmp(&key_a).unwrap_or(Ordering::Equal).then(ia.cmp(ib))
    });

    let mut stdout = std::io::stdout().lock();
    for (_, p) in indexed.into_iter().take(n) {
        let _ = writeln!(stdout, "{}", p.original);
    }
    ExitCode::from(0)
}

fn run_sort(key: &str, desc: bool) -> ExitCode {
    let (records, any_input) = collect_parsed();
    if !any_input {
        return ExitCode::from(0);
    }

    // Try numeric sort first; fall back to lexicographic if any value isn't numeric.
    let all_numeric = records.iter().all(|p| {
        get_field(&p.value, key).is_some_and(|v| v.as_f64().is_some())
    });

    let mut indexed: Vec<(usize, ParsedLine)> = records.into_iter().enumerate().collect();
    indexed.sort_by(|(ia, a), (ib, b)| {
        let cmp = if all_numeric {
            let ka = get_field(&a.value, key).and_then(Value::as_f64).unwrap_or(0.0);
            let kb = get_field(&b.value, key).and_then(Value::as_f64).unwrap_or(0.0);
            ka.partial_cmp(&kb).unwrap_or(Ordering::Equal)
        } else {
            let ka = field_string(&a.value, key);
            let kb = field_string(&b.value, key);
            ka.cmp(&kb)
        };
        // Stable on ties via input index.
        if desc { cmp.reverse() } else { cmp }.then(ia.cmp(ib))
    });

    let mut stdout = std::io::stdout().lock();
    for (_, p) in indexed {
        let _ = writeln!(stdout, "{}", p.original);
    }
    ExitCode::from(0)
}

fn field_string(record: &Value, key: &str) -> String {
    get_field(record, key)
        .map(|v| match v {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        })
        .unwrap_or_default()
}

fn run_filter(expr: &str) -> ExitCode {
    let predicate = match parse_predicate(expr) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("apipe filter: bad expression: {e}");
            return ExitCode::from(2);
        }
    };

    let mut stdout = std::io::stdout().lock();
    let mut emitted = false;
    for (line_no, read_result) in read_lines() {
        let Ok(line) = read_result else { continue };
        if line.is_empty() {
            continue;
        }
        match parse_and_validate(&line) {
            LineOutcome::Ok(p) => {
                if predicate.eval(&p.value) {
                    let _ = writeln!(stdout, "{}", p.original);
                    emitted = true;
                }
            }
            LineOutcome::Malformed { line: orig, error } => {
                eprintln!("apipe: line {line_no} dropped: {error}\n  {orig}");
            }
            LineOutcome::MissingField { line: orig, field } => {
                eprintln!("apipe: line {line_no} dropped (missing: {field})\n  {orig}");
            }
        }
    }
    let _ = emitted;
    ExitCode::from(0)
}

#[derive(Debug)]
struct Predicate {
    field: String,
    op: Op,
    literal: Lit,
}

#[derive(Debug)]
enum Op {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Contains,
}

#[derive(Debug)]
enum Lit {
    Num(f64),
    Str(String),
}

impl Predicate {
    fn eval(&self, record: &Value) -> bool {
        let Some(lhs) = get_field(record, &self.field) else {
            return false;
        };
        match (&self.literal, lhs) {
            (Lit::Num(rhs), Value::Number(num)) => {
                let l = num.as_f64().unwrap_or(0.0);
                match self.op {
                    Op::Eq => (l - rhs).abs() < f64::EPSILON,
                    Op::Ne => (l - rhs).abs() >= f64::EPSILON,
                    Op::Lt => l < *rhs,
                    Op::Le => l <= *rhs,
                    Op::Gt => l > *rhs,
                    Op::Ge => l >= *rhs,
                    Op::Contains => false,
                }
            }
            (Lit::Str(rhs), Value::String(s)) => match self.op {
                Op::Eq => s == rhs,
                Op::Ne => s != rhs,
                Op::Lt => s < rhs,
                Op::Le => s <= rhs,
                Op::Gt => s > rhs,
                Op::Ge => s >= rhs,
                Op::Contains => s.contains(rhs.as_str()),
            },
            _ => false,
        }
    }
}

fn parse_predicate(expr: &str) -> Result<Predicate, String> {
    // Very small parser: `<field> <op> <literal>`. Tokens separated by whitespace.
    // Literal: quoted string `"foo"` / `'foo'`, or bare number.
    let trimmed = expr.trim();
    let (field, rest) = trimmed
        .split_once(char::is_whitespace)
        .ok_or_else(|| format!("expected `<field> <op> <literal>`: {expr}"))?;
    let rest = rest.trim_start();
    let (op_str, lit_str) = rest
        .split_once(char::is_whitespace)
        .ok_or_else(|| format!("expected `<op> <literal>`: {rest}"))?;
    let op = match op_str {
        "==" => Op::Eq,
        "!=" => Op::Ne,
        "<" => Op::Lt,
        "<=" => Op::Le,
        ">" => Op::Gt,
        ">=" => Op::Ge,
        "~" => Op::Contains,
        other => return Err(format!("unknown operator: {other}")),
    };
    let lit_str = lit_str.trim();
    let literal = if let Some(s) = lit_str.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        Lit::Str(s.to_string())
    } else if let Some(s) = lit_str.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        Lit::Str(s.to_string())
    } else if let Ok(n) = lit_str.parse::<f64>() {
        Lit::Num(n)
    } else {
        return Err(format!("bad literal (quote strings, numbers bare): {lit_str}"));
    };
    Ok(Predicate {
        field: field.to_string(),
        op,
        literal,
    })
}

fn run_schema(kind: Option<&str>) -> ExitCode {
    let mut stdout = std::io::stdout().lock();
    match kind {
        None => {
            for k in KNOWN_KINDS {
                let _ = writeln!(stdout, "{k}");
            }
            ExitCode::from(0)
        }
        Some(k) => match schema_for_kind(k) {
            Ok(schema) => {
                if let Ok(s) = serde_json::to_string_pretty(&schema) {
                    let _ = writeln!(stdout, "{s}");
                }
                ExitCode::from(0)
            }
            Err(e) => {
                eprintln!("apipe schema: {e}");
                ExitCode::from(2)
            }
        },
    }
}

fn run_pretty() -> ExitCode {
    let (records, any_input) = collect_parsed();
    if !any_input {
        return ExitCode::from(0);
    }
    let mut stdout = std::io::stdout().lock();
    let _ = writeln!(stdout, "{:<18} {:<26} {:<24} {:<20} {:>8}  payload", "kind", "id", "ts", "subject", "score");
    for p in records {
        let kind = field_string(&p.value, "kind");
        let id = field_string(&p.value, "id");
        let ts = field_string(&p.value, "ts");
        let subject = field_string(&p.value, "subject");
        let score = get_field(&p.value, "score").and_then(Value::as_f64).unwrap_or(0.0);
        let payload = get_field(&p.value, "payload")
            .map(|v| v.to_string())
            .unwrap_or_default();
        let payload_snip = truncate(&payload, 60);
        let _ = writeln!(
            stdout,
            "{:<18} {:<26} {:<24} {:<20} {:>8.3}  {}",
            truncate(&kind, 18),
            truncate(&id, 26),
            truncate(&ts, 24),
            truncate(&subject, 20),
            score,
            payload_snip
        );
    }
    ExitCode::from(0)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max - 1).collect();
        out.push('…');
        out
    }
}

fn run_to_paths() -> ExitCode {
    let mut stdout = std::io::stdout().lock();
    for (line_no, read_result) in read_lines() {
        let Ok(line) = read_result else { continue };
        if line.is_empty() {
            continue;
        }
        match parse_and_validate(&line) {
            LineOutcome::Ok(p) => {
                if let Some(path) = get_field(&p.value, "payload.path").and_then(Value::as_str) {
                    let _ = writeln!(stdout, "{path}");
                }
            }
            LineOutcome::Malformed { line: orig, error } => {
                eprintln!("apipe: line {line_no}: {error}\n  {orig}");
            }
            LineOutcome::MissingField { line: orig, field } => {
                eprintln!("apipe: line {line_no} (missing: {field})\n  {orig}");
            }
        }
    }
    ExitCode::from(0)
}

fn run_from_paths() -> ExitCode {
    let mut stdout = std::io::stdout().lock();
    let now = chrono_now_rfc3339();
    for read_result in std::io::stdin().lock().lines() {
        let Ok(line) = read_result else { continue };
        let path = line.trim();
        if path.is_empty() {
            continue;
        }
        let id = stable_id_from(path);
        let ts = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(rfc3339_from_systime)
            .unwrap_or_else(|| now.clone());
        let record = serde_json::json!({
            "kind": "file_event",
            "source": format!("from-paths:{path}"),
            "id": id,
            "ts": ts,
            "payload": { "path": path }
        });
        if let Ok(s) = serde_json::to_string(&record) {
            let _ = writeln!(stdout, "{s}");
        }
    }
    ExitCode::from(0)
}

fn stable_id_from(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let hash = hasher.finalize();
    let mut out = String::with_capacity(12);
    for b in &hash[..6] {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn chrono_now_rfc3339() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    // Minimal RFC3339 without pulling chrono in; not zero-padded perfectly but stable.
    format!("1970-01-01T00:00:{secs:02}Z")
}

fn rfc3339_from_systime(t: std::time::SystemTime) -> Option<String> {
    let secs = t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    Some(format!("1970-01-01T00:00:{secs:02}Z"))
}
