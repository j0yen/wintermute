# Wintermute ecosystem repos

The wintermute project is a collection of small Rust CLIs and supporting
tooling for running Claude Code agents locally. Each piece is now its
own GitHub repo; this index lists them with one-line descriptions.

The [`bootstrap/install.sh`](bootstrap/install.sh) script in this repo
clones, builds, and wires up everything on a fresh machine.

## Pipeline / meta

| Repo | Binary | What it does |
|---|---|---|
| [autobuilder](https://github.com/j0yen/autobuilder) | `autobuilder` | PRD-driven Rust code generation pipeline with intent-cards, an iterate-and-prove loop, and a 25-receipt release gate. |
| [autobuilder-metric-harness](https://github.com/j0yen/autobuilder-metric-harness) | — | Unfakeable-scalar metric collector the autobuilder loop polls each iteration. |
| [learning-db](https://github.com/j0yen/learning-db) | — | Lessons-learned database used by autobuilder to seed the next slice with past mistakes. |

## Agent runtime

| Repo | Binary | What it does |
|---|---|---|
| [agorabus](https://github.com/j0yen/agorabus) | `agorabus` | Single-host advisory pub/sub bus for concurrent Claude sessions (UDS, announce/publish/subscribe/heartbeat). |
| [agent-pipe](https://github.com/j0yen/agent-pipe) | `apipe` | Streaming NDJSON record pipeline + shared schema for composing agent tools (`pass`/`top`/`sort`/`filter`/`pretty`). |
| [agentsh](https://github.com/j0yen/agentsh) | — | zsh plugin that flips agent-hostile defaults under `$CLAUDE_TOOL` (per-session HISTFILE, NOMATCH, NO_UNSET, no aliases). |
| [baton](https://github.com/j0yen/baton) | `baton` | Cross-window claude delegation primitive: address a visible target session by surface and type a prompt into it (xdotool/tmux). |
| [mcp-autotuner](https://github.com/j0yen/mcp-autotuner) | — | Cost-aware MCP tool-set tuner that prunes rarely-used tools to keep context tight. |
| [skill-manifest](https://github.com/j0yen/skill-manifest) | `skill` | Validator for the optional SKILL.md `manifest:` block (parser + JSON schema + CLI). |
| [skill-telemetry](https://github.com/j0yen/skill-telemetry) | `spool` | Per-skill invocation log with monthly JSONL buckets, rank + stale reports. |

## Memory layer

| Repo | Binary | What it does |
|---|---|---|
| [recall](https://github.com/j0yen/recall) | `recall` | Local-first agentic memory: file-backed memories with a keyword/FTS5 index. |
| [recall-doctor](https://github.com/j0yen/recall-doctor) | `recall-doctor` | Health checker for the recall store (fsck for memories). |
| [recall-io](https://github.com/j0yen/recall-io) | `recall-io` | Frontmatter parser + serializer used as the memory file I/O contract. |
| [recall-ops](https://github.com/j0yen/recall-ops) | `recall-ops` | Bulk ops over the recall store (move/relabel/dedupe). |
| [recall-memory-linter](https://github.com/j0yen/recall-memory-linter) | `recall-lint` | Style + structure linter for individual memory files. |
| [memory-reliquary](https://github.com/j0yen/memory-reliquary) | — | Annual book-of-memories renderer; pulls from recall, lays out a printable artifact. |

## Session / context

| Repo | Binary | What it does |
|---|---|---|
| [session-index](https://github.com/j0yen/session-index) | `transcript` | FTS5 index over `~/.claude/projects/*.jsonl` session traces (search past Claude Code conversations). |
| [session-trace-receipt](https://github.com/j0yen/session-trace-receipt) | — | NDJSON receipt producer recording per-session activity for the autobuilder gate. |
| [episodic-observer](https://github.com/j0yen/episodic-observer) | `episode` | Background observer that chunks long sessions into episodic-memory candidates for recall. |
| [claude-self](https://github.com/j0yen/claude-self) | `claude-self` | Maintainer for `CLAUDE_SELF.md`, the negotiated agent-self contract on disk. |
| [self-portrait](https://github.com/j0yen/self-portrait) | — | Visualizer of CLAUDE_SELF.md diffs over time (how Claude's self-description has drifted). |
| [mirror](https://github.com/j0yen/mirror) | `mirror` | Tool-use evaluator + feedback loop so Claude knows whether it is improving on its tool calls. |

## Artist / narrative

These are intentionally personal projects — operational tooling for art,
zines, and tracking the laptop-day's rhythm. They're shared in case the
patterns are useful, not because they're meant to be reused as-is.

| Repo | What it does |
|---|---|
| [ambient](https://github.com/j0yen/ambient) | Telemetry-driven parameter orchestrator that maps laptop signals (ctrace/wchg/git/builds) to cues for a generative ambient piece. |
| [confidant](https://github.com/j0yen/confidant) | Weekly letter composer + e-ink PNG renderer (400×300) for a desk-side RPi Zero device. |
| [conversations-zine](https://github.com/j0yen/conversations-zine) | Quarterly zine moment-extractor: pulls memorable lines from Claude conversation logs. |
| [daily-receipt](https://github.com/j0yen/daily-receipt) | Deterministic Rust core for the Daily Receipt art project (one-day-on-a-page printable). |
| [fsstory](https://github.com/j0yen/fsstory) | Filesystem attribution timeline: who-changed-what when a file changes. |
| [letters-we-never-sent](https://github.com/j0yen/letters-we-never-sent) | Monthly draft-ritual aggregator over `~/.claude/letters/`. |
| [morsel-bake](https://github.com/j0yen/morsel-bake) | Build helper for the morsel embedded-ML crate. |
| [provfs](https://github.com/j0yen/provfs) | FUSE-overlay that stamps `user.prov.*` xattrs at write-time (session/tool/turn/intent/history); the data layer fsstory queries. |
| [repo-as-landscape](https://github.com/j0yen/repo-as-landscape) | Topographic visualization of a repository (per-file primitives → terrain). |
| [tide-chart](https://github.com/j0yen/tide-chart) | Glanceable instrument-style view of the laptop's daily rhythm. |

## License

Each repo is dual-licensed MIT or Apache-2.0 at the user's option.
