//! Walk a single file's git history and emit structured per-commit
//! records suitable for downstream rendering passes.
//!
//! The algorithm:
//!
//! 1. Open the repository (rejecting non-repos with [`Error::NotARepository`]).
//! 2. Resolve the initial path to track. If the file does not exist at
//!    HEAD but did exist historically, record `deleted_at` (the
//!    committer time of the most recent commit that touched it under
//!    that name) and continue walking.
//! 3. Walk reachable history from HEAD in time order. For each commit
//!    that touches the currently-tracked path (against its first parent
//!    — root commits are diffed against an empty tree), render the
//!    unified diff hunk, strip the `diff --git` / `index` / `---` /
//!    `+++` header lines, and produce a [`CommitEntry`].
//! 4. After computing the diff, detect renames via
//!    [`git2::Diff::find_similar`] with `renames(true)`; if the
//!    currently-tracked path is the *new* side of a rename, swap to the
//!    old side so subsequent (older) commits track the file under its
//!    historical name.
//! 5. Reverse the accumulated list so the output is oldest-first.
//! 6. Optionally, with `collapse_trivial`, fold runs of commits whose
//!    diff body (after stripping headers, whitespace, and `+`/`-` lines
//!    that contain only whitespace) is empty into a single synthetic
//!    `{hash: "trivial", subject: "N trivial edits", ...}` entry.
//! 7. Apply the privacy clause: any commit whose message body contains
//!    a trailer line `private: true` at column 0 has its `diff` field
//!    replaced with the literal string `[redacted]`.

use std::path::{Path, PathBuf};

use chrono::{DateTime, TimeZone, Utc};
use git2::{
    Commit, Diff, DiffFindOptions, DiffFormat, DiffOptions, Repository, Sort, Tree,
};
// `Diff` is retained as a name for `render_patch_for_delta`'s signature.
use serde::{Deserialize, Serialize};

use crate::error::Error;

/// Options controlling extractor behaviour.
#[derive(Debug, Clone, Default)]
pub struct ExtractOptions {
    /// When true, runs of commits with empty (post-strip) diff bodies
    /// are collapsed into a single synthetic `trivial` entry. Default
    /// is false (AC3: the extractor does not editorialize).
    pub collapse_trivial: bool,
}

/// Top-level output structure serialised to JSON on stdout.
#[derive(Debug, Serialize, Deserialize)]
pub struct Extract {
    /// Repo-relative path requested by the caller.
    pub file: String,
    /// Absolute path to the repository's working directory (or git dir
    /// for bare repos).
    pub repo: String,
    /// Time at which the extract was performed (RFC3339, UTC).
    pub generated_at: DateTime<Utc>,
    /// If the file does not exist at HEAD but did historically, the
    /// committer time of the most recent commit that touched it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<DateTime<Utc>>,
    /// Per-commit records, oldest first.
    pub commits: Vec<CommitEntry>,
}

/// One commit that touched the tracked file.
#[derive(Debug, Serialize, Deserialize)]
pub struct CommitEntry {
    /// Full commit SHA, or the literal `"trivial"` for a collapsed
    /// synthetic entry produced by `--collapse-trivial`.
    pub hash: String,
    /// First 7 chars of `hash`. For a synthetic entry this is also
    /// `"trivial"`.
    pub short_hash: String,
    /// Author display name (`<unknown>` if unset).
    pub author_name: String,
    /// Author email (`<unknown>` if unset).
    pub author_email: String,
    /// Committer time (RFC3339, UTC).
    pub committed_at: DateTime<Utc>,
    /// First line of the commit message.
    pub subject: String,
    /// Everything after the subject line of the commit message
    /// (trimmed of leading blank lines).
    pub body: String,
    /// Unified diff hunk for the tracked file across this commit,
    /// with the `diff --git` / `index` / `---` / `+++` header lines
    /// stripped. `[redacted]` for commits whose body contains a
    /// `private: true` trailer.
    pub diff: String,
}

