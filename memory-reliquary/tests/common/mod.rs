//! Shared test helpers for the acceptance suite.
//!
//! Builds a fake recall-root and a stub `recall` command so each acceptance
//! test can exercise the binary end-to-end without touching `$HOME`.

#![allow(dead_code, unreachable_pub, clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use serde_json::json;

/// A single fake memory: id, frontmatter map, body Markdown.
pub struct FakeMemory {
    pub id: &'static str,
    pub kind: &'static str,
    pub subject: &'static str,
    pub created_at: &'static str,
    pub recall_count: u64,
    pub last_recalled_at: Option<&'static str>,
    pub private: bool,
    pub body: &'static str,
}

impl FakeMemory {
    pub fn frontmatter(&self) -> String {
        let mut s = String::from("---\n");
        s.push_str(&format!("id: {}\n", self.id));
        s.push_str(&format!("kind: {}\n", self.kind));
        s.push_str(&format!("subject: {}\n", self.subject));
        s.push_str(&format!("created_at: {}\n", self.created_at));
        s.push_str(&format!("recall_count: {}\n", self.recall_count));
        if let Some(lr) = self.last_recalled_at {
            s.push_str(&format!("last_recalled_at: {lr}\n"));
        }
        if self.private {
            s.push_str("private: true\n");
        }
        s.push_str("---\n");
        s
    }
}

/// Stages a recall-root with one Markdown file per `FakeMemory` and a
/// stub `recall` shell script that prints the corresponding JSON array.
///
/// Returns `(recall_root, recall_cmd_path)`.
pub fn stage_recall_root(dir: &Path, memories: &[FakeMemory]) -> (PathBuf, PathBuf) {
    let root = dir.join("recall");
    let user_dir = root.join("memories").join("user");
    fs::create_dir_all(&user_dir).unwrap();

    // Write a body file per memory.
    let mut json_entries = Vec::new();
    for m in memories {
        // Place all memories under memories/<subject>/<id>.md regardless of
        // the subject prefix variation, so the path field is well-defined.
        let subject_dir = root.join("memories").join(m.subject);
        fs::create_dir_all(&subject_dir).unwrap();
        let path = subject_dir.join(format!("{}.md", m.id));
        let mut content = m.frontmatter();
        content.push('\n');
        content.push_str(m.body);
        fs::write(&path, content).unwrap();

        json_entries.push(json!({
            "id": m.id,
            "kind": m.kind,
            "subject": m.subject,
            "path": path.to_string_lossy(),
            "recall_count": m.recall_count,
            "last_recalled_at": m.last_recalled_at,
            "confidence": 0.5,
        }));
    }
    let json = serde_json::to_string(&json_entries).unwrap();
    let json_path = dir.join("recall.json");
    fs::write(&json_path, &json).unwrap();

    // Stub `recall` script: ignore args, print the JSON file.
    let script = dir.join("recall_stub.sh");
    let body = format!("#!/bin/sh\ncat {}\n", json_path.to_string_lossy());
    fs::write(&script, body).unwrap();
    let mut perms = fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script, perms).unwrap();
    (root, script)
}

/// Write a raw memory file (used by AC6 to inject malformed YAML).
pub fn write_raw_memory(dir: &Path, subject: &str, id: &str, body: &str) -> PathBuf {
    let subject_dir = dir.join("recall").join("memories").join(subject);
    fs::create_dir_all(&subject_dir).unwrap();
    let path = subject_dir.join(format!("{id}.md"));
    fs::write(&path, body).unwrap();
    path
}
