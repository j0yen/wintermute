# repo-as-landscape

> Make software visible-as-terrain by extracting the per-file topographic primitives from a git repository (elevation = commit density, biome = language, territory = primary author).

## Why

Make software visible-as-terrain by extracting the per-file topographic primitives from a git repository (elevation = commit density, biome = language, territory = primary author). Phase 0 produces the structured walker JSON that all later rendering passes (SVG mockup, cartographer, A1 print) consume. Until the walker emits faithful primitives, no downstream art can be evaluated.

## Build

```sh
cargo build --release
```

Produces `target/release/repo-as-landscape`. Symlink into `~/.local/bin/` if you want it on `$PATH`.

## Usage

```sh
repo-as-landscape --help
```

## Audience

the author running it against `~/wintermute` (and later other repos) to inspect the JSON output before any rendering exists. Audience is the human eye reading the JSON to sanity-check that the topographic primitives feel right.

## Acceptance criteria

This project was scaffolded from a PRD via the `autobuilder` pipeline. The MUST-level acceptance criteria are:

- **AC1**: `repo-as-landscape walk --repo <git-dir>` emits a single JSON object on stdout. Top-level: `{"repo": "<absolute path>", "generated_at": "<RFC3339>", "files": [...]}`. Each file entry is `{"path": "<repo-relative>", "language": "<lang>", ...
- **AC2**: Language detection covers at least: Rust (.rs), Python (.py), TypeScript (.ts/.tsx), JavaScript (.js/.jsx), Markdown (.md), JSON (.json), TOML (.toml), Shell (.sh/.bash/.zsh), YAML (.yml/.yaml), HTML (.html), CSS (.css), Go (.go), C (.c/...
- **AC3**: `commit_count` counts the number of commits touching the file across the full reachable history of HEAD (equivalent to `git log --follow --oneline -- <path> | wc -l`). `last_touched` is the committer date of the most recent such commit.
- **AC4**: `primary_author` is the author whose commits touching the file outnumber any other author's commits on it. Ties break by most recent commit. Author identity is the commit author name (not email).
- **AC5**: Running against a non-git path exits with code 2 and stderr containing the path and `not a git repository`.
- **AC6**: Files matched by `.gitignore` are excluded from the output (we walk the index/HEAD, not the working tree). Untracked files do not appear.

Each AC has a matching integration test under `tests/acceptance_ac<n>.rs`.

## Provenance

Built via the [`autobuilder`](https://github.com/j0yen/autobuilder) pipeline (PRD intake -> intent-card -> scaffold -> iterate-and-prove). Originally consolidated as a subdir of the [`wintermute`](https://github.com/j0yen/wintermute) monorepo; this standalone repo is a fresh-init snapshot for easier consumption and distribution.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
