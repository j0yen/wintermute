//! Walk `~/.claude/projects/<dir>/*.jsonl`, parse each line, and load rows
//! into the FTS5 index.

use anyhow::{Context, Result};
use std::io::{BufRead, BufReader};
use std::path::Path;
use walkdir::WalkDir;

use crate::index::Index;
use crate::jsonl::parse_line;

#[derive(Debug, Default)]
pub struct ReindexReport {
    pub files: usize,
    pub rows: usize,
    pub skipped_lines: usize,
}

/// Walk `projects_root` for `*.jsonl` files (one level deep into per-project
/// directories), parse, and load into `idx`. Caller is responsible for
/// calling `idx.truncate()` first when doing a full reindex.
pub fn run(idx: &mut Index, projects_root: &Path) -> Result<ReindexReport> {
    let mut report = ReindexReport::default();
    if !projects_root.exists() {
        return Ok(report);
    }
    for entry in WalkDir::new(projects_root)
        .min_depth(2)
        .max_depth(2)
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let session_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_owned();
        let project_dir = path
            .parent()
            .and_then(Path::file_name)
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_owned();
        let (rows, skipped) = index_one(idx, &session_id, &project_dir, path)?;
        report.files += 1;
        report.rows += rows;
        report.skipped_lines += skipped;
    }
    Ok(report)
}

fn index_one(
    idx: &mut Index,
    session_id: &str,
    project_dir: &str,
    file: &Path,
) -> Result<(usize, usize)> {
    let f = std::fs::File::open(file).with_context(|| format!("open {}", file.display()))?;
    let reader = BufReader::new(f);
    let mut records = Vec::new();
    let mut skipped = 0;
    for line in reader.lines() {
        let Ok(line) = line else {
            skipped += 1;
            continue;
        };
        if line.trim().is_empty() {
            continue;
        }
        let rows = parse_line(&line);
        if rows.is_empty() {
            // Either a skip-by-design type (system / ai-title / …) or
            // malformed. We can't distinguish without re-parsing; counting
            // every empty parse as "skipped" overstates the noise. Only
            // count lines that fail JSON parse outright.
            if serde_json::from_str::<serde_json::Value>(&line).is_err() {
                skipped += 1;
            }
            continue;
        }
        records.extend(rows);
    }
    let n = records.len();
    if !records.is_empty() {
        idx.insert_session(session_id, project_dir, &records)?;
    }
    Ok((n, skipped))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn walks_and_indexes() {
        let tmp = tempdir().unwrap();
        let projects = tmp.path().join("projects");
        let dir = projects.join("-home-jsy-foo");
        std::fs::create_dir_all(&dir).unwrap();
        let mut f = std::fs::File::create(dir.join("abc.jsonl")).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","timestamp":"t","message":{{"role":"user","content":"hello transcript"}}}}"#
        )
        .unwrap();
        writeln!(
            f,
            r#"{{"type":"assistant","timestamp":"t","message":{{"role":"assistant","content":[{{"type":"text","text":"hi back"}}]}}}}"#
        )
        .unwrap();
        writeln!(f, "garbage line that won't parse").unwrap();
        drop(f);

        let db = tmp.path().join("t.sqlite");
        let mut idx = Index::open(&db).unwrap();
        let report = run(&mut idx, &projects).unwrap();
        assert_eq!(report.files, 1);
        assert_eq!(report.rows, 2);
        assert_eq!(report.skipped_lines, 1);

        let hits = idx.query("transcript", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].session_id, "abc");
        assert_eq!(hits[0].project_dir, "-home-jsy-foo");
    }

    #[test]
    fn missing_projects_root_is_ok() {
        let tmp = tempdir().unwrap();
        let db = tmp.path().join("t.sqlite");
        let mut idx = Index::open(&db).unwrap();
        let r = run(&mut idx, &tmp.path().join("does-not-exist")).unwrap();
        assert_eq!(r.files, 0);
    }
}
