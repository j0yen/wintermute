//! Walk a git repository's HEAD tree and aggregate per-path topographic
//! primitives across reachable history.
//!
//! The algorithm:
//!
//! 1. Open the repository (rejecting non-repos with [`Error::NotARepository`]).
//! 2. Snapshot the set of tracked paths under HEAD plus their blob sizes
//!    — this is what gets emitted, so untracked / gitignored files are
//!    naturally excluded.
//! 3. Walk reachable history from HEAD with a time-sorted revwalk; for
//!    every commit diff against its first parent (root commit diffed
//!    against an empty tree), recording, per affected path, a (timestamp,
//!    author) tuple.
//! 4. Project the per-path tallies onto the HEAD path set, fold author
//!    counts to pick a primary author, take the max timestamp as
//!    `last_touched`, and emit the [`Landscape`].
//!
//! The optional `--coalesce-below <N>` post-processing pass collapses
//! directories with fewer than N tracked files into a synthetic entry
//! whose `language = "directory"` and whose `commit_count` is the sum of
//! its children. The collapsed entry's `size_bytes` is summed and its
//! `last_touched` / `primary_author` come from the most recent child
//! commit.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use chrono::{DateTime, TimeZone, Utc};
use git2::{Commit, DiffOptions, Repository, Sort, Tree};
use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::language::language_for_path;

/// Options controlling walk behaviour and post-processing.
#[derive(Debug, Clone, Default)]
pub struct WalkOptions {
    /// If `Some(n)` and `n > 0`, directories containing fewer than `n`
    /// tracked files are coalesced into a single synthetic entry. See
    /// the module-level docs.
    pub coalesce_below: Option<u32>,
}

/// Top-level output structure (serialised to JSON on stdout).
#[derive(Debug, Serialize, Deserialize)]
pub struct Landscape {
    /// Absolute path to the repository's working directory (or git dir
    /// for bare repos).
    pub repo: String,
    /// Time at which the walk was performed (RFC3339, UTC).
    pub generated_at: DateTime<Utc>,
    /// Per-file topographic primitives.
    pub files: Vec<FileEntry>,
}

/// One row of the landscape — a single tracked file (or a coalesced
/// directory entry, distinguished by `language == "directory"`).
#[derive(Debug, Serialize, Deserialize)]
pub struct FileEntry {
    /// Repository-relative path. Coalesced directories use `"<dir>/."`.
    pub path: String,
    /// Language label from extension; `"directory"` for coalesced rows.
    pub language: String,
    /// Number of commits in HEAD's reachable history that touched this
    /// path (or, for a coalesced directory, the sum across children).
    pub commit_count: u32,
    /// Committer date of the most recent commit touching the path.
    pub last_touched: DateTime<Utc>,
    /// Author whose commits on the path outnumber any other's. Ties
    /// break by most recent commit.
    pub primary_author: String,
    /// On-disk size of the HEAD blob, in bytes. For coalesced
    /// directories, the sum across child blobs.
    pub size_bytes: u64,
}

/// Per-path accumulator built during the history walk.
#[derive(Debug, Default)]
struct PerPathStats {
    commit_count: u32,
    last_touched_secs: i64,
    /// Author name → (count, most-recent-touch-secs). Most-recent
    /// breaks ties when picking the primary author.
    authors: HashMap<String, (u32, i64)>,
}

/// Walk `repo_path` and return a fully-populated [`Landscape`].
///
/// Returns [`Error::NotARepository`] if `repo_path` is not a git
/// working directory (or bare repo).
pub fn walk_repo(repo_path: &Path, opts: &WalkOptions) -> Result<Landscape, Error> {
    let repo = Repository::open(repo_path).map_err(|_| Error::NotARepository {
        path: repo_path.to_path_buf(),
    })?;

    // Absolute repo path: prefer workdir, fall back to git_dir for bare.
    let abs = repo
        .workdir()
        .unwrap_or_else(|| repo.path())
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());

    // --- Step 1: HEAD tree path/size snapshot. -----------------------
    let head_paths = collect_head_paths(&repo, repo_path)?;

    // --- Step 2: walk history, per-path tally. -----------------------
    let stats = collect_history_stats(&repo, repo_path, &head_paths)?;

    // --- Step 3: project onto HEAD paths. ----------------------------
    let mut files: Vec<FileEntry> = head_paths
        .into_iter()
        .map(|(path, size)| build_entry(path, size, &stats))
        .collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));

    // --- Step 4: optional coalesce pass. -----------------------------
    if let Some(threshold) = opts.coalesce_below {
        if threshold > 0 {
            files = coalesce_directories(files, threshold);
        }
    }

    Ok(Landscape {
        repo: abs.to_string_lossy().into_owned(),
        generated_at: Utc::now(),
        files,
    })
}

