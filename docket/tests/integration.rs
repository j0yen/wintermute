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
    for sub in &["report", "list", "show", "resolve", "sweep", "digest", "ack", "unack"] {
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

// ═══════════════════════════════════════════════════════════════════════════
// docket-escalate acceptance criteria
// ═══════════════════════════════════════════════════════════════════════════

// ── escalate AC 1 — 3 runs escalates; escalated_at + reason set ─────────────

#[test]
fn test_escalate_at_threshold() {
    let tmp = tempfile::tempdir().expect("tempdir");

    for run in &["r1", "r2", "r3"] {
        let st = docket(tmp.path())
            .args(["report", "--run", run, "--key", "k", "--title", "T"])
            .status()
            .expect("report");
        assert!(st.success(), "report {run} failed");
    }

    let out = docket(tmp.path())
        .args(["show", "k", "--format", "json"])
        .output()
        .expect("show");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");

    assert_eq!(v["status"], "escalated", "should be escalated after 3 runs: {v}");
    assert!(
        !v["escalated_at"].is_null(),
        "escalated_at should be non-null: {v}"
    );
    let reason = v["escalation_reason"].as_str().unwrap_or("");
    assert!(
        reason.contains('3'),
        "escalation_reason should mention '3': {v}"
    );
    assert!(
        reason.contains("SKILL.md"),
        "escalation_reason should mention SKILL.md: {v}"
    );
}

// ── escalate AC 2 — 2 runs stays open; 3rd trips; --escalate-threshold 2 ────

#[test]
fn test_escalate_two_runs_stay_open() {
    let tmp = tempfile::tempdir().expect("tempdir");

    for run in &["r1", "r2"] {
        docket(tmp.path())
            .args(["report", "--run", run, "--key", "k", "--title", "T"])
            .status()
            .expect("report");
    }

    let out = docket(tmp.path())
        .args(["show", "k", "--format", "json"])
        .output()
        .expect("show");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(v["status"], "open", "2 runs should stay open (threshold=3): {v}");
}

#[test]
fn test_escalate_threshold_2_trips_on_second_run() {
    let tmp = tempfile::tempdir().expect("tempdir");

    docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "k", "--title", "T", "--escalate-threshold", "2"])
        .status()
        .expect("report r1");

    docket(tmp.path())
        .args(["report", "--run", "r2", "--key", "k", "--title", "T", "--escalate-threshold", "2"])
        .status()
        .expect("report r2");

    let out = docket(tmp.path())
        .args(["show", "k", "--format", "json"])
        .output()
        .expect("show");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(v["status"], "escalated", "--escalate-threshold 2 should escalate at run 2: {v}");
}

// ── escalate AC 3 — list --escalated returns only escalated findings ─────────

#[test]
fn test_list_escalated_filter() {
    let tmp = tempfile::tempdir().expect("tempdir");

    // Escalate k1 (3 runs)
    for run in &["r1", "r2", "r3"] {
        docket(tmp.path())
            .args(["report", "--run", run, "--key", "k1", "--title", "T1"])
            .status()
            .expect("report k1");
    }
    // k2 stays open (1 run)
    docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "k2", "--title", "T2"])
        .status()
        .expect("report k2");

    let out = docket(tmp.path())
        .args(["list", "--escalated", "--format", "json"])
        .output()
        .expect("list escalated");
    assert!(out.status.success(), "exit {:?}", out.status);
    let arr: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let arr = arr.as_array().expect("json array");

    assert!(
        arr.iter().any(|f| f["key"] == "k1"),
        "k1 should be in escalated list: {arr:?}"
    );
    assert!(
        !arr.iter().any(|f| f["key"] == "k2"),
        "k2 (open) should not be in escalated list: {arr:?}"
    );
}

// ── escalate AC 4 — once escalated, 4th report stays escalated ──────────────

#[test]
fn test_escalated_is_sticky() {
    let tmp = tempfile::tempdir().expect("tempdir");

    for run in &["r1", "r2", "r3", "r4"] {
        docket(tmp.path())
            .args(["report", "--run", run, "--key", "k", "--title", "T"])
            .status()
            .expect("report");
    }

    let out = docket(tmp.path())
        .args(["show", "k", "--format", "json"])
        .output()
        .expect("show");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(v["status"], "escalated", "4th report should keep escalated: {v}");
}

// ── escalate AC 5 — runs table records run-ids once, in arrival order ────────

#[test]
fn test_runs_table_no_duplicates() {
    let tmp = tempfile::tempdir().expect("tempdir");

    // r1, r2, then r1 again — should only have r1 seq=1, r2 seq=2
    for run in &["r1", "r2", "r1"] {
        docket(tmp.path())
            .args(["report", "--run", run, "--key", "k", "--title", "T"])
            .status()
            .expect("report");
    }

    // Verify via sweep on r3 (records r3 in ledger too)
    // Use stale-after=100 so nothing is resolved — we just care the ledger didn't duplicate
    let st = docket(tmp.path())
        .args(["sweep", "--run", "r3", "--stale-after", "100"])
        .status()
        .expect("sweep");
    assert!(st.success(), "sweep exit {:?}", st);

    // After r1, r2, r1 (dup), r3 — finding should have consecutive_runs=3 (r1→r2→r3)
    // if ledger is correct (r1 seq=1, r2 seq=2, r3 seq=3).
    // The re-report of r1 is a same-run-id idempotent bump (doesn't advance streak).
    // So consecutive_runs from the last report (r1) is still 1 (r1 last seen),
    // but after sweep with r3 being seen, the finding's last_run is r1 not r3 → streak reset to 0.
    // This verifies the ledger has no duplicate seq for r1.
    let out = docket(tmp.path())
        .args(["show", "k", "--format", "json"])
        .output()
        .expect("show");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    // After sweep, finding not seen at r3 → streak reset to 0 (not stale yet with stale-after=100)
    assert_eq!(v["consecutive_runs"], 0, "streak reset after gap: {v}");
}

// ── escalate AC 6 — sweep resolves stale, does not resolve recent ────────────

