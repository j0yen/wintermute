//! Integration tests for docket — covers all PRD acceptance criteria.
//!
//! Each test uses a temporary directory so they are fully isolated.

use std::path::PathBuf;
use std::process::Command;

/// Path to the compiled docket binary.
fn docket_bin() -> PathBuf {
    // Cargo sets CARGO_MANIFEST_DIR; binary lives at target/release/docket.
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    PathBuf::from(&manifest)
        .join("target")
        .join("release")
        .join("docket")
}

/// Create a `Command` pointing at the docket binary and use `dir` as the
/// `XDG_DATA_HOME` so every test has its own isolated DB.
fn docket(dir: &std::path::Path) -> Command {
    let mut cmd = Command::new(docket_bin());
    cmd.env("XDG_DATA_HOME", dir.to_str().expect("non-UTF8 tmpdir"));
    cmd
}

// ── AC 1 — binary exists, --version and --help ──────────────────────────────

#[test]
fn test_version_flag() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = docket(tmp.path())
        .arg("--version")
        .output()
        .expect("docket --version");
    assert!(out.status.success(), "exit {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.starts_with("docket "),
        "expected 'docket <ver>', got: {stdout}"
    );
}

#[test]
fn test_help_lists_subcommands() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = docket(tmp.path())
        .arg("--help")
        .output()
        .expect("docket --help");
    assert!(out.status.success(), "exit {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    for sub in &["report", "list", "show", "resolve"] {
        assert!(
            stdout.contains(sub),
            "help output missing subcommand '{sub}': {stdout}"
        );
    }
}

// ── AC 2 — first report creates entry with expected fields ───────────────────

#[test]
fn test_first_report_creates_entry() {
    let tmp = tempfile::tempdir().expect("tempdir");

    // report
    let st = docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "k", "--title", "T"])
        .status()
        .expect("docket report");
    assert!(st.success(), "report exit {:?}", st);

    // show --format json
    let out = docket(tmp.path())
        .args(["show", "k", "--format", "json"])
        .output()
        .expect("docket show");
    assert!(out.status.success(), "show exit {:?}", out.status);

    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("show output is valid JSON");

    assert_eq!(v["status"], "open", "{v}");
    assert_eq!(v["runs_seen"], 1, "{v}");
    assert_eq!(v["consecutive_runs"], 1, "{v}");
    assert_eq!(v["report_count"], 1, "{v}");
    assert_eq!(v["first_seen"], v["last_seen"], "first_seen == last_seen on first report: {v}");
}

// ── AC 3 — same run-id is idempotent for streak ──────────────────────────────

#[test]
fn test_same_run_idempotent_for_streak() {
    let tmp = tempfile::tempdir().expect("tempdir");

    // first report
    docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "k", "--title", "T"])
        .status()
        .expect("report 1");

    // second report — same run
    docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "k", "--title", "T"])
        .status()
        .expect("report 2");

    let out = docket(tmp.path())
        .args(["show", "k", "--format", "json"])
        .output()
        .expect("show");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");

    assert_eq!(v["runs_seen"], 1, "runs_seen unchanged: {v}");
    assert_eq!(v["consecutive_runs"], 1, "streak unchanged: {v}");
    assert_eq!(v["report_count"], 2, "report_count bumped: {v}");
    // last_seen should be ≥ first_seen (both RFC3339, string comparison OK)
    assert!(
        v["last_seen"].as_str().unwrap_or("") >= v["first_seen"].as_str().unwrap_or("z"),
        "last_seen bumped: {v}"
    );
}

// ── AC 4 — new run-id advances streak ───────────────────────────────────────

#[test]
fn test_new_run_advances_streak() {
    let tmp = tempfile::tempdir().expect("tempdir");

    docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "k", "--title", "T"])
        .status()
        .expect("report r1");

    docket(tmp.path())
        .args(["report", "--run", "r2", "--key", "k", "--title", "T"])
        .status()
        .expect("report r2");

    let out = docket(tmp.path())
        .args(["show", "k", "--format", "json"])
        .output()
        .expect("show");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");

    assert_eq!(v["runs_seen"], 2, "{v}");
    assert_eq!(v["consecutive_runs"], 2, "{v}");
    assert_eq!(v["last_run"], "r2", "{v}");
}

// ── AC 5 — list open / resolve / list open omits / list resolved includes ────