/// Walk the history of `file` under `repo_path` and return a fully
/// populated [`Extract`].
///
/// Returns [`Error::NotARepository`] if `repo_path` is not a git
/// repo, or [`Error::NoCommitsFound`] if the file never existed
/// under any name in the reachable history of HEAD.
pub fn extract(
    repo_path: &Path,
    file: &str,
    opts: &ExtractOptions,
) -> Result<Extract, Error> {
    let repo = Repository::open(repo_path).map_err(|_| Error::NotARepository {
        path: repo_path.to_path_buf(),
    })?;

    let abs_repo = repo
        .workdir()
        .unwrap_or_else(|| repo.path())
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());

    // Walk history newest-first, tracking the current name of the file.
    let mut current_path = PathBuf::from(file);
    let mut newest_first: Vec<CommitEntry> = Vec::new();
    let mut last_touched_secs: Option<i64> = None;

    let mut revwalk = repo.revwalk()?;
    revwalk.set_sorting(Sort::TIME)?;
    let head_oid = repo
        .head()
        .map_err(|e| Error::HeadUnresolved {
            path: repo_path.to_path_buf(),
            source: e,
        })?
        .target()
        .ok_or_else(|| Error::HeadUnresolved {
            path: repo_path.to_path_buf(),
            source: git2::Error::from_str("HEAD has no target oid"),
        })?;
    revwalk.push(head_oid)?;

    for oid_res in revwalk {
        let oid = oid_res?;
        let commit = repo.find_commit(oid)?;

        let (touched, raw_patch, new_path) =
            commit_diff_for_path(&repo, &commit, &current_path)?;

        if !touched {
            continue;
        }

        let patch = strip_diff_headers(&raw_patch);
        let entry = build_commit_entry(&commit, patch);
        if let Some(secs) = last_touched_secs {
            if commit.committer().when().seconds() > secs {
                last_touched_secs = Some(commit.committer().when().seconds());
            }
        } else {
            last_touched_secs = Some(commit.committer().when().seconds());
        }
        newest_first.push(entry);

        if let Some(old) = new_path {
            current_path = old;
        }
    }

    if newest_first.is_empty() {
        return Err(Error::NoCommitsFound {
            path: PathBuf::from(file),
        });
    }

    // Oldest first.
    newest_first.reverse();
    let mut commits = newest_first;

    if opts.collapse_trivial {
        commits = collapse_trivial_runs(commits);
    }

    // Privacy redaction (after collapse so collapsed synthetic entries,
    // which have no commit body, are unaffected).
    for c in &mut commits {
        if has_private_trailer(&c.body) {
            c.diff = "[redacted]".to_string();
        }
    }

    // Determine deleted_at: file is "deleted" if not present at HEAD.
    let deleted_at = if file_exists_at_head(&repo, file)? {
        None
    } else {
        last_touched_secs.and_then(|s| Utc.timestamp_opt(s, 0).single())
    };

    Ok(Extract {
        file: file.to_string(),
        repo: abs_repo.to_string_lossy().into_owned(),
        generated_at: Utc::now(),
        deleted_at,
        commits,
    })
}

/// Diff `commit` against its first parent (empty tree for root) with
/// rename detection enabled, then look for a delta whose *new* side is
/// `path`. Returns `(touched, patch_text, renamed_from)`.
///
/// When the delta is a rename whose new side is `path`, `renamed_from`
/// is the old side — the caller switches its tracked path to that for
/// older commits.
fn commit_diff_for_path(
    repo: &Repository,
    commit: &Commit<'_>,
    path: &Path,
) -> Result<(bool, String, Option<PathBuf>), Error> {
    let new_tree = commit.tree()?;
    let parent_tree: Option<Tree<'_>> = if commit.parent_count() == 0 {
        None
    } else {
        Some(commit.parent(0)?.tree()?)
    };

    // Always do the full diff with rename detection so we can follow
    // the file across renames. Pathspec filtering would miss the case
    // where the file appears as a "new" delta but was actually renamed
    // from a different historical name.
    let mut full_opts = DiffOptions::new();
    full_opts.include_typechange(true);
    let mut full_diff = repo.diff_tree_to_tree(
        parent_tree.as_ref(),
        Some(&new_tree),
        Some(&mut full_opts),
    )?;
    let mut find_opts = DiffFindOptions::new();
    find_opts.renames(true);
    find_opts.copies(false);
    // `force` makes rename detection run even when the same file
    // appears as an add+delete pair (which is the common case after
    // `git mv` without --renames in the porcelain log).
    find_opts.renames_from_rewrites(true);
    find_opts.rewrites(true);
    full_diff.find_similar(Some(&mut find_opts))?;

    // Scan for a delta whose new path == `path`. The hash semantics:
    // - new path == path, old path == path → modify (no rename)
    // - new path == path, old path != path → rename (return old path)
    // - new path == path, old path == None → add (created here)
    let path_str = path.to_string_lossy().into_owned();
    let mut rename_old: Option<PathBuf> = None;
    let mut target_idx: Option<usize> = None;
    for (idx, delta) in full_diff.deltas().enumerate() {
        let new_p = delta.new_file().path().map(Path::to_path_buf);
        let old_p = delta.old_file().path().map(Path::to_path_buf);
        if let Some(np) = &new_p {
            if np.to_string_lossy() == path_str {
                match &old_p {
                    Some(op) if op != np => {
                        rename_old = Some(op.clone());
                    }
                    _ => {}
                }
                target_idx = Some(idx);
                break;
            }
        }
    }

    if let Some(idx) = target_idx {
        let rendered = render_patch_for_delta(&full_diff, idx)?;
        return Ok((true, rendered, rename_old));
    }

    Ok((false, String::new(), None))
}