#[test]
fn test_sweep_resolves_stale_not_recent() {
    let tmp = tempfile::tempdir().expect("tempdir");

    // finding1: last seen at r2, should be stale after r3, r4 elapsed (2 runs)
    for run in &["r1", "r2"] {
        docket(tmp.path())
            .args(["report", "--run", run, "--key", "finding1", "--title", "Old"])
            .status()
            .expect("report finding1");
    }
    // finding2: last seen at r4, should NOT be stale
    for run in &["r1", "r2", "r3", "r4"] {
        docket(tmp.path())
            .args(["report", "--run", run, "--key", "finding2", "--title", "Recent"])
            .status()
            .expect("report finding2");
    }

    // Record r3 and r4 via sweeps (without resolving anything yet with stale-after=100)
    docket(tmp.path())
        .args(["sweep", "--run", "r3", "--stale-after", "100"])
        .status()
        .expect("sweep r3");
    docket(tmp.path())
        .args(["sweep", "--run", "r4", "--stale-after", "100"])
        .status()
        .expect("sweep r4");

    // Now sweep r5 with stale-after=2
    let st = docket(tmp.path())
        .args(["sweep", "--run", "r5", "--stale-after", "2"])
        .status()
        .expect("sweep r5");
    assert!(st.success(), "sweep r5 exit {:?}", st);

    // finding1 (last seen r2, 2 runs elapsed r3+r4 before r5) should be resolved
    let out = docket(tmp.path())
        .args(["show", "finding1", "--format", "json"])
        .output()
        .expect("show finding1");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(v["status"], "resolved", "finding1 should be resolved as stale: {v}");
    let reason = v["resolve_reason"].as_str().unwrap_or("");
    assert!(
        reason.starts_with("stale:"),
        "resolve_reason should start with 'stale:': {v}"
    );

    // finding2 (last seen r4) should remain escalated/open (not stale)
    let out = docket(tmp.path())
        .args(["show", "finding2", "--format", "json"])
        .output()
        .expect("show finding2");
    let v2: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert!(
        v2["status"] != "resolved",
        "finding2 (last seen r4) should not be resolved at sweep r5: {v2}"
    );
}

// ── escalate AC 7 — gap at r2 breaks streak (r1, r3 not consecutive) ─────────

#[test]
fn test_gap_breaks_streak() {
    let tmp = tempfile::tempdir().expect("tempdir");

    // Report at r1
    docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "k", "--title", "T"])
        .status()
        .expect("report r1");

    // Skip r2 — sweep with stale-after=100 to record r2 in ledger without resolving
    docket(tmp.path())
        .args(["sweep", "--run", "r2", "--stale-after", "100"])
        .status()
        .expect("sweep r2");

    // Report at r3 — after the gap, streak should reset
    docket(tmp.path())
        .args(["report", "--run", "r3", "--key", "k", "--title", "T"])
        .status()
        .expect("report r3");

    let out = docket(tmp.path())
        .args(["show", "k", "--format", "json"])
        .output()
        .expect("show");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");

    // After sweep at r2 (gap), consecutive_runs is reset to 0.
    // Then report at r3 reopens/continues: streak = 1 (not 2, since gap was recorded).
    // The sweep resets streak to 0; the r3 report sets streak=1.
    // So it's NOT monotonically 1→2 through the gap.
    assert_eq!(v["consecutive_runs"], 1, "after gap at r2, r3 report starts fresh streak=1: {v}");
    assert_eq!(v["status"], "open", "status should be open (streak=1 < threshold=3): {v}");
}

// ── escalate AC 8 — migration is idempotent on pre-escalate DB ───────────────

#[test]
fn test_migration_idempotent() {
    let tmp = tempfile::tempdir().expect("tempdir");

    // Two opens on a fresh DB — migration runs twice (once per process), must not error.
    for run in &["r1", "r2"] {
        let st = docket(tmp.path())
            .args(["report", "--run", run, "--key", "k", "--title", "T"])
            .status()
            .expect("report");
        assert!(st.success(), "migration idempotent run {run}: {:?}", st);
    }

    // Third invocation: list — must also succeed.
    let st = docket(tmp.path())
        .args(["list", "--format", "json"])
        .status()
        .expect("list");
    assert!(st.success(), "list after repeated opens: {:?}", st);
}

// ── escalate AC 9 — DOCKET_ESCALATE_THRESHOLD env honored; flag overrides ────

#[test]
fn test_env_escalate_threshold() {
    let tmp = tempfile::tempdir().expect("tempdir");

    // With DOCKET_ESCALATE_THRESHOLD=2, 2 runs should escalate
    for run in &["r1", "r2"] {
        let st = docket(tmp.path())
            .env("DOCKET_ESCALATE_THRESHOLD", "2")
            .args(["report", "--run", run, "--key", "k", "--title", "T"])
            .status()
            .expect("report");
        assert!(st.success());
    }

    let out = docket(tmp.path())
        .args(["show", "k", "--format", "json"])
        .output()
        .expect("show");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(
        v["status"], "escalated",
        "DOCKET_ESCALATE_THRESHOLD=2 should escalate at 2 runs: {v}"
    );
}

