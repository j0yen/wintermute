# recall

Local-first agentic memory: file-backed memories with a keyword/FTS5 index.
The ninth wintermute tool. Implements Phase 0 + Phase 1 of the agentic
memory PRD (`~/projects/autobuilder/PRD-agentic-memory.md`).

`recall` stores agent memories as plain Markdown files with YAML frontmatter
under `~/.claude/recall/memories/`, and layers a SQLite + FTS5 index on top
for fast keyword search and ranked retrieval. Markdown is the source of
truth; the index is rebuildable from disk.

## Status

| Phase | Scope                                                                | Status   |
| ----- | -------------------------------------------------------------------- | -------- |
| 0     | Data layout under `~/.claude/recall/`                                | done     |
| 1     | File store + FTS5 keyword index + ranked retrieval + CLI             | done     |
| 2a    | `Embedder` trait + hashed-feature default + hybrid retrieval         | done     |
| 2b    | Swap in BGE-small (`fastembed-rs`) or HTTP sidecar (`ollama`)        | todo     |
| 3     | Within-session scratch + compaction survival                          | todo     |
| 4     | Observed-write proposals (`PostToolUse` hook)                         | todo     |
| 5     | Cross-project recall + audit trail + session-diff                     | todo     |

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
recall query "compile project with cargo" --hybrid    # FTS5 + vector cosine
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
      + w_vector · cosine(query_vec, memory_vec)     [hybrid mode only]
      + w_recency · exp(-days_since_last_recall / 30)
      + w_recall_count · tanh(recall_count / 5)
      + w_confidence · confidence
```

Defaults are in `recall::retrieval::Weights`.

## Embeddings (Phase 2a)

`recall write` and `recall reindex` compute a 256-dim L2-normalized vector for
each memory and store it as a BLOB in `memories_meta.embedding`. The default
embedder is `HashEmbedder` — character n-grams + word features mixed into the
vector via hashing. It's not a transformer-quality semantic model: it catches
morphological variation that BM25 alone misses, but it will not match true
synonyms or paraphrases.

Phase 2b will swap in a real local model (BGE-small via `fastembed-rs`, or
`ollama`'s `/api/embeddings`) behind the same `Embedder` trait — the storage
schema (`embedding`, `embedding_id`, `embedding_dim`) is already in place.

`recall query --hybrid` enables vector-augmented retrieval. Without `--hybrid`
the CLI behaves exactly like Phase 1.

## Hooks

| Hook           | Status   | Behavior                                                                 |
| -------------- | -------- | ------------------------------------------------------------------------ |
| `SessionStart` | shipped  | [`hooks/session-start.sh`](hooks/session-start.sh) emits user + project + self memories at session start |
| `PostToolUse`  | planned  | observe corrections and propose new memories                              |
| `Stop`         | planned  | promote `session/<id>.md` to long-term memory; emit a session diff       |

### Wire the SessionStart hook into Claude Code

```sh
install -Dm755 hooks/session-start.sh ~/.claude/scripts/recall-session-start.sh
```

Then add this entry to the `SessionStart` hook list in `~/.claude/settings.json`:

```jsonc
{
  "hooks": {
    "SessionStart": [
      {
        "matcher": "*",
        "hooks": [
          /* …existing hooks… */
          {
            "type": "command",
            "command": "/home/jsy/.claude/scripts/recall-session-start.sh"
          }
        ]
      }
    ]
  }
}
```

The script honors `$CLAUDE_PROJECT_DIR` (falls back to `$PWD`) for the
project scope and `$RECALL_BIN` / `$RECALL_SESSION_LIMIT` overrides.
It exits silently when the memory store is empty.

## Testing

```sh
cargo test --release
cargo clippy --release --all-targets -- -D warnings
```

17 tests across unit + integration: schema roundtrip, file store
read/write/walk/delete, index upsert + search + vector_search + list +
touch_recall + count + rebuild, embeddings determinism + cosine ranking +
pack/unpack roundtrip, end-to-end CLI flow including hybrid retrieval.

## License

Dual-licensed under MIT OR Apache-2.0.