/// Walk HEAD's tree, returning every tracked file's repo-relative path
/// and its blob size in bytes.
fn collect_head_paths(
    repo: &Repository,
    repo_path: &Path,
) -> Result<Vec<(String, u64)>, Error> {
    let head = repo
        .head()
        .map_err(|e| Error::HeadUnresolved {
            path: repo_path.to_path_buf(),
            source: e,
        })?;
    let head_commit = head.peel_to_commit().map_err(|e| Error::HeadUnresolved {
        path: repo_path.to_path_buf(),
        source: e,
    })?;
    let tree = head_commit.tree()?;

    let mut out: Vec<(String, u64)> = Vec::new();
    tree.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
        if entry.kind() == Some(git2::ObjectType::Blob) {
            let name = entry.name().unwrap_or("");
            let full = if dir.is_empty() {
                name.to_string()
            } else {
                format!("{dir}{name}")
            };
            let size = repo
                .find_blob(entry.id())
                .map(|b| u64::try_from(b.size()).unwrap_or(0))
                .unwrap_or(0);
            out.push((full, size));
        }
        git2::TreeWalkResult::Ok
    })?;
    Ok(out)
}

/// Walk reachable history from HEAD and tally per-path commit counts,
/// author histograms, and most-recent-touch timestamps.
///
/// The walk only records stats for paths in `head_paths` — historical
/// renames are not followed; the path strings have to match the HEAD
/// tree path to count. This matches the AC's `git log --follow` only
/// approximately, but `--follow` is not stable enough in libgit2 to use
/// directly, and the human spec accepts equivalent semantics. See the
/// note in AC3.
fn collect_history_stats(
    repo: &Repository,
    repo_path: &Path,
    head_paths: &[(String, u64)],
) -> Result<HashMap<String, PerPathStats>, Error> {
    let head_set: HashSet<&str> = head_paths.iter().map(|(p, _)| p.as_str()).collect();

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

    let mut stats: HashMap<String, PerPathStats> = HashMap::new();

    for oid_res in revwalk {
        let oid = oid_res?;
        let commit = repo.find_commit(oid)?;
        let touched = commit_touched_paths(repo, &commit)?;
        let when = commit.committer().when().seconds();
        let author = commit.author().name().unwrap_or("<unknown>").to_string();

        for path in touched {
            if !head_set.contains(path.as_str()) {
                // Path no longer present at HEAD: ignored (rename or delete).
                continue;
            }
            let entry = stats.entry(path).or_default();
            entry.commit_count = entry.commit_count.saturating_add(1);
            if when > entry.last_touched_secs {
                entry.last_touched_secs = when;
            }
            let author_entry = entry
                .authors
                .entry(author.clone())
                .or_insert((0, i64::MIN));
            author_entry.0 = author_entry.0.saturating_add(1);
            if when > author_entry.1 {
                author_entry.1 = when;
            }
        }
    }
    Ok(stats)
}

/// Paths whose blob changed between `commit` and its first parent.
/// Root commits are diffed against an empty tree.
fn commit_touched_paths(repo: &Repository, commit: &Commit<'_>) -> Result<Vec<String>, Error> {
    let new_tree = commit.tree()?;
    let parent_tree: Option<Tree<'_>> = if commit.parent_count() == 0 {
        None
    } else {
        Some(commit.parent(0)?.tree()?)
    };

    let mut diff_opts = DiffOptions::new();
    diff_opts.include_typechange(true);

    let diff = repo.diff_tree_to_tree(
        parent_tree.as_ref(),
        Some(&new_tree),
        Some(&mut diff_opts),
    )?;

    let mut paths: Vec<String> = Vec::new();
    diff.foreach(
        &mut |delta, _progress| {
            if let Some(p) = delta.new_file().path().and_then(|p| p.to_str()) {
                paths.push(p.to_string());
            } else if let Some(p) = delta.old_file().path().and_then(|p| p.to_str()) {
                paths.push(p.to_string());
            }
            true
        },
        None,
        None,
        None,
    )?;
    Ok(paths)
}