#[test]
fn test_flag_overrides_env_escalate_threshold() {
    let tmp = tempfile::tempdir().expect("tempdir");

    // DOCKET_ESCALATE_THRESHOLD=2 but --escalate-threshold 5 overrides → stays open at 2 runs
    for run in &["r1", "r2"] {
        docket(tmp.path())
            .env("DOCKET_ESCALATE_THRESHOLD", "2")
            .args(["report", "--run", run, "--key", "k", "--title", "T", "--escalate-threshold", "5"])
            .status()
            .expect("report");
    }

    let out = docket(tmp.path())
        .args(["show", "k", "--format", "json"])
        .output()
        .expect("show");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(
        v["status"], "open",
        "--escalate-threshold 5 should override DOCKET_ESCALATE_THRESHOLD=2 (stays open at 2 runs): {v}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// docket-digest acceptance criteria
// ═══════════════════════════════════════════════════════════════════════════

// ── digest AC 1 — empty store: text is single clean line, json has status=ok ──

#[test]
fn digest_empty_store_text_single_line() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = docket(tmp.path())
        .args(["digest"])
        .output()
        .expect("digest empty");
    assert!(out.status.success(), "exit {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let trimmed = stdout.trim();
    assert_eq!(trimmed, "docket: 0 open", "empty text: {trimmed:?}");
    assert_eq!(trimmed.lines().count(), 1, "single line: {trimmed:?}");
}

#[test]
fn digest_empty_store_json_status_ok() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = docket(tmp.path())
        .args(["digest", "--format", "json"])
        .output()
        .expect("digest empty json");
    assert!(out.status.success(), "exit {:?}", out.status);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid json");
    assert_eq!(v["status"], "ok", "empty store status: {v}");
    assert_eq!(v["detail"]["open"], 0);
    assert_eq!(v["detail"]["escalated"], 0);
    assert_eq!(v["component"], "docket");
}

// ── digest AC 2 — 4 open + 1 escalated: text reports counts + oldest ─────────

#[test]
fn digest_four_open_one_escalated_text() {
    let tmp = tempfile::tempdir().expect("tempdir");

    // 4 open findings
    for (key, run_count) in &[("o1", 1i32), ("o2", 2), ("o3", 3), ("o4-crit", 1)] {
        for r in 1..=*run_count {
            docket(tmp.path())
                .args(["report", "--run", &format!("r{r}"), "--key", key, "--title", key])
                .status()
                .expect("report open");
        }
    }
    // o4-crit: mark as crit
    docket(tmp.path())
        .args(["report", "--run", "rx", "--key", "o4-crit", "--title", "crit finding", "--severity", "crit"])
        .status()
        .expect("report crit");

    // 1 escalated finding: report 12 distinct runs
    for r in 1..=12_i32 {
        docket(tmp.path())
            .args([
                "report", "--run", &format!("esc-r{r}"),
                "--key", "escalated-finding",
                "--title", "escalated finding",
                "--escalate-threshold", "3",
            ])
            .status()
            .expect("report escalated");
    }

    let out = docket(tmp.path())
        .args(["digest"])
        .output()
        .expect("digest");
    assert!(out.status.success(), "exit {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);

    // Must mention open count (4 open across all open findings, plus escalated-finding is escalated)
    assert!(stdout.contains("open"), "open count: {stdout}");
    // Must mention escalated
    assert!(stdout.contains("escalated"), "escalated: {stdout}");
    // Must mention oldest finding by name (runs_seen=12 → escalated-finding)
    assert!(stdout.contains("escalated-finding"), "oldest: {stdout}");
    // ≤3 lines
    let line_count = stdout.trim().lines().count();
    assert!(line_count <= 3, "≤3 lines ({line_count}): {stdout:?}");
}

// ── digest AC 3 — json field names match wm.health.* envelope ────────────────

#[test]
fn digest_json_envelope_field_names() {
    let tmp = tempfile::tempdir().expect("tempdir");
    docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "k", "--title", "T"])
        .status()
        .expect("report");

    let out = docket(tmp.path())
        .args(["digest", "--format", "json"])
        .output()
        .expect("digest json");
    assert!(out.status.success(), "exit {:?}", out.status);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid json");

    // Top-level fields required by the wm.health.* envelope spec
    assert!(v.get("component").is_some(), "missing 'component': {v}");
    assert!(v.get("status").is_some(), "missing 'status': {v}");
    assert!(v.get("summary").is_some(), "missing 'summary': {v}");
    assert!(v.get("detail").is_some(), "missing 'detail': {v}");

    let d = &v["detail"];
    assert!(d.get("open").is_some(), "missing detail.open: {v}");
    assert!(d.get("escalated").is_some(), "missing detail.escalated: {v}");
    assert!(d.get("crit").is_some(), "missing detail.crit: {v}");
    assert!(d.get("oldest_key").is_some(), "missing detail.oldest_key: {v}");
    assert!(d.get("oldest_runs").is_some(), "missing detail.oldest_runs: {v}");
    assert!(d.get("escalated_keys").is_some(), "missing detail.escalated_keys: {v}");
    assert_eq!(v["component"], "docket");
}

// ── digest AC 4 — status mapping branches ────────────────────────────────────

#[test]
fn digest_status_mapping_open_only_is_ok() {
    let tmp = tempfile::tempdir().expect("tempdir");
    docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "k", "--title", "T"])
        .status()
        .expect("report");

    let out = docket(tmp.path())
        .args(["digest", "--format", "json"])
        .output()
        .expect("digest json");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(v["status"], "ok", "open-only should be ok: {v}");
}

#[test]
fn digest_status_mapping_escalated_is_degraded() {
    let tmp = tempfile::tempdir().expect("tempdir");
    // 3 runs → escalated (threshold=3)
    for r in 1..=3_i32 {
        docket(tmp.path())
            .args(["report", "--run", &format!("r{r}"), "--key", "k", "--title", "T"])
            .status()
            .expect("report");
    }

    let out = docket(tmp.path())
        .args(["digest", "--format", "json"])
        .output()
        .expect("digest json");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(v["status"], "degraded", "escalated (warn) should be degraded: {v}");
}

#[test]
fn digest_status_mapping_escalated_crit_is_down() {
    let tmp = tempfile::tempdir().expect("tempdir");
    // 3 runs with severity=crit → escalated crit
    for r in 1..=3_i32 {
        docket(tmp.path())
            .args([
                "report", "--run", &format!("r{r}"),
                "--key", "k", "--title", "T",
                "--severity", "crit",
            ])
            .status()
            .expect("report");
    }

    let out = docket(tmp.path())
        .args(["digest", "--format", "json"])
        .output()
        .expect("digest json");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(v["status"], "down", "escalated crit should be down: {v}");
}

// ── digest AC 5 — --severity warn excludes info findings ─────────────────────

#[test]
fn digest_severity_filter_excludes_info() {
    let tmp = tempfile::tempdir().expect("tempdir");
    // Report an info finding only
    docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "info-k", "--title", "T", "--severity", "info"])
        .status()
        .expect("report info");

    let out = docket(tmp.path())
        .args(["digest", "--format", "json", "--severity", "warn"])
        .output()
        .expect("digest json severity=warn");
    assert!(out.status.success(), "exit {:?}", out.status);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(v["status"], "ok", "info finding should be excluded by --severity warn: {v}");
    assert_eq!(v["detail"]["open"], 0, "open should be 0 after info excluded: {v}");
}

// ── digest AC 6 — oldest uses runs_seen, stable across formats ───────────────

#[test]
fn digest_oldest_uses_run_age() {
    let tmp = tempfile::tempdir().expect("tempdir");

    // k1: 1 run, k2: 5 runs, k3: 2 runs — k2 should be oldest
    docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "k1", "--title", "T1"])
        .status()
        .expect("report k1");
    for r in 1..=5_i32 {
        docket(tmp.path())
            .args(["report", "--run", &format!("r{r}"), "--key", "k2", "--title", "T2"])
            .status()
            .expect("report k2");
    }
    for r in 1..=2_i32 {
        docket(tmp.path())
            .args(["report", "--run", &format!("r{r}"), "--key", "k3", "--title", "T3"])
            .status()
            .expect("report k3");
    }

    // JSON check
    let out = docket(tmp.path())
        .args(["digest", "--format", "json"])
        .output()
        .expect("digest json");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(
        v["detail"]["oldest_key"], "k2",
        "oldest should be k2 (5 runs): {v}"
    );
    assert_eq!(v["detail"]["oldest_runs"], 5);

    // Text check — should also name k2
    let out = docket(tmp.path())
        .args(["digest"])
        .output()
        .expect("digest text");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("k2"), "text should name oldest (k2): {stdout}");
}

