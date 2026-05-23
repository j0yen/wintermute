//! End-to-end: lay down JSONLs, run reindex, query.

#![allow(clippy::expect_used)]

use session_index::index::Index;
use session_index::{paths, reindex};
use std::io::Write;

#[test]
fn reindex_and_query_against_a_fake_projects_tree() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let projects = tmp.path().join("projects");
    let session_dir = projects.join("-home-jsy-foo");
    std::fs::create_dir_all(&session_dir).expect("mkdir");

    let mut f = std::fs::File::create(session_dir.join("sess-1.jsonl")).expect("create jsonl");
    let lines = [
        r#"{"type":"user","timestamp":"2026-05-23T10:00:00Z","message":{"role":"user","content":"please bisect the regression"}}"#,
        r#"{"type":"assistant","timestamp":"2026-05-23T10:00:01Z","message":{"role":"assistant","content":[{"type":"text","text":"running git bisect"},{"type":"tool_use","name":"Bash","input":{"command":"git bisect start","description":"begin bisect"}}]}}"#,
        r#"{"type":"user","timestamp":"2026-05-23T10:00:02Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"x","content":"bisect started"}]}}"#,
    ];
    for l in &lines {
        writeln!(f, "{l}").expect("write");
    }
    drop(f);

    let root = tmp.path().join("transcript");
    std::fs::create_dir_all(paths::index_dir(&root)).expect("mkdir index");

    let mut idx = Index::open(&paths::index_db(&root)).expect("open");
    idx.truncate().expect("truncate");
    let report = reindex::run(&mut idx, &projects).expect("reindex");
    assert_eq!(report.files, 1, "saw one file");
    assert_eq!(
        report.rows, 4,
        "user + assistant text + tool_use + tool_result"
    );
    assert_eq!(report.skipped_lines, 0);

    let bisect_hits = idx.query("bisect", 10).expect("query");
    assert!(
        bisect_hits.len() >= 2,
        "bisect appears in user, assistant, and Bash command"
    );
    let bash_hits = idx.query("git", 10).expect("query");
    assert!(bash_hits.iter().any(|h| h.role == "tool_use"));

    let stats = idx.stats().expect("stats");
    assert_eq!(stats.session_count, 1);
    assert_eq!(stats.row_count, 4);
}
