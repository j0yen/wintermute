# Wintermute ecosystem repos

The wintermute project is a collection of small Rust CLIs and supporting
tooling for running Claude Code agents locally. Each piece is now its
own GitHub repo; this index lists them with one-line descriptions.

The [`bootstrap/install.sh`](bootstrap/install.sh) script in this repo
clones, builds, and wires up everything on a fresh machine.

## Pipeline / meta

| Repo | Binary | What it does |
|---|---|---|
| [atlas](https://github.com/j0yen/atlas) | `atlas` | Queryable node graph of the wintermute PRD corpus: parse every PRD's frontmatter + both skill manifests + REPOS.md into typed vision/prd/repo nodes; `atlas nodes` + `atlas show <vision>` with `--format json`. Read-only; ~23 ms cold run over 100+ PRDs. |
| [autobuilder](https://github.com/j0yen/autobuilder) | `autobuilder` | Claude Code skill + Rust companion binary that turns a PRD into a vetted Rust project — intent-cards, iterate-and-prove loop, 25-receipt release gate. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/autobuilder/main/skill/install.sh \| bash`. |
| [autobuilder-metric-harness](https://github.com/j0yen/autobuilder-metric-harness) | — | Unfakeable-scalar metric collector the autobuilder loop polls each iteration. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/autobuilder-metric-harness/main/install.sh \| bash`. |
| [cradle](https://github.com/j0yen/cradle) | `cradle` | Self-trained-model pipeline: harvest labeled data from Claude transcripts, orchestrate train (Python shellout) + bake (deferred — see PRD-cradle-bake-integration.md) into Rust crates via [morsel](https://github.com/j0yen/morsel). v0.1 ships harvest + features + train orchestration. |
| [learning-db](https://github.com/j0yen/learning-db) (aka `database0`) | — | Educational, configurable DBMS — every subsystem (buffer pool, indexes, joins, MVCC) is a swappable implementation. Companion to CMU 15-445/645. TypeScript / pnpm workspace; see the repo README for install. |

## Agent runtime

| Repo | Binary | What it does |
|---|---|---|
| [agorabus](https://github.com/j0yen/agorabus) | `agorabus` | Single-host advisory pub/sub bus for concurrent Claude sessions (UDS, announce/publish/subscribe/heartbeat). One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/agorabus/main/install.sh \| bash`. |
| [agent-pipe](https://github.com/j0yen/agent-pipe) | `apipe` | Streaming NDJSON record pipeline + shared schema for composing agent tools (`pass`/`top`/`sort`/`filter`/`pretty`). One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/agent-pipe/main/install.sh \| bash`. |
| [agentsh](https://github.com/j0yen/agentsh) | — | zsh plugin that flips agent-hostile defaults under `$CLAUDE_TOOL` (per-session HISTFILE, NOMATCH, NO_UNSET, no aliases). |
| [agentns-claude](https://github.com/j0yen/agentns-claude) | `agentns-claude` | Wrap a command (typically `claude`) in a wintermute agent namespace so `/proc/$PID/agent_session` reads stably from session birth. Mock mode + `--no-unshare` fall back to userspace session-id synthesis on stock kernels; live unshare/budget/intent-tag plumbing activates under `linux-wintermute`. |
| [agentns-doctor](https://github.com/j0yen/agentns-doctor) | `agentns-doctor` | Read-only CLI that classifies a process's agent-namespace state from `/proc` surfaces into `absent`/`init`/`live`/`malformed`; ends the mis-diagnosis of healthy init-ns zeros as "broken kernel." |
| [baton](https://github.com/j0yen/baton) | `baton` | Cross-window claude delegation primitive: address a visible target session by surface and type a prompt into it (xdotool/tmux). |
| [mcp-autotuner](https://github.com/j0yen/mcp-autotuner) | — | Cost-aware MCP tool-set tuner that prunes rarely-used tools to keep context tight. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/mcp-autotuner/main/install.sh \| bash`. |
| [skill-doctor](https://github.com/j0yen/skill-doctor) | `skill-doctor` | Walks `~/.claude/skills/*/SKILL.md`, extracts shell invocations, and cross-references each flag/subcommand against the `tool-manifest` JSON; parks review-gated drift proposals at `~/.claude/skill-doctor/proposals/<ULID>.md` (no auto-edit). |
| [skill-manifest](https://github.com/j0yen/skill-manifest) | `skill` | Validator for the optional SKILL.md `manifest:` block (parser + JSON schema + CLI). One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/skill-manifest/main/install.sh \| bash`. |
| [skill-telemetry](https://github.com/j0yen/skill-telemetry) | `spool` | Per-skill invocation log with monthly JSONL buckets, rank + stale reports. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/skill-telemetry/main/install.sh \| bash`. |
| [tool-manifest](https://github.com/j0yen/tool-manifest) | `tool-manifest` | Probes installed binaries' `--help` surface into a structured JSON manifest — ground truth for skill-doctor and Fleet 2 drift checks. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/tool-manifest/main/install.sh \| bash`. |
| [build-skill](https://github.com/j0yen/build-skill) | — | Claude Code skill (`/build`): continuous PRD implementation loop. Runs every 5 min via systemd-user timer, picks one queued PRD per tick, delegates Rust to `/autobuilder`, publishes the result. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/build-skill/main/install.sh \| bash`. |
| [dream-skill](https://github.com/j0yen/dream-skill) | — | Claude Code skill (`/dream`): vision into PRDs. Listens, researches, decomposes a vision into a fleet of PRD-sized pieces, gossips with `/build` via a shared channel. Runs overnight every 30 min. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/dream-skill/main/install.sh \| bash`. |

## Self-review / observability

| Repo | Binary | What it does |
|---|---|---|
| [binstale](https://github.com/j0yen/binstale) | `binstale` | Running-binary staleness detector: classifies each process's executing binary as `fresh \| deleted-exe \| inode-drift \| prov-stale` using `/proc` kernel signals and provfs xattrs. Detection only — never restarts anything. |
| [docket](https://github.com/j0yen/docket) | `docket` | SQLite-backed CLI ledger for standing findings — deduplicates recurring self-review discoveries by key, tracks first/last-seen timestamps and consecutive-run streak, exposes `report`/`list`/`show`/`resolve` commands. |

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
| [session-postmortem](https://github.com/j0yen/session-postmortem) | `session-postmortem` | One-command session forensics: joins memlog snapshots, provfs-attributed writes, recall memories, and ctrace events on `session_id` into a single markdown brief (who/intent/time/writes/learnings/cause-of-death). `--brief` collapses to 10 lines; closing tool of the continuity fleet. |
| [provq](https://github.com/j0yen/provq) | `provq` | File → session provenance query CLI. Reads `user.prov.*` xattrs stamped by provfs and answers "who wrote this file": `provq show <path>` decodes one file to JSON or table; `provq scan <dir> --since 1h --session <id>` walks a tree with predicates. |
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
| [cadence](https://github.com/j0yen/cadence) | Shared time-pyramid record store: `record`/`list`/`latest`/`register`/`where` over `~/.claude/cadence/<tier>/<period>/`. The composable substrate that lets the five reflective tools (daily-receipt, confidant, letters-we-never-sent, conversations-zine, memory-reliquary) look each other up by tier and period. Append-only, ULID-keyed. |
| [confidant](https://github.com/j0yen/confidant) | Weekly letter composer + e-ink PNG renderer (400×300) for a desk-side RPi Zero device. v0.2 binds to the cadence substrate: reads `daily` records as intake and writes a `weekly` record per emitted letter. |
| [conversations-zine](https://github.com/j0yen/conversations-zine) | Quarterly zine moment-extractor: pulls memorable lines from Claude conversation logs. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/conversations-zine/main/install.sh \| bash`. |
| [daily-receipt](https://github.com/j0yen/daily-receipt) | Deterministic Rust core for the Daily Receipt art project (one-day-on-a-page printable). One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/daily-receipt/main/install.sh \| bash`. |
| [daily-receipt-printer](https://github.com/j0yen/daily-receipt-printer) | Physical emitter for `daily-receipt`: pushes the ESC/POS byte stream to a MASUNG IP1000 at `/dev/usb/lp0` once per day via a systemd-user timer. Idempotent per-day delivery, `receipt status` JSON, no composition logic — writes bytes and gets out of the way. |
| [day-summarize](https://github.com/j0yen/day-summarize) | Upstream producer for `daily-receipt`: gathers ctrace + git + recall + journal + stamp signals into `summary.json` (canonical-ordered, byte-deterministic). Best-effort on missing tools; never panics. |
| [day-stamps](https://github.com/j0yen/day-stamps) | Special-day stamp catalog + lookup for `daily-receipt`: `day-stamp add\|list\|render\|which\|seed` over date-specific + recurring JSON stamps (pre-rendered ESC/POS bytes keyed by date). Read by `day-summarize` and `day-haiku`. |
| [day-haiku](https://github.com/j0yen/day-haiku) | The "art" half of `daily-receipt`: reads `day-summarize`'s `summary.json`, calls Claude (cached system + past-claude few-shot, ephemeral daily block) and writes a schema-guarded 3-line haiku into `content.json`. `--re-roll` veto path, fail-open exit codes so a bad response never breaks the nightly print. |
| [fsstory](https://github.com/j0yen/fsstory) | Filesystem attribution timeline: who-changed-what when a file changes. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/fsstory/main/install.sh \| bash`. |
| [letters-we-never-sent](https://github.com/j0yen/letters-we-never-sent) | Monthly draft-ritual aggregator over `~/.claude/letters/`. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/letters-we-never-sent/main/install.sh \| bash`. |
| [morsel](https://github.com/j0yen/morsel) | Embeddable inference primitives for tiny neural networks in Rust (Linear, Sigmoid, Tanh, ReLU, Softmax, LSTM, Argmax). Allocation-free, deterministic, safe-Rust. The library half of the morsel / morsel-bake pair; weights are baked into source by morsel-bake. |
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
| [wintermute-platform](https://github.com/j0yen/wintermute-platform) | `wmd-init`, `wm` | Autologin + systemd user target + Rust supervisor (wmd-init) and CLI (wm) — load-bearing scaffold under which the rest of Fleet 1 runs. |
| [wintermute-tts](https://github.com/j0yen/wintermute-tts) | `wm-tts` | Text-to-speech daemon: Piper CPU-local primary, ElevenLabs opt-in cloud streaming, agorabus-driven, voicepack-based. |
| [wintermute-stt](https://github.com/j0yen/wintermute-stt) | `wm-stt` | Speech-to-text daemon: whisper.cpp via whisper-rs (feature-gated), agorabus subscribe on `wm.audio.speech.*`, confidence-thresholded `wm.stt.{final,uncertain}` emits. |
| [wintermute-audio](https://github.com/j0yen/wintermute-audio) | `wm-audio` | Microphone pipeline daemon: PipeWire AEC + NoiseTorch + microWakeWord + Silero VAD; one canonical mic capture fans out PCM on UDS and publishes `wm.audio.{wake,speech.*,mute,unmute}` on agorabus. |
| [wintermute-dialog](https://github.com/j0yen/wintermute-dialog) | `wm-dialog` | Dialog FSM daemon: wake/barge-in/confirm/child-lock orchestration; subscribes `wm.audio.*`/`wm.stt.*`/`wm.brain.*`, publishes `wm.dialog.{tts.{speak,cancel},state,brain.request,confirm.{granted,denied},audio.{mute,unmute}}` with 200ms barge-in/mute budgets. |
| [wintermute-brain](https://github.com/j0yen/wintermute-brain) | `wmd` | Claude API conversation loop with recall-backed persistent memory; Sonnet 4.6 default + Opus 4.7 opt-in, prompt-cached profile + day thread, sub-10 ms recall retrieval, tool-router seam for Fleet 2, destructive-intent JSON gating through wm-dialog. |
| [wintermute-music](https://github.com/j0yen/wintermute-music) | `wm-music` | Fleet 2 action layer: voice-driven MPRIS player control (play/pause/next/prev/volume/now-playing) over D-Bus. Provider-agnostic remote control — drives Spotify, Rhythmbox, mpv, VLC; no in-process audio. Clean `no_player` contract when nothing's running. |
| [wintermute-calendar](https://github.com/j0yen/wintermute-calendar) | `wm-cal` | Fleet 2 action layer: voice-driven CalDAV calendar daemon; list/add/find/delete events, weekly RRULE expansion, SecretService credentials, NL time parsing, upcoming-event publish. |
| [wintermute-desktop](https://github.com/j0yen/wintermute-desktop) | `wm-desktop` | Fleet 2 action layer: AT-SPI accessibility tree reader + xdotool/baton keystroke injector giving the brain a read+act surface on the X11 desktop (apps/focus/read_window/click/type/key/find tools over agorabus). |
| [wintermute-browser](https://github.com/j0yen/wintermute-browser) | `wm-browser` | Fleet 2 action layer: voice-driven web browsing daemon driving a headed Chromium over CDP (chromiumoxide); open/read/click/type/back/find/screenshot tools over agorabus, a11y-snapshot read mode capped at 2000 refs, 5-min idle exit, crash-restart supervision. |
| [wintermute-screen-narrate](https://github.com/j0yen/wintermute-screen-narrate) | `wm-screen-narrate` | Fleet 2 action layer: screen-capture + Claude vision CLI; describe/read_text/find_in_image/screenshot tools over agorabus; image-mode fallback for the brain when the a11y tree is empty (canvas apps, video, Electron UIs). |
| [wintermute-mail](https://github.com/j0yen/wintermute-mail) | `wm-mail` | Fleet 2 action layer: voice-driven IMAP/SMTP mail daemon; inbox/read/send/search/mark-read/delete/folders over agorabus, async-imap + lettre (rustls), SecretService credentials, IMAP IDLE new-mail signal, verbal-confirm on destructive send/delete. |
| [wintermute-almanac](https://github.com/j0yen/wintermute-almanac) | `wm-almanac` | Local offline store of recurring routine entries for the elder (med/meal/appt/activity); daily/weekly/once recurrences, per-entry enable/disable, DST-correct next-due; no network, no bus. The schedule model every almanac-* PRD builds on. |
| [wm-verify](https://github.com/j0yen/wm-verify) | — | Pure in-process quality gate for local LLM answers: catches shape failures (empty, refusal, looping, wrong language, disclaimer, non-answer) before they reach the user. No network, no model call, deterministic. Library consumed by the brain backend ladder. |

## License

Each repo is dual-licensed MIT or Apache-2.0 at the user's option.