// ── digest AC 7 — json parseable; text ≤3 lines ──────────────────────────────

#[test]
fn digest_json_parseable_with_jq() {
    let tmp = tempfile::tempdir().expect("tempdir");
    docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "k", "--title", "T"])
        .status()
        .expect("report");

    let out = docket(tmp.path())
        .args(["digest", "--format", "json"])
        .output()
        .expect("digest json");
    assert!(out.status.success(), "exit {:?}", out.status);
    // Verify valid JSON parseable by serde_json (same as jq can parse)
    let _: serde_json::Value = serde_json::from_slice(&out.stdout).expect("digest json must be valid JSON");
}

#[test]
fn digest_text_max_three_lines() {
    let tmp = tempfile::tempdir().expect("tempdir");
    // escalated finding
    for r in 1..=3_i32 {
        docket(tmp.path())
            .args(["report", "--run", &format!("r{r}"), "--key", "esc", "--title", "E"])
            .status()
            .expect("report esc");
    }
    // open finding
    docket(tmp.path())
        .args(["report", "--run", "ro1", "--key", "open1", "--title", "O"])
        .status()
        .expect("report open");

    let out = docket(tmp.path())
        .args(["digest"])
        .output()
        .expect("digest text");
    assert!(out.status.success(), "exit {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let line_count = stdout.trim().lines().count();
    assert!(line_count <= 3, "text must be ≤3 lines ({line_count}): {stdout:?}");
}

// ── digest AC 10 — no escalated: degrades gracefully, escalated=0 ────────────

#[test]
fn digest_no_escalated_graceful() {
    let tmp = tempfile::tempdir().expect("tempdir");
    docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "k", "--title", "T"])
        .status()
        .expect("report");

    let out = docket(tmp.path())
        .args(["digest", "--format", "json"])
        .output()
        .expect("digest json");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(v["status"], "ok");
    assert_eq!(v["detail"]["escalated"], 0);
    let keys = v["detail"]["escalated_keys"].as_array().expect("array");
    assert!(keys.is_empty(), "escalated_keys empty when no escalations: {v}");
}

// ═══════════════════════════════════════════════════════════════════════════
// docket-evidence acceptance criteria
// ═══════════════════════════════════════════════════════════════════════════

// ── evidence AC 1 — two --evidence refs create two typed rows ───────────────

#[test]
fn test_evidence_two_kinds_in_one_report() {
    let tmp = tempfile::tempdir().expect("tempdir");

    let st = docket(tmp.path())
        .args([
            "report", "--run", "r1", "--key", "k",
            "--title", "T",
            "--evidence", "recall:01KSRV7R4FERPP40HQGV5RGZNT",
            "--evidence", "pid:2138939",
        ])
        .status()
        .expect("report");
    assert!(st.success(), "report exit {:?}", st);

    let out = docket(tmp.path())
        .args(["show", "k", "--format", "json"])
        .output()
        .expect("show");
    assert!(out.status.success(), "show exit {:?}", out.status);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");

    let trail = v["evidence_trail"].as_array().expect("evidence_trail array");
    assert_eq!(trail.len(), 2, "expected 2 evidence rows: {v}");

    let recall_row = trail.iter().find(|r| r["kind"] == "recall").expect("recall row");
    assert_eq!(recall_row["ref_val"], "01KSRV7R4FERPP40HQGV5RGZNT", "{recall_row}");
    assert_eq!(recall_row["run_id"], "r1", "{recall_row}");

    let pid_row = trail.iter().find(|r| r["kind"] == "pid").expect("pid row");
    assert_eq!(pid_row["ref_val"], "2138939", "{pid_row}");
}

// ── evidence AC 2 — second run appends; first-run rows are unchanged ─────────

#[test]
fn test_evidence_append_only() {
    let tmp = tempfile::tempdir().expect("tempdir");

    // r1: two evidence refs
    docket(tmp.path())
        .args([
            "report", "--run", "r1", "--key", "k",
            "--title", "T",
            "--evidence", "recall:01KSRV7R4FERPP40HQGV5RGZNT",
            "--evidence", "pid:2138939",
        ])
        .status()
        .expect("report r1");

    // r2: one more evidence ref
    docket(tmp.path())
        .args([
            "report", "--run", "r2", "--key", "k",
            "--title", "T",
            "--evidence", "provfs:1780026726",
        ])
        .status()
        .expect("report r2");

    let out = docket(tmp.path())
        .args(["show", "k", "--format", "json"])
        .output()
        .expect("show");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let trail = v["evidence_trail"].as_array().expect("evidence_trail");
    assert_eq!(trail.len(), 3, "expected 3 evidence rows after r2: {v}");

    // r1 rows must be unchanged
    let r1_rows: Vec<_> = trail.iter().filter(|r| r["run_id"] == "r1").collect();
    assert_eq!(r1_rows.len(), 2, "r1 should still have 2 rows: {v}");

    // r2 row
    let r2_rows: Vec<_> = trail.iter().filter(|r| r["run_id"] == "r2").collect();
    assert_eq!(r2_rows.len(), 1, "r2 should have 1 row: {v}");
    assert_eq!(r2_rows[0]["kind"], "provfs", "{}", r2_rows[0]);
    assert_eq!(r2_rows[0]["ref_val"], "1780026726", "{}", r2_rows[0]);
}

// ── evidence AC 3 — unknown prefix stored as kind=raw ───────────────────────

#[test]
fn test_evidence_unknown_prefix_stored_as_raw() {
    let tmp = tempfile::tempdir().expect("tempdir");

    let st = docket(tmp.path())
        .args([
            "report", "--run", "r1", "--key", "k",
            "--title", "T",
            "--evidence", "somethingweird",
        ])
        .status()
        .expect("report");
    assert!(st.success(), "report should succeed for unknown prefix: {:?}", st);

    let out = docket(tmp.path())
        .args(["show", "k", "--format", "json"])
        .output()
        .expect("show");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let trail = v["evidence_trail"].as_array().expect("evidence_trail");
    assert_eq!(trail.len(), 1, "one row: {v}");
    assert_eq!(trail[0]["kind"], "raw", "kind should be raw: {v}");
    assert_eq!(trail[0]["ref_val"], "somethingweird", "{v}");
}

// ── evidence AC 4 — text output renders Evidence section ────────────────────

