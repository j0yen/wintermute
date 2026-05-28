# Wintermute ecosystem repos

The wintermute project is a collection of small Rust CLIs and supporting
tooling for running Claude Code agents locally. Each piece is now its
own GitHub repo; this index lists them with one-line descriptions.

The [`bootstrap/install.sh`](bootstrap/install.sh) script in this repo
clones, builds, and wires up everything on a fresh machine.

## Pipeline / meta

| Repo | Binary | What it does |
|---|---|---|
| [autobuilder](https://github.com/j0yen/autobuilder) | `autobuilder` | Claude Code skill + Rust companion binary that turns a PRD into a vetted Rust project — intent-cards, iterate-and-prove loop, 25-receipt release gate. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/autobuilder/main/skill/install.sh \| bash`. |
| [autobuilder-metric-harness](https://github.com/j0yen/autobuilder-metric-harness) | — | Unfakeable-scalar metric collector the autobuilder loop polls each iteration. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/autobuilder-metric-harness/main/install.sh \| bash`. |
| [learning-db](https://github.com/j0yen/learning-db) (aka `database0`) | — | Educational, configurable DBMS — every subsystem (buffer pool, indexes, joins, MVCC) is a swappable implementation. Companion to CMU 15-445/645. TypeScript / pnpm workspace; see the repo README for install. |

## Agent runtime

| Repo | Binary | What it does |
|---|---|---|
| [agorabus](https://github.com/j0yen/agorabus) | `agorabus` | Single-host advisory pub/sub bus for concurrent Claude sessions (UDS, announce/publish/subscribe/heartbeat). One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/agorabus/main/install.sh \| bash`. |
| [agent-pipe](https://github.com/j0yen/agent-pipe) | `apipe` | Streaming NDJSON record pipeline + shared schema for composing agent tools (`pass`/`top`/`sort`/`filter`/`pretty`). One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/agent-pipe/main/install.sh \| bash`. |
| [agentsh](https://github.com/j0yen/agentsh) | — | zsh plugin that flips agent-hostile defaults under `$CLAUDE_TOOL` (per-session HISTFILE, NOMATCH, NO_UNSET, no aliases). |
| [baton](https://github.com/j0yen/baton) | `baton` | Cross-window claude delegation primitive: address a visible target session by surface and type a prompt into it (xdotool/tmux). |
| [mcp-autotuner](https://github.com/j0yen/mcp-autotuner) | — | Cost-aware MCP tool-set tuner that prunes rarely-used tools to keep context tight. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/mcp-autotuner/main/install.sh \| bash`. |
| [skill-manifest](https://github.com/j0yen/skill-manifest) | `skill` | Validator for the optional SKILL.md `manifest:` block (parser + JSON schema + CLI). One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/skill-manifest/main/install.sh \| bash`. |
| [skill-telemetry](https://github.com/j0yen/skill-telemetry) | `spool` | Per-skill invocation log with monthly JSONL buckets, rank + stale reports. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/skill-telemetry/main/install.sh \| bash`. |
| [build-skill](https://github.com/j0yen/build-skill) | — | Claude Code skill (`/build`): continuous PRD implementation loop. Runs every 5 min via systemd-user timer, picks one queued PRD per tick, delegates Rust to `/autobuilder`, publishes the result. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/build-skill/main/install.sh \| bash`. |
| [dream-skill](https://github.com/j0yen/dream-skill) | — | Claude Code skill (`/dream`): vision into PRDs. Listens, researches, decomposes a vision into a fleet of PRD-sized pieces, gossips with `/build` via a shared channel. Runs overnight every 30 min. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/dream-skill/main/install.sh \| bash`. |

## Memory layer

| Repo | Binary | What it does |
|---|---|---|
| [recall](https://github.com/j0yen/recall) | `recall` | Local-first agentic memory for Claude Code: file-backed memories, FTS5 + semantic (BGE-small/fastembed) hybrid index, four hook scripts that wire the braid correlator into a live session. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/recall/main/install.sh \| bash`. |
| [recall-doctor](https://github.com/j0yen/recall-doctor) | `recall-doctor` | Health checker for the recall store (fsck for memories). One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/recall-doctor/main/install.sh \| bash`. |
| [recall-io](https://github.com/j0yen/recall-io) | `recall-io` | Frontmatter parser + serializer used as the memory file I/O contract. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/recall-io/main/install.sh \| bash`. |
| [recall-ops](https://github.com/j0yen/recall-ops) | `recall-ops` | Bulk ops over the recall store (move/relabel/dedupe). One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/recall-ops/main/install.sh \| bash`. |
| [recall-memory-linter](https://github.com/j0yen/recall-memory-linter) | `recall-lint` | Style + structure linter for individual memory files. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/recall-memory-linter/main/install.sh \| bash`. |
| [memory-reliquary](https://github.com/j0yen/memory-reliquary) | — | Annual book-of-memories renderer; pulls from recall, lays out a printable artifact. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/memory-reliquary/main/install.sh \| bash`. |

## Session / context

| Repo | Binary | What it does |
|---|---|---|
| [session-index](https://github.com/j0yen/session-index) | `transcript` | FTS5 index over `~/.claude/projects/*.jsonl` session traces (search past Claude Code conversations). One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/session-index/main/install.sh \| bash`. |
| [session-trace-receipt](https://github.com/j0yen/session-trace-receipt) | — | NDJSON receipt producer recording per-session activity for the autobuilder gate. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/session-trace-receipt/main/install.sh \| bash`. |
| [episodic-observer](https://github.com/j0yen/episodic-observer) | `episode` | Background observer that chunks long sessions into episodic-memory candidates for recall. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/episodic-observer/main/install.sh \| bash`. |
| [claude-self](https://github.com/j0yen/claude-self) | `claude-self` | Maintainer for `CLAUDE_SELF.md`, the negotiated agent-self contract on disk. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/claude-self/main/install.sh \| bash`. |
| [self-portrait](https://github.com/j0yen/self-portrait) | — | Visualizer of CLAUDE_SELF.md diffs over time (how Claude's self-description has drifted). One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/self-portrait/main/install.sh \| bash`. |
| [mirror](https://github.com/j0yen/mirror) | `mirror` | Tool-use evaluator + feedback loop so Claude knows whether it is improving on its tool calls. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/mirror/main/install.sh \| bash`. |

## Artist / narrative

These are intentionally personal projects — operational tooling for art,
zines, and tracking the laptop-day's rhythm. They're shared in case the
patterns are useful, not because they're meant to be reused as-is.

| Repo | What it does |
|---|---|
| [ambient](https://github.com/j0yen/ambient) | Telemetry-driven parameter orchestrator that maps laptop signals (ctrace/wchg/git/builds) to cues for a generative ambient piece. |
| [confidant](https://github.com/j0yen/confidant) | Weekly letter composer + e-ink PNG renderer (400×300) for a desk-side RPi Zero device. |
| [conversations-zine](https://github.com/j0yen/conversations-zine) | Quarterly zine moment-extractor: pulls memorable lines from Claude conversation logs. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/conversations-zine/main/install.sh \| bash`. |
| [daily-receipt](https://github.com/j0yen/daily-receipt) | Deterministic Rust core for the Daily Receipt art project (one-day-on-a-page printable). One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/daily-receipt/main/install.sh \| bash`. |
| [fsstory](https://github.com/j0yen/fsstory) | Filesystem attribution timeline: who-changed-what when a file changes. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/fsstory/main/install.sh \| bash`. |
| [letters-we-never-sent](https://github.com/j0yen/letters-we-never-sent) | Monthly draft-ritual aggregator over `~/.claude/letters/`. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/letters-we-never-sent/main/install.sh \| bash`. |
| [morsel-bake](https://github.com/j0yen/morsel-bake) | Build helper for the morsel embedded-ML crate. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/morsel-bake/main/install.sh \| bash`. |
| [provfs](https://github.com/j0yen/provfs) | FUSE-overlay + in-kernel LSM that stamps `user.prov.{session,tool,turn,ts,history}` xattrs at write-time. |
| [repo-as-landscape](https://github.com/j0yen/repo-as-landscape) | Topographic visualization of a repository (per-file primitives → terrain). One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/repo-as-landscape/main/install.sh \| bash`. |
| [tide-chart](https://github.com/j0yen/tide-chart) | Glanceable instrument-style view of the laptop's daily rhythm. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/tide-chart/main/install.sh \| bash`. |

## Kernel

Vendor kernel features that give Claude sessions first-class identity,
audit trails, and provenance metadata at the OS layer. Shipped as a
parallel-install Arch kernel so the stock `linux` package stays untouched.

| Repo | What it does |
|---|---|
| [memlog](https://github.com/j0yen/memlog) | /dev/memlog kernel char device + per-uid ring for LLM context-compaction audit records. |
| [agentns](https://github.com/j0yen/agentns) | CLONE_NEWAGENT — 8th Linux namespace type for per-session identity, counters, and budget enforcement. |
| [wintermute-kernel](https://github.com/j0yen/wintermute-kernel) | Arch PKGBUILD for the parallel-install linux-wintermute kernel (agentns + memlog + provfs LSM baked in). |

## Wintermute fleet

The voice-assistant fleet — Fleet 1's hearing, speaking, and routing
pieces. Each runs as a small daemon that exchanges JSON envelopes over
agorabus.

| Repo | Binary | What it does |
|---|---|---|
| [wintermute-bootstrap](https://github.com/j0yen/wintermute-bootstrap) | `wm-bootstrap` | First-boot caregiver setup web server — mDNS-announced HTTP form that writes day-1 env config and hands off to wintermute.target. |
| [wintermute-tts](https://github.com/j0yen/wintermute-tts) | `wm-tts` | Text-to-speech daemon: Piper CPU-local primary, ElevenLabs opt-in cloud streaming, agorabus-driven, voicepack-based. |

## License

Each repo is dual-licensed MIT or Apache-2.0 at the user's option.