/// Render only the lines belonging to the delta at `target_idx`.
fn render_patch_for_delta(diff: &Diff<'_>, target_idx: usize) -> Result<String, Error> {
    let mut out = String::new();
    // libgit2's print walks deltas in order; we filter by the delta
    // index using `delta.nfiles()` isn't possible since it doesn't
    // expose ordinal — instead identify the delta via its new/old path
    // equality with the target delta.
    let target = diff.get_delta(target_idx);
    let target_new = target.as_ref().and_then(|d| d.new_file().path().map(Path::to_path_buf));
    let target_old = target.as_ref().and_then(|d| d.old_file().path().map(Path::to_path_buf));
    diff.print(DiffFormat::Patch, |delta, _hunk, line| {
        let new_p = delta.new_file().path().map(Path::to_path_buf);
        let old_p = delta.old_file().path().map(Path::to_path_buf);
        if new_p == target_new && old_p == target_old {
            push_line(&mut out, &line);
        }
        true
    })?;
    Ok(out)
}

/// Append a [`git2::DiffLine`]'s textual content to `out`, prefixing
/// added/removed/context lines with their origin marker.
fn push_line(out: &mut String, line: &git2::DiffLine<'_>) {
    let origin = line.origin();
    // For file/hunk/binary markers libgit2 emits origin 'F', 'H', 'B';
    // for context/added/removed it's ' '/'+'/'-'. The content already
    // includes the marker for header lines (F), so don't double up.
    match origin {
        '+' | '-' | ' ' => {
            out.push(origin);
        }
        _ => {}
    }
    let content = std::str::from_utf8(line.content()).unwrap_or("<binary>");
    out.push_str(content);
}

/// Strip the four header lines libgit2 emits for each file delta:
/// `diff --git ...`, `index ...`, `--- a/...`, `+++ b/...`. Also keeps
/// any rename-specific headers (`similarity index`, `rename from`,
/// `rename to`) since those are evidence of the rename hunk that the
/// AC requires.
fn strip_diff_headers(patch: &str) -> String {
    let mut out = String::new();
    for line in patch.split_inclusive('\n') {
        let trimmed = line.trim_start_matches(['+', '-', ' ']);
        // The header lines libgit2 emits have no leading +/-/space
        // (origin == 'F'), so `line` and `trimmed` are equal for them.
        let raw = line;
        if raw.starts_with("diff --git ")
            || raw.starts_with("index ")
            || raw.starts_with("--- a/")
            || raw.starts_with("--- /dev/null")
            || raw.starts_with("+++ b/")
            || raw.starts_with("+++ /dev/null")
        {
            // Drop the file header lines.
            let _ = trimmed; // keep `trimmed` referenced for clippy
            continue;
        }
        out.push_str(line);
    }
    out
}

/// Construct a [`CommitEntry`] from a libgit2 [`Commit`] and a
/// pre-stripped diff string.
fn build_commit_entry(commit: &Commit<'_>, diff: String) -> CommitEntry {
    let hash = commit.id().to_string();
    let short_hash: String = hash.chars().take(7).collect();
    let author = commit.author();
    let author_name = author.name().unwrap_or("<unknown>").to_string();
    let author_email = author.email().unwrap_or("<unknown>").to_string();
    let when = commit.committer().when().seconds();
    let committed_at = Utc.timestamp_opt(when, 0).single().unwrap_or_else(Utc::now);
    let message = commit.message().unwrap_or("");
    let (subject, body) = split_subject_body(message);
    CommitEntry {
        hash,
        short_hash,
        author_name,
        author_email,
        committed_at,
        subject,
        body,
        diff,
    }
}