#[test]
fn test_evidence_text_output_has_evidence_section() {
    let tmp = tempfile::tempdir().expect("tempdir");

    docket(tmp.path())
        .args([
            "report", "--run", "r1", "--key", "k",
            "--title", "T",
            "--evidence", "recall:01KSRV7R4FERPP40HQGV5RGZNT",
            "--evidence", "pid:2138939",
        ])
        .status()
        .expect("report");

    let out = docket(tmp.path())
        .args(["show", "k"])
        .output()
        .expect("show text");
    assert!(out.status.success(), "show exit {:?}", out.status);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("Evidence"), "text output should contain 'Evidence': {text}");
    assert!(text.contains("recall"), "text output should contain 'recall': {text}");
    assert!(text.contains("01KSRV7R4FERPP40HQGV5RGZNT"), "text output should contain the ULID: {text}");
    assert!(text.contains("[r1]"), "text output should contain run_id: {text}");
}

// ── evidence AC 5 — show json has evidence array; list json has evidence_count

#[test]
fn test_evidence_json_fields() {
    let tmp = tempfile::tempdir().expect("tempdir");

    docket(tmp.path())
        .args([
            "report", "--run", "r1", "--key", "k",
            "--title", "T",
            "--evidence", "recall:AAAA",
            "--evidence", "pid:123",
            "--evidence", "commit:abc1234",
        ])
        .status()
        .expect("report");

    // show json: evidence_trail length == 3
    let out = docket(tmp.path())
        .args(["show", "k", "--format", "json"])
        .output()
        .expect("show json");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let trail = v["evidence_trail"].as_array().expect("evidence_trail");
    assert_eq!(trail.len(), 3, "show json evidence_trail length: {v}");

    // list json: evidence_count == 3
    let out = docket(tmp.path())
        .args(["list", "--format", "json"])
        .output()
        .expect("list json");
    let arr: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let entry = arr.as_array().unwrap().iter().find(|f| f["key"] == "k").expect("k in list");
    assert_eq!(entry["evidence_count"], 3, "list json evidence_count: {arr}");
}

// ── evidence AC 6 — N --evidence flags in one report yield N rows ────────────

#[test]
fn test_evidence_repeatable_n_flags() {
    let tmp = tempfile::tempdir().expect("tempdir");

    docket(tmp.path())
        .args([
            "report", "--run", "r1", "--key", "k", "--title", "T",
            "--evidence", "recall:A",
            "--evidence", "pid:1",
            "--evidence", "path:/foo",
            "--evidence", "journal:2026-05-29",
            "--evidence", "provfs:123",
        ])
        .status()
        .expect("report");

    let out = docket(tmp.path())
        .args(["show", "k", "--format", "json"])
        .output()
        .expect("show");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let trail = v["evidence_trail"].as_array().expect("evidence_trail");
    assert_eq!(trail.len(), 5, "5 evidence refs → 5 rows: {v}");
}

// ── evidence AC 7 — malformed recall ULID stored; report exits 0 ────────────

#[test]
fn test_evidence_malformed_recall_stored_not_rejected() {
    let tmp = tempfile::tempdir().expect("tempdir");

    let st = docket(tmp.path())
        .args([
            "report", "--run", "r1", "--key", "k", "--title", "T",
            "--evidence", "recall:NOT-A-VALID-ULID",
        ])
        .status()
        .expect("report");
    // Must exit 0 — malformed refs are never rejected
    assert!(st.success(), "malformed recall ref must not cause nonzero exit: {:?}", st);

    let out = docket(tmp.path())
        .args(["show", "k", "--format", "json"])
        .output()
        .expect("show");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let trail = v["evidence_trail"].as_array().expect("evidence_trail");
    assert_eq!(trail.len(), 1, "malformed ULID should still be stored: {v}");
    assert_eq!(trail[0]["kind"], "recall", "kind=recall even for malformed ULID: {v}");
    assert_eq!(trail[0]["ref_val"], "NOT-A-VALID-ULID", "{v}");
}

// ── evidence AC 8 — migration adds evidence table idempotently ───────────────

#[test]
fn test_evidence_migration_idempotent() {
    let tmp = tempfile::tempdir().expect("tempdir");

    // Three separate process invocations — migration runs each time, must be idempotent.
    for run in &["r1", "r2", "r3"] {
        let st = docket(tmp.path())
            .args([
                "report", "--run", run, "--key", "k", "--title", "T",
                "--evidence", &format!("pid:{run}"),
            ])
            .status()
            .expect("report");
        assert!(st.success(), "idempotent migration run {run}: {:?}", st);
    }

    let out = docket(tmp.path())
        .args(["show", "k", "--format", "json"])
        .output()
        .expect("show");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let trail = v["evidence_trail"].as_array().expect("evidence_trail");
    assert_eq!(trail.len(), 3, "3 evidence rows after 3 reports: {v}");
}

// ── evidence AC 9 — JSON outputs are jq-parseable with new evidence fields ───

#[test]
fn test_evidence_json_valid_jq() {
    let tmp = tempfile::tempdir().expect("tempdir");

    docket(tmp.path())
        .args([
            "report", "--run", "r1", "--key", "k", "--title", "T",
            "--evidence", "recall:ULID123",
            "--evidence", "journal:2026-05-29#7",
        ])
        .status()
        .expect("report");

    // Validate both JSON outputs parse cleanly
    let out = docket(tmp.path())
        .args(["show", "k", "--format", "json"])
        .output()
        .expect("show json");
    let _: serde_json::Value = serde_json::from_slice(&out.stdout)
        .expect("show --format json must be valid JSON");

    let out = docket(tmp.path())
        .args(["list", "--format", "json"])
        .output()
        .expect("list json");
    let arr: serde_json::Value = serde_json::from_slice(&out.stdout)
        .expect("list --format json must be valid JSON");
    assert!(arr.is_array(), "list json is an array");
}

// ── evidence — chronological order (by run) ──────────────────────────────────

#[test]
fn test_evidence_chronological_by_run() {
    let tmp = tempfile::tempdir().expect("tempdir");

    // Report across three runs with one evidence ref each
    for (run, ev) in &[("r1", "pid:1"), ("r2", "pid:2"), ("r3", "pid:3")] {
        docket(tmp.path())
            .args(["report", "--run", run, "--key", "k", "--title", "T", "--evidence", ev])
            .status()
            .expect("report");
    }

    let out = docket(tmp.path())
        .args(["show", "k", "--format", "json"])
        .output()
        .expect("show");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let trail = v["evidence_trail"].as_array().expect("trail");
    assert_eq!(trail.len(), 3, "{v}");
    assert_eq!(trail[0]["run_id"], "r1", "first row is r1: {v}");
    assert_eq!(trail[1]["run_id"], "r2", "second row is r2: {v}");
    assert_eq!(trail[2]["run_id"], "r3", "third row is r3: {v}");
}