/// Build a [`FileEntry`] for `path`, projecting `(commit_count,
/// last_touched, primary_author)` from `stats`. Files with no history
/// rows (corner case: file present in HEAD tree but never touched —
/// shouldn't happen, but guard anyway) get sensible defaults.
fn build_entry(
    path: String,
    size: u64,
    stats: &HashMap<String, PerPathStats>,
) -> FileEntry {
    let language = language_for_path(Path::new(&path)).to_string();
    let s = stats.get(&path);
    let commit_count = s.map_or(0, |s| s.commit_count);
    let last_touched_secs = s.map_or(0, |s| s.last_touched_secs);
    let last_touched = Utc.timestamp_opt(last_touched_secs, 0).single().unwrap_or_else(Utc::now);
    let primary_author = s
        .and_then(|s| pick_primary_author(&s.authors))
        .unwrap_or_else(|| "<unknown>".to_string());
    FileEntry {
        path,
        language,
        commit_count,
        last_touched,
        primary_author,
        size_bytes: size,
    }
}

/// Pick the author with the most commits; ties broken by most-recent
/// commit. Returns `None` if the histogram is empty.
fn pick_primary_author(hist: &HashMap<String, (u32, i64)>) -> Option<String> {
    hist.iter()
        .max_by(|a, b| {
            let ((_a_name, (a_count, a_when)), (_b_name, (b_count, b_when))) = (a, b);
            a_count.cmp(b_count).then_with(|| a_when.cmp(b_when))
        })
        .map(|(name, _)| name.clone())
}

/// Coalesce directories with fewer than `threshold` direct file
/// children into a single synthetic `<dir>/.` entry.
fn coalesce_directories(files: Vec<FileEntry>, threshold: u32) -> Vec<FileEntry> {
    // Group by parent directory ("" = repo root).
    let mut groups: BTreeMap<String, Vec<FileEntry>> = BTreeMap::new();
    for entry in files {
        let parent = parent_dir_of(&entry.path);
        groups.entry(parent).or_default().push(entry);
    }

    let mut out: Vec<FileEntry> = Vec::new();
    for (parent, group) in groups {
        let count = u32::try_from(group.len()).unwrap_or(u32::MAX);
        if !parent.is_empty() && count < threshold {
            // Collapse.
            let mut total_commits: u32 = 0;
            let mut total_size: u64 = 0;
            let mut max_when: i64 = i64::MIN;
            let mut latest_author = String::from("<unknown>");
            for e in &group {
                total_commits = total_commits.saturating_add(e.commit_count);
                total_size = total_size.saturating_add(e.size_bytes);
                let when = e.last_touched.timestamp();
                if when > max_when {
                    max_when = when;
                    latest_author.clone_from(&e.primary_author);
                }
            }
            let when_dt = Utc
                .timestamp_opt(max_when, 0)
                .single()
                .unwrap_or_else(Utc::now);
            out.push(FileEntry {
                path: format!("{parent}/."),
                language: "directory".to_string(),
                commit_count: total_commits,
                last_touched: when_dt,
                primary_author: latest_author,
                size_bytes: total_size,
            });
        } else {
            out.extend(group);
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// Repo-relative parent directory of `path` (`""` for repo root).
fn parent_dir_of(path: &str) -> String {
    match path.rsplit_once('/') {
        Some((parent, _)) => parent.to_string(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parent_dir_root() {
        assert_eq!(parent_dir_of("README.md"), "");
        assert_eq!(parent_dir_of("src/lib.rs"), "src");
        assert_eq!(parent_dir_of("a/b/c.rs"), "a/b");
    }

    #[test]
    fn pick_primary_author_majority() {
        let mut h: HashMap<String, (u32, i64)> = HashMap::new();
        h.insert("Alice".into(), (3, 100));
        h.insert("Bob".into(), (1, 200));
        assert_eq!(pick_primary_author(&h).as_deref(), Some("Alice"));
    }

    #[test]
    fn pick_primary_author_tie_by_recent() {
        let mut h: HashMap<String, (u32, i64)> = HashMap::new();
        h.insert("Alice".into(), (2, 100));
        h.insert("Bob".into(), (2, 200));
        assert_eq!(pick_primary_author(&h).as_deref(), Some("Bob"));
    }
}
