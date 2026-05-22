# recall

Local-first agentic memory: file-backed memories with a keyword/FTS5 index.
The ninth wintermute tool. Implements Phase 0 + Phase 1 of the agentic
memory PRD (`~/projects/autobuilder/PRD-agentic-memory.md`).

`recall` stores agent memories as plain Markdown files with YAML frontmatter
under `~/.claude/recall/memories/`, and layers a SQLite + FTS5 index on top
for fast keyword search and ranked retrieval. Markdown is the source of
truth; the index is rebuildable from disk.

## Status

| Phase | Scope                                                                | Status |
| ----- | -------------------------------------------------------------------- | ------ |
| 0     | Data layout under `~/.claude/recall/`                                | done   |
| 1     | File store + FTS5 keyword index + ranked retrieval + CLI             | done   |
| 2     | Local embeddings + hybrid retrieval; confidence + decay              | todo   |
| 3     | Within-session scratch + compaction survival                          | todo   |
| 4     | Observed-write proposals (`PostToolUse` hook)                         | todo   |
| 5     | Cross-project recall + audit trail + session-diff                     | todo   |

## Install

```sh
cd ~/wintermute/recall
cargo build --release
install -Dm755 target/release/recall ~/.local/bin/recall
```

Toolchain pinned to `rustc 1.85.0` via `rust-toolchain.toml`.

## Quickstart

```sh
recall init
recall write --kind semantic  --subject user            --body "user prefers pnpm for typescript; cargo + uv for python"
recall write --kind procedural --subject project:recall --body "build with cargo build --release after sourcing ~/.cargo/env"
echo "user dislikes mocks in integration tests" | recall write --kind reflective --subject user

recall query "pnpm typescript"
recall query "mocks" --touch --format json
recall list --subject project:
recall show <id>
recall delete <id>
recall reindex     # wipe and rebuild the SQLite index from disk
recall where       # print the data root
```

Override the data root with `--root /path` or `RECALL_HOME=/path`.

## CLI

| Command   | Purpose                                                        |
| --------- | -------------------------------------------------------------- |
| `init`    | Create the data dir + SQLite index                             |
| `write`   | Add a memory (`--body`, `--file`, or stdin)                    |
| `query`   | Ranked keyword search (`--format text|json`, `--touch`)        |
| `list`    | List memories newest-first, optional `--subject` prefix        |
| `show`    | Print a memory's Markdown file by id                           |
| `delete`  | Remove a memory (file + index row)                             |
| `reindex` | Wipe and rebuild the SQLite index from disk                    |
| `where`   | Print the resolved data root                                   |

`recall write` and `recall list` take a `--subject` of:

| Value              | Meaning                                          |
| ------------------ | ------------------------------------------------ |
| `user`             | preferences, role, feedback                      |
| `self`             | how I, Claude, work in this environment          |
| `project:<slug>`   | per-project facts and procedures                 |
| `tool:<name>`      | per-tool quirks                                  |

`recall write` `--kind` is one of `semantic`, `procedural`, `episodic`,
`reflective` (see PRD §4.1).

## Data layout

```
~/.claude/recall/
├── memories/
│   ├── user/<id>.md
│   ├── self/<id>.md
│   ├── project/<slug>/<id>.md
│   └── tool/<name>/<id>.md
├── index/recall.sqlite      # FTS5 + meta, derivable from memories/
└── session/                 # within-session scratch (Phase 3, todo)
```

Each `<id>.md` is a [ULID](https://github.com/ulid/spec) with YAML frontmatter
and a Markdown body:

```markdown
---
id: 01KS90MDNK7WP1J6HHWZKBBMJ8
kind: semantic
subject: user
evidence: []
confidence: 0.5
created_at: 2026-05-22T23:34:21Z
recall_count: 0
---

user prefers pnpm for typescript; cargo + uv for python
```

`grep -r <term> ~/.claude/recall/memories/` always works.

## Ranking

`recall query` overfetches from FTS5 (BM25 ordered) and re-ranks with
configurable weights:

```
score = w_bm25 · (-bm25)
      + w_recency · exp(-days_since_last_recall / 30)
      + w_recall_count · tanh(recall_count / 5)
      + w_confidence · confidence
```

Defaults are in `recall::retrieval::Weights`.

## Hooks (planned)

Phase 4 will integrate via Claude Code `settings.json` hooks:

| Hook           | Behavior                                                                 |
| -------------- | ------------------------------------------------------------------------ |
| `SessionStart` | `recall query "<cwd + recent files>" --limit 3 --touch --format json`    |
| `PostToolUse`  | observe corrections and propose new memories                              |
| `Stop`         | promote `session/<id>.md` to long-term memory; emit a session diff       |

For now you can wire `SessionStart` manually:

```jsonc
// ~/.claude/settings.json
{
  "hooks": {
    "SessionStart": [
      "recall query \"$CLAUDE_USER_PROMPT\" --limit 3 --touch --format json"
    ]
  }
}
```

## Testing

```sh
cargo test --release
cargo clippy --release --all-targets -- -D warnings
```

12 tests across unit + integration: schema roundtrip, file store
read/write/walk/delete, index upsert + search + list + touch_recall + count
+ rebuild, end-to-end CLI flow.

## License

Dual-licensed under MIT OR Apache-2.0.