// ═══════════════════════════════════════════════════════════════════════════
// abide-ack-state acceptance criteria
// ═══════════════════════════════════════════════════════════════════════════

// ── ack AC 1 — migration is additive and idempotent ──────────────────────────

#[test]
fn ack_migration_idempotent() {
    let tmp = tempfile::tempdir().expect("tempdir");

    // Two separate process invocations — migration runs twice.
    for run in &["r1", "r2"] {
        let st = docket(tmp.path())
            .args(["report", "--run", run, "--key", "k", "--title", "T"])
            .status()
            .expect("report");
        assert!(st.success(), "migration idempotent run {run}: {:?}", st);
    }

    // Third invocation: list — must also succeed.
    let st = docket(tmp.path())
        .args(["list", "--format", "json"])
        .status()
        .expect("list");
    assert!(st.success(), "list after repeated opens: {:?}", st);
}

// ── ack AC 2 — docket ack sets ack columns; status/runs_seen unchanged ────────

#[test]
fn ack_sets_columns_status_unchanged() {
    let tmp = tempfile::tempdir().expect("tempdir");

    docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "k", "--title", "T"])
        .status()
        .expect("report");

    let st = docket(tmp.path())
        .args(["ack", "k", "--reason", "triaged", "--until-change", "pkgrel:5"])
        .status()
        .expect("ack");
    assert!(st.success(), "ack exit {:?}", st);

    let out = docket(tmp.path())
        .args(["show", "k", "--format", "json"])
        .output()
        .expect("show");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");

    assert!(!v["acknowledged_at"].is_null(), "acknowledged_at set: {v}");
    assert_eq!(v["ack_reason"], "triaged", "{v}");
    assert_eq!(v["ack_fingerprint"], "pkgrel:5", "{v}");
    // Status and runs_seen must be unchanged.
    assert_eq!(v["status"], "open", "status unchanged: {v}");
    assert_eq!(v["runs_seen"], 1, "runs_seen unchanged: {v}");
    assert_eq!(v["consecutive_runs"], 1, "consecutive_runs unchanged: {v}");
}

// ── ack AC 3 — ack errors on unknown or resolved key ─────────────────────────

#[test]
fn ack_errors_on_unknown_key() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = docket(tmp.path())
        .args(["ack", "no-such-key"])
        .output()
        .expect("ack unknown");
    assert!(!out.status.success(), "should fail for unknown key");
}

#[test]
fn ack_errors_on_resolved_key() {
    let tmp = tempfile::tempdir().expect("tempdir");

    docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "k", "--title", "T"])
        .status()
        .expect("report");
    docket(tmp.path())
        .args(["resolve", "k"])
        .status()
        .expect("resolve");

    let out = docket(tmp.path())
        .args(["ack", "k"])
        .output()
        .expect("ack resolved");
    assert!(!out.status.success(), "should fail for resolved key");
}

// ── ack AC 4 — unack clears columns; non-acked unack is no-op ────────────────

#[test]
fn unack_clears_columns() {
    let tmp = tempfile::tempdir().expect("tempdir");

    docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "k", "--title", "T"])
        .status()
        .expect("report");
    docket(tmp.path())
        .args(["ack", "k", "--reason", "R"])
        .status()
        .expect("ack");

    let st = docket(tmp.path())
        .args(["unack", "k"])
        .status()
        .expect("unack");
    assert!(st.success(), "unack exit {:?}", st);

    let out = docket(tmp.path())
        .args(["show", "k", "--format", "json"])
        .output()
        .expect("show");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert!(v["acknowledged_at"].is_null(), "acknowledged_at cleared: {v}");
    assert!(v["ack_reason"].is_null(), "ack_reason cleared: {v}");
    assert!(v["ack_fingerprint"].is_null(), "ack_fingerprint cleared: {v}");
    // Other fields must be intact.
    assert_eq!(v["status"], "open", "status intact: {v}");
    assert_eq!(v["runs_seen"], 1, "runs_seen intact: {v}");
}

#[test]
fn unack_noop_on_non_acked() {
    let tmp = tempfile::tempdir().expect("tempdir");

    docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "k", "--title", "T"])
        .status()
        .expect("report");

    // unack of a non-acked finding must exit 0
    let out = docket(tmp.path())
        .args(["unack", "k"])
        .output()
        .expect("unack");
    assert!(out.status.success(), "unack non-acked must exit 0: {:?}", out.status);
}

// ── ack AC 5 — same fingerprint keeps the ack ────────────────────────────────

#[test]
fn report_same_fingerprint_keeps_ack() {
    let tmp = tempfile::tempdir().expect("tempdir");

    docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "k", "--title", "T"])
        .status()
        .expect("report");
    docket(tmp.path())
        .args(["ack", "k", "--until-change", "pkgrel:5"])
        .status()
        .expect("ack");

    // Report with the SAME fingerprint — ack must persist.
    let st = docket(tmp.path())
        .args(["report", "--run", "r2", "--key", "k", "--title", "T", "--fingerprint", "pkgrel:5"])
        .status()
        .expect("report r2");
    assert!(st.success(), "report exit {:?}", st);

    let out = docket(tmp.path())
        .args(["show", "k", "--format", "json"])
        .output()
        .expect("show");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert!(!v["acknowledged_at"].is_null(), "ack kept with same fp: {v}");
}

// ── ack AC 6 — changed fingerprint clears the ack ────────────────────────────

#[test]
fn report_changed_fingerprint_clears_ack() {
    let tmp = tempfile::tempdir().expect("tempdir");

    docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "k", "--title", "T"])
        .status()
        .expect("report");
    docket(tmp.path())
        .args(["ack", "k", "--until-change", "pkgrel:5"])
        .status()
        .expect("ack");

    // Report with a DIFFERENT fingerprint — ack must be cleared.
    let st = docket(tmp.path())
        .args(["report", "--run", "r2", "--key", "k", "--title", "T", "--fingerprint", "pkgrel:11"])
        .status()
        .expect("report r2");
    assert!(st.success(), "report exit {:?}", st);

    let out = docket(tmp.path())
        .args(["show", "k", "--format", "json"])
        .output()
        .expect("show");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert!(v["acknowledged_at"].is_null(), "ack cleared on fp change: {v}");
    assert_eq!(v["runs_seen"], 2, "runs_seen still increments: {v}");
}

// ═══════════════════════════════════════════════════════════════════════════
// abide-digest-quiet acceptance criteria
// ═══════════════════════════════════════════════════════════════════════════

// ── digest-quiet AC 1 — acked findings excluded from default list ─────────────