/// Split a commit message into `(subject, body)`. The subject is the
/// first line; the body is everything after, with leading blank lines
/// trimmed.
fn split_subject_body(msg: &str) -> (String, String) {
    let mut lines = msg.split('\n');
    let subject = lines.next().unwrap_or("").trim_end().to_string();
    let rest: String = lines.collect::<Vec<&str>>().join("\n");
    let body = rest.trim_start_matches('\n').to_string();
    (subject, body)
}

/// True if any line of `body` is exactly `private: true` at column 0.
fn has_private_trailer(body: &str) -> bool {
    body.lines().any(|l| l == "private: true")
}

/// Does the file exist in the HEAD tree?
fn file_exists_at_head(repo: &Repository, file: &str) -> Result<bool, Error> {
    let head = repo.head()?;
    let tree = head.peel_to_commit()?.tree()?;
    Ok(tree.get_path(Path::new(file)).is_ok())
}

/// Fold runs of commits whose diff body, after stripping headers,
/// whitespace, and lines that contain only whitespace after a `+`/`-`
/// marker, are empty into single synthetic `trivial` entries.
fn collapse_trivial_runs(commits: Vec<CommitEntry>) -> Vec<CommitEntry> {
    let mut out: Vec<CommitEntry> = Vec::new();
    let mut run: Vec<CommitEntry> = Vec::new();

    for c in commits {
        if is_trivial_diff(&c.diff) {
            run.push(c);
        } else {
            flush_run(&mut run, &mut out);
            out.push(c);
        }
    }
    flush_run(&mut run, &mut out);
    out
}

/// Flush the accumulated trivial run into a single synthetic entry
/// appended to `out`.
fn flush_run(run: &mut Vec<CommitEntry>, out: &mut Vec<CommitEntry>) {
    if run.is_empty() {
        return;
    }
    let n = run.len();
    // Use the latest committer time of the run for the synthetic
    // entry's `committed_at`.
    let when = run
        .iter()
        .map(|c| c.committed_at)
        .max()
        .unwrap_or_else(Utc::now);
    out.push(CommitEntry {
        hash: "trivial".to_string(),
        short_hash: "trivial".to_string(),
        author_name: "<various>".to_string(),
        author_email: "<various>".to_string(),
        committed_at: when,
        subject: format!("{n} trivial edits"),
        body: String::new(),
        diff: String::new(),
    });
    run.clear();
}

/// A diff is "trivial" if, after dropping `+`/`-`-prefixed lines that
/// contain only whitespace and hunk headers, no `+`/`-` lines remain.
fn is_trivial_diff(diff: &str) -> bool {
    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix('+') {
            if !rest.trim().is_empty() && !line.starts_with("+++") {
                return false;
            }
        } else if let Some(rest) = line.strip_prefix('-') {
            if !rest.trim().is_empty() && !line.starts_with("---") {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_trailer_detected() {
        assert!(has_private_trailer("foo\nprivate: true\n"));
        assert!(has_private_trailer("private: true"));
        assert!(!has_private_trailer("  private: true"));
        assert!(!has_private_trailer("private:true"));
    }

    #[test]
    fn split_subject_and_body() {
        let (s, b) = split_subject_body("first\n\nrest line 1\nrest line 2\n");
        assert_eq!(s, "first");
        assert_eq!(b, "rest line 1\nrest line 2\n");
    }

    #[test]
    fn trivial_diff_classification() {
        assert!(is_trivial_diff(""));
        assert!(is_trivial_diff("@@ -1 +1 @@\n+   \n-\t\n"));
        assert!(!is_trivial_diff("@@ -1 +1 @@\n+hello\n"));
    }

    #[test]
    fn strip_headers_drops_file_header_lines() {
        let raw = "diff --git a/x b/x\nindex abc..def 100644\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-old\n+new\n";
        let stripped = strip_diff_headers(raw);
        assert!(!stripped.contains("diff --git"));
        assert!(!stripped.contains("index "));
        assert!(!stripped.contains("--- a/"));
        assert!(!stripped.contains("+++ b/"));
        assert!(stripped.contains("@@ -1 +1 @@"));
        assert!(stripped.contains("+new"));
    }
}
