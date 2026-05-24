# letters-we-never-sent

> The monthly draft ritual produces 2-4 letter Markdown files per month in ~/.claude/letters-we-never-sent/<year>/.

## Why

The monthly draft ritual produces 2-4 letter Markdown files per month in ~/.claude/letters-we-never-sent/<year>/. Phase 1 is the curation tool the author needs to triage drafts: read, accept/decline/edit, optionally mark `send-for-real`. Without it, drafts accumulate in a directory and the annual binding ritual has no signal for which letters made the cut.

## Build

```sh
cargo build --release
```

Produces `target/release/letter`. Symlink into `~/.local/bin/` if you want it on `$PATH`.

## Usage

```sh
letter --help
```

## Audience

the author at the end of each month (or whenever the practice calls), running the curation CLI over ~/.claude/letters-we-never-sent/<year>/ to triage drafts before the annual binding ritual. Audience: the author himself, alone, in a focused reading session.

## Acceptance criteria

This project was scaffolded from a PRD via the `autobuilder` pipeline. The MUST-level acceptance criteria are:

- **AC1**: `letter list [--root <dir>] [--year <YYYY>] [--state <all|pending|accepted|declined|send-real>]` prints a one-line-per-letter table to stdout: `<filename> <state> <accepted_at|-> <subject-line>`. Default --root = `~/.claude/letters-we-ne...
- **AC2**: Each letter is a Markdown file with YAML frontmatter. Recognised frontmatter keys: `recipient` (anonymized, e.g. `the PM`), `month` (YYYY-MM), `state` (one of `pending|accepted|declined|send-real`, default `pending`), `accepted_at` (RFC3...
- **AC3**: `letter accept <filename> [--root <dir>]` rewrites the file's frontmatter to set state=accepted and accepted_at=now(RFC3339). The body content is preserved byte-for-byte after the frontmatter. Exit 0 on success.
- **AC4**: `letter decline <filename> [--root <dir>]` sets state=declined and accepted_at=now. Same byte-for-byte body preservation. `letter mark-send-real <filename>` sets state=send-real. All three state transitions are idempotent (re-running the...
- **AC5**: Letters without frontmatter, or with malformed YAML, are surfaced in `list` output as state=`(unparseable)` and skipped from state changes (`accept`/`decline` exits 3 with stderr explaining why). The CLI never crashes on malformed input.
- **AC6**: Filenames are positional arguments; both relative-to-root (`NN.md`) and absolute paths are accepted. Glob patterns (e.g. `2026-05-*.md`) are NOT auto-expanded — the user uses shell globbing for that. Single filename per invocation.

Each AC has a matching integration test under `tests/acceptance_ac<n>.rs`.

## Provenance

Built via the [`autobuilder`](https://github.com/j0yen/autobuilder) pipeline (PRD intake -> intent-card -> scaffold -> iterate-and-prove). Originally consolidated as a subdir of the [`wintermute`](https://github.com/j0yen/wintermute) monorepo; this standalone repo is a fresh-init snapshot for easier consumption and distribution.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
