# memory-reliquary

> The annual recall-memories book is the artifact, but the operational bottleneck is the deterministic typesetting-input step: a script that walks the year's recall memories and produces a clean, ordered, frontmatter-rich Markdown bundle that a Typst template (Phase 1b) can consume.

## Why

The annual recall-memories book is the artifact, but the operational bottleneck is the deterministic typesetting-input step: a script that walks the year's recall memories and produces a clean, ordered, frontmatter-rich Markdown bundle that a Typst template (Phase 1b) can consume. Without this composer the January ritual is a manual afternoon; with it, the ritual is `composer + render + send-to-printer`.

## Build

```sh
cargo build --release
```

Produces `target/release/reliquary`. Symlink into `~/.local/bin/` if you want it on `$PATH`.

## Usage

```sh
reliquary --help
```

## Audience

the author running it the first week of January for the prior calendar year, reading the resulting Markdown bundle to spot-check before handing to the Typst template. Audience: future the author with a hardback book.

## Acceptance criteria

This project was scaffolded from a PRD via the `autobuilder` pipeline. The MUST-level acceptance criteria are:

- **AC1**: `reliquary compose --year <YYYY> [--recall-root <dir>] [--out <file>]` reads memory index entries with subject==user and created_at within the calendar year, slurps each memory's Markdown body from disk, and emits a single bundle file. D...
- **AC2**: The bundle format: each memory is a section separated by `\n---\n` lines. Each section starts with a YAML frontmatter block (id, kind, subject, recall_count, last_recalled_at, created_at), then a blank line, then the memory's Markdown bo...
- **AC3**: Memories are ordered by created_at ascending. Two memories with identical timestamps tiebreak by id.
- **AC4**: Memories whose frontmatter contains `private: true` are excluded from the bundle. The top-level frontmatter records `excluded_private_count: <N>` so the omission is auditable.
- **AC5**: When the recall-root path does not exist or is not a directory: exit 2 with stderr containing the path and `recall root not found`.
- **AC6**: A memory file whose YAML frontmatter is unparseable is skipped (not crashed) and a warning line is written to stderr: `WARN: skipping <path>: frontmatter parse error: <msg>`. The bundle continues.

Each AC has a matching integration test under `tests/acceptance_ac<n>.rs`.

## Provenance

Built via the [`autobuilder`](https://github.com/j0yen/autobuilder) pipeline (PRD intake -> intent-card -> scaffold -> iterate-and-prove). Originally consolidated as a subdir of the [`wintermute`](https://github.com/j0yen/wintermute) monorepo; this standalone repo is a fresh-init snapshot for easier consumption and distribution.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