#[test]
fn digest_quiet_list_excludes_acked() {
    let tmp = tempfile::tempdir().expect("tempdir");

    docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "open1", "--title", "Open"])
        .status()
        .expect("report open1");
    docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "acked1", "--title", "Acked"])
        .status()
        .expect("report acked1");
    docket(tmp.path())
        .args(["ack", "acked1"])
        .status()
        .expect("ack acked1");

    // Default list --open: should show only open1
    let out = docket(tmp.path())
        .args(["list", "--open", "--format", "json"])
        .output()
        .expect("list open");
    let arr: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let arr = arr.as_array().expect("array");
    assert!(arr.iter().any(|f| f["key"] == "open1"), "open1 in list: {arr:?}");
    assert!(!arr.iter().any(|f| f["key"] == "acked1"), "acked1 excluded by default: {arr:?}");

    // --include-acked: both visible
    let out = docket(tmp.path())
        .args(["list", "--open", "--include-acked", "--format", "json"])
        .output()
        .expect("list open --include-acked");
    let arr2: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let arr2 = arr2.as_array().expect("array");
    assert!(arr2.iter().any(|f| f["key"] == "open1"), "open1 visible with --include-acked: {arr2:?}");
    assert!(arr2.iter().any(|f| f["key"] == "acked1"), "acked1 visible with --include-acked: {arr2:?}");
}

// ── digest-quiet AC 2 — digest excludes acked from open/escalated counts ──────

#[test]
fn digest_quiet_excludes_acked_from_counts() {
    let tmp = tempfile::tempdir().expect("tempdir");

    docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "open1", "--title", "Open"])
        .status()
        .expect("report open1");
    docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "acked1", "--title", "Acked"])
        .status()
        .expect("report acked1");
    docket(tmp.path())
        .args(["ack", "acked1"])
        .status()
        .expect("ack acked1");

    let out = docket(tmp.path())
        .args(["digest", "--format", "json"])
        .output()
        .expect("digest json");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(v["detail"]["open"], 1, "only 1 non-acked open: {v}");
    assert_eq!(v["detail"]["escalated"], 0, "0 escalated: {v}");
}

// ── digest-quiet AC 3 — DigestDetail.acked count correct ─────────────────────

#[test]
fn digest_quiet_acked_count_correct() {
    let tmp = tempfile::tempdir().expect("tempdir");

    docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "open1", "--title", "Open"])
        .status()
        .expect("report");
    docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "acked1", "--title", "Acked"])
        .status()
        .expect("report");
    docket(tmp.path())
        .args(["ack", "acked1"])
        .status()
        .expect("ack");

    let out = docket(tmp.path())
        .args(["digest", "--format", "json"])
        .output()
        .expect("digest json");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(v["detail"]["acked"], 1, "acked count == 1: {v}");
}

// ── digest-quiet AC 4 — summary includes acked clause when non-zero ───────────

#[test]
fn digest_quiet_summary_includes_acked_clause() {
    let tmp = tempfile::tempdir().expect("tempdir");

    docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "open1", "--title", "Open"])
        .status()
        .expect("report");
    docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "acked1", "--title", "Acked"])
        .status()
        .expect("report");
    docket(tmp.path())
        .args(["ack", "acked1"])
        .status()
        .expect("ack");

    let out = docket(tmp.path())
        .args(["digest", "--format", "json"])
        .output()
        .expect("digest json");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let summary = v["summary"].as_str().unwrap_or("");
    assert!(summary.contains("1 acked"), "summary should contain '1 acked': {v}");
}

// ── digest-quiet AC 5 — acked escalated finding → HealthStatus::Ok ───────────

#[test]
fn digest_quiet_acked_escalated_is_ok() {
    let tmp = tempfile::tempdir().expect("tempdir");

    // Escalate a finding (3 runs).
    for r in 1..=3_i32 {
        docket(tmp.path())
            .args(["report", "--run", &format!("r{r}"), "--key", "esc", "--title", "E"])
            .status()
            .expect("report esc");
    }

    // Ack it.
    docket(tmp.path())
        .args(["ack", "esc"])
        .status()
        .expect("ack esc");

    let out = docket(tmp.path())
        .args(["digest", "--format", "json"])
        .output()
        .expect("digest json");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(v["status"], "ok", "acked escalated should be ok: {v}");
}

// ── digest-quiet AC 6 — --include-acked restores prior behaviour ──────────────

#[test]
fn digest_quiet_include_acked_restores_counts() {
    let tmp = tempfile::tempdir().expect("tempdir");

    for r in 1..=3_i32 {
        docket(tmp.path())
            .args(["report", "--run", &format!("r{r}"), "--key", "esc", "--title", "E"])
            .status()
            .expect("report esc");
    }
    docket(tmp.path())
        .args(["ack", "esc"])
        .status()
        .expect("ack esc");

    // --include-acked: should show status=degraded (acked finding counted)
    let out = docket(tmp.path())
        .args(["digest", "--format", "json", "--include-acked"])
        .output()
        .expect("digest json --include-acked");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(v["status"], "degraded", "--include-acked restores degraded: {v}");
    assert_eq!(v["detail"]["escalated"], 1, "--include-acked counts escalated: {v}");
}

// ── digest-quiet AC 7 — existing tests green with zero acked ─────────────────

#[test]
fn digest_quiet_zero_acked_backward_compat_summary() {
    let tmp = tempfile::tempdir().expect("tempdir");

    docket(tmp.path())
        .args(["report", "--run", "r1", "--key", "k", "--title", "T"])
        .status()
        .expect("report");

    let out = docket(tmp.path())
        .args(["digest", "--format", "json"])
        .output()
        .expect("digest json");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let summary = v["summary"].as_str().unwrap_or("");
    // With zero acked, summary must NOT have ", 0 acked"
    assert!(!summary.contains("acked"), "zero-acked summary must not mention acked: {v}");
    assert_eq!(summary, "1 open", "zero-acked summary byte-identical: {v}");
}

// ── litmus-stuck-detector ACs ─────────────────────────────────────────────────

/// Seed a finding with many reports across many runs. Returns the tmp dir.
fn seed_stuck_finding(tmp: &std::path::Path, key: &str, runs: u32, reports_per_run: u32) {
    for r in 0..runs {
        let run_id = format!("r{r}");
        for _ in 0..reports_per_run {
            docket(tmp)
                .args([
                    "report",
                    "--run",
                    &run_id,
                    "--key",
                    key,
                    "--title",
                    "Probe never clears",
                ])
                .status()
                .expect("report");
        }
    }
}