#[test]
fn test_list_and_resolve_flow() {
    let tmp = tempfile::tempdir().expect("tempdir");

    docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "k", "--title", "T"])
        .status()
        .expect("report");

    // list --open --format json should contain "k"
    let out = docket(tmp.path())
        .args(["list", "--open", "--format", "json"])
        .output()
        .expect("list open");
    assert!(out.status.success());
    let arr: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json array");
    assert!(
        arr.as_array().unwrap().iter().any(|f| f["key"] == "k"),
        "k not in open list: {arr}"
    );

    // resolve
    let st = docket(tmp.path())
        .args(["resolve", "k", "--reason", "done"])
        .status()
        .expect("resolve");
    assert!(st.success(), "resolve exit {st:?}");

    // list --open should omit k
    let out = docket(tmp.path())
        .args(["list", "--open", "--format", "json"])
        .output()
        .expect("list open after resolve");
    let arr: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert!(
        !arr.as_array().unwrap().iter().any(|f| f["key"] == "k"),
        "k still in open list after resolve: {arr}"
    );

    // list --resolved should include k with resolve_reason=done
    let out = docket(tmp.path())
        .args(["list", "--resolved", "--format", "json"])
        .output()
        .expect("list resolved");
    let arr: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let entry = arr
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["key"] == "k")
        .expect("k in resolved list");
    assert_eq!(entry["resolve_reason"], "done", "{entry}");
    assert!(
        !entry["resolved_at"].is_null(),
        "resolved_at non-null: {entry}"
    );
}

// ── AC 6 — reporting a resolved finding reopens it ───────────────────────────

#[test]
fn test_reopen_resolved_finding() {
    let tmp = tempfile::tempdir().expect("tempdir");

    docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "k", "--title", "T"])
        .status()
        .expect("report");
    docket(tmp.path())
        .args(["resolve", "k"])
        .status()
        .expect("resolve");

    // report again — should reopen
    docket(tmp.path())
        .args(["report", "--run", "r2", "--key", "k", "--title", "T2"])
        .status()
        .expect("report after resolve");

    let out = docket(tmp.path())
        .args(["show", "k", "--format", "json"])
        .output()
        .expect("show");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");

    assert_eq!(v["status"], "open", "reopened: {v}");
    assert_eq!(v["consecutive_runs"], 1, "streak reset: {v}");
    assert!(v["resolved_at"].is_null(), "resolved_at cleared: {v}");
}

// ── AC 7 — show unknown key exits nonzero; list on empty store exits 0 ───────

#[test]
fn test_show_unknown_key_exits_nonzero() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = docket(tmp.path())
        .args(["show", "does-not-exist"])
        .output()
        .expect("show unknown");
    assert!(
        !out.status.success(),
        "expected nonzero exit for unknown key"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.is_empty() || !String::from_utf8_lossy(&out.stdout).is_empty(),
        "expected some output for unknown key"
    );
}

#[test]
fn test_list_empty_store_exits_zero() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = docket(tmp.path())
        .args(["list"])
        .output()
        .expect("list empty");
    assert!(out.status.success(), "exit {:?}", out.status);
}

#[test]
fn test_list_empty_store_json_is_empty_array() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = docket(tmp.path())
        .args(["list", "--format", "json"])
        .output()
        .expect("list empty json");
    assert!(out.status.success());
    let arr: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert!(arr.as_array().unwrap().is_empty(), "expected []");
}

// ── AC 8 — XDG_DATA_HOME respected; data persists across processes ───────────

#[test]
fn test_xdg_path_and_persistence() {
    let tmp = tempfile::tempdir().expect("tempdir");

    // process 1: report
    docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "persist-test", "--title", "Persisted"])
        .status()
        .expect("report process 1");

    // process 2: list — must find the finding
    let out = docket(tmp.path())
        .args(["list", "--format", "json"])
        .output()
        .expect("list process 2");
    assert!(out.status.success());
    let arr: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert!(
        arr.as_array()
            .unwrap()
            .iter()
            .any(|f| f["key"] == "persist-test"),
        "finding not persisted across processes: {arr}"
    );
}

// ── AC 9 — JSON output valid for jq / python3 -m json.tool ─────────────────

#[test]
fn test_json_output_parseable() {
    let tmp = tempfile::tempdir().expect("tempdir");

    docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "jq-test", "--title", "JSON check"])
        .status()
        .expect("report");

    // list json
    let out = docket(tmp.path())
        .args(["list", "--format", "json"])
        .output()
        .expect("list json");
    let _: serde_json::Value = serde_json::from_slice(&out.stdout)
        .expect("list --format json must be valid JSON");

    // show json
    let out = docket(tmp.path())
        .args(["show", "jq-test", "--format", "json"])
        .output()
        .expect("show json");
    let _: serde_json::Value = serde_json::from_slice(&out.stdout)
        .expect("show --format json must be valid JSON");
}
