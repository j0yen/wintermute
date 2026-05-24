# self-portrait

> CLAUDE_SELF.md is the negotiated contract between user and agent — the *diffs* are the portrait.

## Why

CLAUDE_SELF.md is the negotiated contract between user and agent — the *diffs* are the portrait. Phase 1a extracts every commit's hash/date/diff-hunk/message into structured JSON so downstream rendering passes (Claude reflections, Typst typesetting, A2 print) can consume a stable contract. Until the extractor exists, no later artifact can be built or audited.

## Build

```sh
cargo build --release
```

Produces `target/release/self-portrait`. Symlink into `~/.local/bin/` if you want it on `$PATH`.

## Usage

```sh
self-portrait --help
```

## Audience

the author running it against the wintermute CLAUDE_SELF.md (`/home/the author/wintermute/dotfiles/.claude/CLAUDE_SELF.md` or wherever the file lives in dotfiles). Audience for the JSON: the next pipeline stage (reflection generator) and the human eye spot-checking that diffs round-trip cleanly.

## Acceptance criteria

This project was scaffolded from a PRD via the `autobuilder` pipeline. The MUST-level acceptance criteria are:

- **AC1**: `self-portrait extract --file <repo-relative-path> [--repo <dir>]` emits a single JSON object on stdout: `{file, repo, generated_at, commits: [...]}`. Each commit entry has: hash (full sha), short_hash (7 chars), author_name, author_emai...
- **AC2**: Follows file renames via git's --follow semantics — a commit that renamed the file appears in the commits array, with `diff` containing the rename hunk.
- **AC3**: Commits that touched the file with a trivial change (no semantic diff after the rename) still appear and their `diff` is included verbatim; the consumer decides whether to collapse them. The extractor does not editorialize.
- **AC4**: When the file does not exist in the current HEAD but did historically (e.g. it was deleted), the extractor still walks the history — exits 0 with all commits in `commits` and a top-level `"deleted_at": "<RFC3339>"` field. If the file nev...
- **AC5**: When `--repo <dir>` points at a non-git path: exit code 2 with stderr containing the path and `not a git repository`.
- **AC6**: Commit messages with `private: true` in a Markdown-style trailer line (e.g. line `private: true` at column 0) cause the diff hunk for that commit to be redacted to the string `"[redacted]"`. The hash/date/author/subject still appear so t...

Each AC has a matching integration test under `tests/acceptance_ac<n>.rs`.

## Provenance

Built via the [`autobuilder`](https://github.com/j0yen/autobuilder) pipeline (PRD intake -> intent-card -> scaffold -> iterate-and-prove). Originally consolidated as a subdir of the [`wintermute`](https://github.com/j0yen/wintermute) monorepo; this standalone repo is a fresh-init snapshot for easier consumption and distribution.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