// AC 1 — basic stuck listing: open findings with report_count>=6 AND runs_seen>=5
#[test]
fn stuck_ac1_lists_qualifying_open_findings() {
    let tmp = tempfile::tempdir().expect("tempdir");
    // Seed: 12 reports across 10 runs (qualifies for default thresholds 6/5)
    seed_stuck_finding(tmp.path(), "ctrace-phantom", 10, 1);
    // Add extra reports on run r0 to push report_count to 12
    for _ in 0..2 {
        docket(tmp.path())
            .args(["report", "--run", "r0", "--key", "ctrace-phantom", "--title", "Probe never clears"])
            .status()
            .expect("extra report");
    }

    let out = docket(tmp.path())
        .args(["stuck"])
        .output()
        .expect("docket stuck");
    assert!(out.status.success(), "exit {:?}\nstderr: {}", out.status, String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("ctrace-phantom"), "should list ctrace-phantom: {stdout}");
    assert!(!stdout.contains("no probe-suspect findings"), "should not be empty: {stdout}");
}

// AC 2 — end-to-end: appears in stuck, disappears after resolve
#[test]
fn stuck_ac2_disappears_after_resolve() {
    let tmp = tempfile::tempdir().expect("tempdir");
    seed_stuck_finding(tmp.path(), "ctrace-like", 10, 1);
    // add 2 more to hit report_count=12
    for _ in 0..2 {
        docket(tmp.path())
            .args(["report", "--run", "r0", "--key", "ctrace-like", "--title", "Probe never clears"])
            .status()
            .expect("extra report");
    }

    // Verify appears
    let out = docket(tmp.path())
        .args(["stuck"])
        .output()
        .expect("stuck before resolve");
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("ctrace-like"));

    // Resolve
    docket(tmp.path())
        .args(["resolve", "ctrace-like"])
        .status()
        .expect("resolve");

    // Verify disappears
    let out2 = docket(tmp.path())
        .args(["stuck"])
        .output()
        .expect("stuck after resolve");
    assert!(out2.status.success());
    let stdout2 = String::from_utf8_lossy(&out2.stdout);
    assert!(!stdout2.contains("ctrace-like"), "resolved finding should not appear: {stdout2}");
    assert!(stdout2.contains("no probe-suspect findings"), "should be empty after resolve: {stdout2}");
}

// AC 3 — --min-reports / --min-runs flags change the cutoff
#[test]
fn stuck_ac3_cutoff_flags() {
    let tmp = tempfile::tempdir().expect("tempdir");
    // 4 runs, 1 report each → report_count=4, runs_seen=4
    seed_stuck_finding(tmp.path(), "borderline", 4, 1);

    // default thresholds (6/5) → should NOT appear
    let out = docket(tmp.path())
        .args(["stuck"])
        .output()
        .expect("stuck default");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains("borderline"), "should be below default threshold: {stdout}");

    // lower thresholds → should appear
    let out2 = docket(tmp.path())
        .args(["stuck", "--min-reports", "3", "--min-runs", "3"])
        .output()
        .expect("stuck lower threshold");
    assert!(out2.status.success());
    let stdout2 = String::from_utf8_lossy(&out2.stdout);
    assert!(stdout2.contains("borderline"), "should appear at lower threshold: {stdout2}");
}

// AC 4 — --format json emits a valid JSON array with all documented fields
#[test]
fn stuck_ac4_json_output() {
    let tmp = tempfile::tempdir().expect("tempdir");
    seed_stuck_finding(tmp.path(), "probe-json", 10, 1);
    // push report_count to 12
    for _ in 0..2 {
        docket(tmp.path())
            .args(["report", "--run", "r0", "--key", "probe-json", "--title", "JSON probe"])
            .status()
            .expect("extra report");
    }

    let out = docket(tmp.path())
        .args(["stuck", "--format", "json"])
        .output()
        .expect("stuck --format json");
    assert!(out.status.success(), "exit {:?}\nstderr: {}", out.status, String::from_utf8_lossy(&out.stderr));

    let arr: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");
    assert!(arr.is_array(), "should be JSON array");
    let items = arr.as_array().expect("array");
    assert!(!items.is_empty(), "array should be non-empty");

    let item = &items[0];
    for field in &["key", "title", "report_count", "runs_seen", "consecutive_runs", "first_seen", "last_seen", "suspect_reason"] {
        assert!(item.get(field).is_some(), "JSON missing field '{field}': {item}");
    }
    assert!(item["suspect_reason"].as_str().unwrap_or("").contains("never resolved"),
        "suspect_reason should mention 'never resolved': {item}");
}

// AC 5 — acked findings excluded by default, included with --include-acked
#[test]
fn stuck_ac5_acked_exclusion() {
    let tmp = tempfile::tempdir().expect("tempdir");
    seed_stuck_finding(tmp.path(), "acked-probe", 10, 1);
    // push report_count to 12
    for _ in 0..2 {
        docket(tmp.path())
            .args(["report", "--run", "r0", "--key", "acked-probe", "--title", "Acked probe"])
            .status()
            .expect("extra report");
    }

    // Ack it
    docket(tmp.path())
        .args(["ack", "acked-probe"])
        .status()
        .expect("ack");

    // Default: should be excluded
    let out = docket(tmp.path())
        .args(["stuck"])
        .output()
        .expect("stuck default");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains("acked-probe"), "acked finding should be excluded by default: {stdout}");

    // --include-acked: should appear
    let out2 = docket(tmp.path())
        .args(["stuck", "--include-acked"])
        .output()
        .expect("stuck --include-acked");
    assert!(out2.status.success());
    let stdout2 = String::from_utf8_lossy(&out2.stdout);
    assert!(stdout2.contains("acked-probe"), "acked finding should appear with --include-acked: {stdout2}");
}

// AC 6 — no db writes: byte-identical db before and after stuck
#[test]
fn stuck_ac6_no_db_writes() {
    let tmp = tempfile::tempdir().expect("tempdir");
    seed_stuck_finding(tmp.path(), "readonly-probe", 10, 1);

    let db_path = tmp.path().join("docket").join("docket.db");
    let before = std::fs::read(&db_path).expect("read db before");

    // Run stuck (read-only)
    let out = docket(tmp.path())
        .args(["stuck"])
        .output()
        .expect("stuck");
    assert!(out.status.success());

    let after = std::fs::read(&db_path).expect("read db after");
    assert_eq!(before, after, "docket stuck must not write to the database");
}

// AC 7 — help includes 'stuck' subcommand
#[test]
fn stuck_ac7_help_lists_stuck() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = docket(tmp.path())
        .arg("--help")
        .output()
        .expect("docket --help");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("stuck"), "help output missing 'stuck' subcommand: {stdout}");
}
