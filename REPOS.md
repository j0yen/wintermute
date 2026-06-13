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
| [ac-judge](https://github.com/j0yen/ac-judge) | `ac-judge` | LLM-based semantic AC judge for the autobuilder pipeline: pairs each PRD acceptance criterion to its test, asks Claude Sonnet 4.6 whether the test actually exercises the AC's stated behavior, and emits a Stage-4 receipt (`ac-semantic-judge.json`). |
| [cradle](https://github.com/j0yen/cradle) | `cradle` | Self-trained-model pipeline: harvest labeled data from Claude transcripts, orchestrate train (Python shellout) + bake (morsel shellout, receipt-7 accuracy gate) into Rust crates via [morsel](https://github.com/j0yen/morsel). v0.1.1 ships full harvest → train → bake pipeline end-to-end. |
| [learning-db](https://github.com/j0yen/learning-db) (aka `database0`) | — | Educational, configurable DBMS — every subsystem (buffer pool, indexes, joins, MVCC) is a swappable implementation. Companion to CMU 15-445/645. TypeScript / pnpm workspace; see the repo README for install. |

## Agent runtime

| Repo | Binary | What it does |
|---|---|---|
| [agorabus](https://github.com/j0yen/agorabus) | `agorabus` | Single-host advisory pub/sub bus for concurrent Claude sessions (UDS, announce/publish/subscribe/heartbeat). One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/agorabus/main/install.sh \| bash`. |
| [agent-pipe](https://github.com/j0yen/agent-pipe) | `apipe` | Streaming NDJSON record pipeline + shared schema for composing agent tools (`pass`/`top`/`sort`/`filter`/`pretty`). One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/agent-pipe/main/install.sh \| bash`. |
| [agentsh](https://github.com/j0yen/agentsh) | — | zsh plugin that flips agent-hostile defaults under `$CLAUDE_TOOL` (per-session HISTFILE, NOMATCH, NO_UNSET, no aliases). |
| [agentns-claude](https://github.com/j0yen/agentns-claude) | `agentns-claude` | Wrap a command (typically `claude`) in a wintermute agent namespace so `/proc/$PID/agent_session` reads stably from session birth. Mock mode + `--no-unshare` fall back to userspace session-id synthesis on stock kernels; live unshare/budget/intent-tag plumbing activates under `linux-wintermute`. |
| [agentns-doctor](https://github.com/j0yen/agentns-doctor) | `agentns-doctor` | Read-only CLI that classifies a process's agent-namespace state from `/proc` surfaces into `absent`/`init`/`live`/`malformed`; ends the mis-diagnosis of healthy init-ns zeros as "broken kernel." |
| [christen](https://github.com/j0yen/christen) | `christen` | Launch-site model and route plan for agent-namespace wiring: `LaunchSite`, `SiteKind`, `WrapState`, `RouteAction`, `RoutePlan` types + `LaunchSiteSource` trait + `FakeSource` + pure `plan()` + `christen plan` CLI. Foundational corpus for the christen fleet (christen-detect / christen-route / christen-cap / christen-ledger). |
| [baton](https://github.com/j0yen/baton) | `baton` | Cross-window claude delegation primitive: address a visible target session by surface and type a prompt into it (xdotool/tmux). |
| [mcp-core](https://github.com/j0yen/mcp-core) | (library) | Reusable JSON-RPC 2.0 stdio MCP-server core: `Tool` trait + `serve_stdio` loop. Extracts the wire layer shared by all wintermute MCP servers into one crate (`proto` / `tool` / `dispatch` / `serve`). |
| [mcp-autotuner](https://github.com/j0yen/mcp-autotuner) | — | Cost-aware MCP tool-set tuner that prunes rarely-used tools to keep context tight. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/mcp-autotuner/main/install.sh \| bash`. |
| [muster-mcp](https://github.com/j0yen/muster-mcp) | `muster-mcp` | Live-session census as a read-only MCP server — exposes `sessions_census` and `sessions_verdict` over MCP stdio; `reap` intentionally excluded. |
| [provenance-mcp](https://github.com/j0yen/provenance-mcp) | `provenance-mcp` | kernel-stamped file provenance and context snapshots over MCP — exposes `file_provenance` (provfs xattrs) and `memlog_recent` (memlog circular log) as read-only tools; gracefully degrades when wintermute kernel features are absent. |
| [skill-doctor](https://github.com/j0yen/skill-doctor) | `skill-doctor` | Walks `~/.claude/skills/*/SKILL.md`, extracts shell invocations, and cross-references each flag/subcommand against the `tool-manifest` JSON; parks review-gated drift proposals at `~/.claude/skill-doctor/proposals/<ULID>.md` (no auto-edit). |
| [skill-manifest](https://github.com/j0yen/skill-manifest) | `skill` | Validator for the optional SKILL.md `manifest:` block (parser + JSON schema + CLI). One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/skill-manifest/main/install.sh \| bash`. |
| [skill-telemetry](https://github.com/j0yen/skill-telemetry) | `spool` | Per-skill invocation log with monthly JSONL buckets, rank + stale reports. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/skill-telemetry/main/install.sh \| bash`. |
| [tool-manifest](https://github.com/j0yen/tool-manifest) | `tool-manifest` | Probes installed binaries' `--help` surface into a structured JSON manifest — ground truth for skill-doctor and Fleet 2 drift checks. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/tool-manifest/main/install.sh \| bash`. |
| [build-skill](https://github.com/j0yen/build-skill) | — | Claude Code skill (`/build`): continuous PRD implementation loop. Runs every 5 min via systemd-user timer, picks one queued PRD per tick, delegates Rust to `/autobuilder`, publishes the result. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/build-skill/main/install.sh \| bash`. |
| [dream-skill](https://github.com/j0yen/dream-skill) | — | Claude Code skill (`/dream`): vision into PRDs. Listens, researches, decomposes a vision into a fleet of PRD-sized pieces, gossips with `/build` via a shared channel. Runs overnight every 30 min. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/dream-skill/main/install.sh \| bash`. |
| [wm-skill-edit](https://github.com/j0yen/wm-skill-edit) | `wm-skill-edit` | Allow-listed wrapper for anchored idempotent SKILL.md edits — escapes the self-modification classifier for `/build` branch agents. Guarded insert-after-anchor + revert; `Bash(wm-skill-edit:*)` allow rule replaces raw `Edit` calls on skill files. |

## Self-review / observability

| Repo | Binary | What it does |
|---|---|---|
| [anchor](~/wintermute/anchor) | `anchor` | Declared watch-root manifest and pure reconcile plan: `anchor plan` diffs a versioned `roots.toml` against the live watchman state and prints which roots are watched/missing/stale/undeclared; exits non-zero if any declared root is missing. Foundation for anchor-probe/anchor-reconcile/anchor-boot. |
| [bpolicy](https://github.com/j0yen/bpolicy) | `bpolicy` | eBPF-LSM write enforcer with versioned home: Rust control-plane CLI replacing the original Python script; `load`/`unload`/`enforce`/`release`/`status`/`log` subcommands with byte-identical JSON output; BPF source vendored in-repo; back-compat anchor for the warden fleet. |
| [binstale](https://github.com/j0yen/binstale) | `binstale` | Running-binary staleness detector: classifies each process's executing binary as `fresh \| deleted-exe \| inode-drift \| prov-stale` using `/proc` kernel signals and provfs xattrs. Detection only — never restarts anything. |
| [ctrace-orphan-reap](https://github.com/j0yen/ctrace-orphan-reap) | `ctrace-orphan-reap` | Reconcile orphaned ctrace tracer state against live PIDs: classifies into `healthy`/`orphaned-tracer`/`stale-marker`/`no-tracer`; with `--apply` stops the orphan and renders its log. Read-only by default. |
| [ctrace-scribe](https://github.com/j0yen/ctrace-scribe) | `ctrace-scribe` | Cross-session daily trace digest: reads ctrace JSON logs and emits per-tool/per-session summaries; `rollup` subcommand aggregates across sessions for self-review. |
| [docket](https://github.com/j0yen/docket) | `docket` | SQLite-backed CLI ledger for standing findings — deduplicates recurring self-review discoveries by key, tracks first/last-seen timestamps and consecutive-run streak, exposes `report`/`list`/`show`/`resolve` commands. |
| [keel](https://github.com/j0yen/keel) | `keel` | Brain tier-ladder health probe: `keel status` reports the effective tier ceiling and how long the brain has been floored; `keel beacon` emits `wm.keel.degraded`/`wm.keel.refloat` on ceiling changes (edge-triggered, not a heartbeat) so the operator reacts live instead of at the next self-review. |
| [rollout](https://github.com/j0yen/rollout) | `rollout` | Safe rolling restart for the live daemon fleet: `rollout install <bin> --dest <path>` atomically installs a freshly-built binary and restarts the owning systemd-user unit (agorabus-reload path or systemctl); `plan`/`apply` subcommands for bulk stale-binary remediation driven by binstale JSON. Closes the install-without-restart gap for recalld, wmd, wm-audio, and the voice fleet. |
| [quicken](https://github.com/j0yen/quicken) | `quicken` | Wintermute kernel primitive liveness checker: `quicken probe` classifies memlog/agentns/warden/provfs as `Live \| LiveDegraded \| InstalledNotActivated \| StagedNotInstalled \| Inert \| Unknown` with structured evidence. Sibling of `binstale` (stale-but-running axis) for the never-activated axis. `--json` output usable as a pipeline gate. |
| [wm-hardware-drift](https://github.com/j0yen/wm-hardware-drift) | `wm-hardware-drift` | Sweep CLI that runs both mock and `--features=real-hardware` cargo test sets, diffs per-test outcomes, and emits a `hardware-drift.json` receipt; `/self-review` surfaces any `drift_count > 0` as a finding. |

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
| [continuity-attest](https://github.com/j0yen/continuity-attest) | `continuity-attest` | Capstone e2e continuity attestation CLI. Drives a probe child via `agentns-claude`, then asserts provfs, recall, memlog, and ctrace signals all agree on the ground-truth session id `S`. Emits `CONTINUITY: ATTESTED (id=S)` or `CONTINUITY: BROKEN at <signal>`; writes durable receipt to `~/brain/continuity/`. |
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
| [daily-receipt-yearend-letter](https://github.com/j0yen/daily-receipt-yearend-letter) | Year-end thermal strip for the daily-receipt ritual: composes a ~200–400 word letter in the past-Claude / future-Claude voice from the year's cadence records, renders to ESC/POS + PNG, with idempotent caching and a systemd-user timer firing at Dec 31 23:55. The scroll's annual punctuation. |
| [fsstory](https://github.com/j0yen/fsstory) | Filesystem attribution timeline: who-changed-what when a file changes. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/fsstory/main/install.sh \| bash`. |
| [letters-we-never-sent](https://github.com/j0yen/letters-we-never-sent) | Monthly draft-ritual aggregator over `~/.claude/letters/`. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/letters-we-never-sent/main/install.sh \| bash`. |
| [the-lunch](https://github.com/j0yen/the-lunch) | Foundation library for round-table creative artifact gathering: defines `Table`/`Dish` types and adapters that pull today's creative artifacts (day-haiku, conversations-zine, letters-we-never-sent, self-portrait, ambient) onto a unified, serialized structure. |
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
| [wm-router](https://github.com/j0yen/wm-router) | — | Conservative utterance classifier for the companion loop: classifies one transcribed utterance into `Skill(id)` / `CacheLookup` / `Brain{stakes}` using deterministic rules then recall's embed RPC; safety stage runs first, tuned for high recall on medication/medical/emergency/distress/money turns. Library crate — the integration spine the thrift tiers plug into. |
| [wm-semcache](https://github.com/j0yen/wm-semcache) | — | Embedding-keyed semantic response cache: returns cached answers for near-duplicate utterances (cosine ≥ threshold) with zero API cost; cache-unsafe gate blocks stale time/weather/calendar answers from ever being served. |
| [wm-local-llm](https://github.com/j0yen/wm-local-llm) | — | OpenAI-compatible local LLM client (ollama/llama-server/llamafile); infallible `generate` always returns Answer or Escalate-to-Sonnet; streaming DeltaSink for TTS, zero weights vendored, zero live-network in tests. |
| [wintermute-reach](https://github.com/j0yen/wintermute-reach) | `wm-reach` | Off-device transport boundary: subscribes to `wm.family.message`/`wm.family.distress`, delivers via email (default)/ntfy/webhook, acks delivery back on the bus, provides v1 inbound-reply stub. Closes the `wm.family.ack` loop for family-intents. |
| [wintermute-family-enroll](https://github.com/j0yen/wintermute-family-enroll) | `wm-family` | Caregiver setup wizard (kin capstone): writes `/etc/wintermute/conf.d/50-family.env` atomically; setup/show/announce subcommands; privacy-opt-in defaults (presence/silence/digest=off, distress=on); the consent ceremony every other kin service reads from. |
| [wintermute-presence](https://github.com/j0yen/wintermute-presence) | `wm-presence` | Privacy-first presence heartbeat daemon: emits `wm.presence.summon` on each interaction (count only, never text) and `wm.presence.silence` when no interaction falls in the configured waking-hours window. Default OFF; enrolls via `wm-family`. |
| [wintermute-lucid](https://github.com/j0yen/wintermute-lucid) | `wm-lucid` | Flight recorder for the agorabus bus: subscribes to the full `wm.` prefix, persists every event to a rotating turn-keyed NDJSON log under `~/.cache/wintermute/lucid/`, ships `lucid tap/trace/mind/explain/watch` subcommands for live tailing and post-hoc turn reconstruction. |

## Constellation fleet

Multi-machine expansion of the wintermute ecosystem — fleet provisioning, mesh networking, and coordinated deployment.

| Repo | Binary | Description |
|------|--------|-------------|
| [constellation](~/wintermute/constellation) | — | Fleet provisioning repo: Ansible playbooks (base/desktop/voice roles), greetd autologin + i3 graphical-session bridge, host_vars per-node config, isobuild archiso profile, and localrepo scripts — one ISO + one playbook run = an identical wintermute node. |
| [agorabus-nats-bridge](https://github.com/j0yen/agorabus-nats-bridge) | `wm-busbridge` | agorabus ↔ NATS bridge daemon: mirrors allowlisted `wm.fleet.*` events between the local agorabus UDS and a NATS leaf node, loop-guarded and bandwidth-selective — the keystone that makes the local wintermute bus fleet-wide. |
| [constellation-burst-builder](https://github.com/j0yen/constellation-burst-builder) | `wm-burst` | Mesh-free cloud-burst CLI: points `cargo` at a cheap dedicated box + shared sccache so heavy compiles stop pinning local cores; hard-fails on toolchain drift; enforces monthly pod budget cap. |
| [wake-train](~/wintermute/wake-train) | `burst-train.sh` | Wake-word retrain offloaded to an on-demand GPU burst pod (via `wm-burst`), then gated: installs the returned ONNX into wm-audio only if it is exactly `[1,186,40]→[1,1]`, non-streaming, and passes the local `verify` stage — atomic swap with one-command `--rollback`. Replaces the OOM-prone local retrain. |

## Relay (human services)

On-device human-services resource directory — ingests HSDS JSON exports and 211-style CSVs, normalises into a local SQLite store, answers proximity + eligibility queries offline.

| Repo | Binary | What it does |
|---|---|---|
| [relay](~/wintermute/relay) | `relay` | Foundation workspace: normalised `Resource` schema, HSDS-JSON + CSV ingestors, dedup, SQLite store, proximity+eligibility query layer; `relay directory import/query/stats`. |

## Homeward (lost-pet)

| Repo | Binary | What it does |
|---|---|---|
| [homeward](https://github.com/j0yen/homeward) | `homeward` | A dozen heterogeneous sources describe the same thing — a dog or cat in a shelter |

## Concord (argument analysis)

Perspective-diverse source gathering and stance-tagged corpus for contested claims.

| Repo | Binary | What it does |
|---|---|---|
| [concord](https://github.com/j0yen/concord) | `concord` | Gather perspective-diverse sources for a contested claim, tag by stance, dedup near-identical framings, score credibility, emit a structured Corpus — foundation of the concord workspace. |
| [ousia-forge](https://github.com/j0yen/ousia-forge) | `ousia-forge` | Build the World Ontology (OWL 2 DL / RDF/XML) from a declarative TOML spec; subcommands: build, check, stats. Gate tool for the full ousia toolchain. |
| [ousia-sparql](https://github.com/j0yen/ousia-sparql) | `ousia-sparql` | SPARQL 1.1 query layer over the materialized World Ontology: load OWL + ABox into an oxigraph store, materialize entailments via ousia-reason, and query with `load`/`query`/`ask`/`serve`; ships a canned pack (dignity-bearers, rights-violations, unaccountable-authority, just-societies, etc.) that turns the ethical structure into runnable demo queries. |
| [ousia-atscale](https://github.com/j0yen/ousia-atscale) | `ousia-atscale` | BFO grounding bridge for the AtScale semantic layer: maps AtScale model elements (measures, dimensions, column-groups) onto BFO 2020 categories and emits annotation overlays with §4.4 vocabulary (philosophicalGrounding, domainModule, aristotelianDefinition). Subcommands: ground, annotate, report. Offline JSON path; MCP live path gated. |
| [ousia-mcp](https://github.com/j0yen/ousia-mcp) | `ousia-mcp` | MCP server exposing ousia-reason, ousia-sparql, and ousia-guard as five read-only tools (classify, query, ask_canned, guard_check, explain) over stdio; ontology loaded once at startup, answers from in-memory reasoned store. |
| [recourse](https://github.com/j0yen/recourse) | `recourse` | Durable, PII-free verdict receipt layer for ousia-guard: `receipt emit` hashes the action (blake3, never raw), appends NDJSON receipt; `show`/`ls` for audit and pipeline integration. First crate in the verdict-recourse chain. |
| [tribunal](https://github.com/j0yen/tribunal) | `tribunal` `tribunal-bench` `tribunal-gate` | Ethics engine conformance, bench, and gate harness. `tribunal conformance` checks OWL 2 DL compliance, single-inheritance, TBox-only, and 10-axiom encoding; `tribunal-bench` runs ousia-guard over the 64-case corpus with confusion matrix and false-allow detection; `tribunal-gate` composes both into a publish precondition (conformance + accuracy + zero-false-allow). |
| [lattice-registry](https://github.com/j0yen/lattice-registry) | `lattice-registry` | Local catalog of BFO-grounded ontologies for the ethical lattice: fetch, verify BFO grounding, and index ontologies from OBO Foundry and other registries. `sync`/`add`/`list`/`show`/`path` with etag-based caching under `~/.cache/lattice/registry/`. |
| [lattice-ground](https://github.com/j0yen/lattice-ground) | `lattice-ground` | Ground natural-language mentions to BFO-anchored ontological classes across the federated lattice. `resolve` ranks candidates with lexical+context scoring and surfaces cross-ontology bridge consequences (patient→dignity); `link` expands a known IRI's bridge/subsumption neighborhood. Output is structured JSON consumable by `ousia-guard`. |

## Warrant suite

Close-claim corpus reader and assertion runner — parses, classifies, and re-verifies PRD close notes so false "outcome achieved by a different mechanism" closes are caught before the symptom recurs from scratch.

| Repo | Binary | What it does |
|---|---|---|
| [warrant](https://github.com/j0yen/warrant) | `warrant` | Root workspace: close-claim domain model (`CloseClaim`, `ClaimKind`, `Warrant`, `AssertionSpec`, `WarrantVerdict`), pure `classify()` function (zero IO), `CloseSource` trait + `FakeSource`; `warrant list [--format json]` and `warrant list-sources` CLI. Foundation for warrant-audit and warrant-docket. |

## Answerable suite

Semantic audit trail for autonomous agent actions — an append-only human-readable ledger and guard layer.

| Repo | Binary | What it does |
|---|---|---|
| [answerable](https://github.com/joeyen-atscale/answerable) | `answerable` | Append-only JSONL ledger of high-consequence autonomous actions (`record`/`log`/`stats`). Library API (`Action`, `ActionKind`, `Ledger::append/read/since`) for sibling PRDs. SIGPIPE-safe; O_APPEND atomic writes. |

## License

Each repo is dual-licensed MIT or Apache-2.0 at the user's option.

## Recently shipped (auto)

| Repo | Binary | What it does |
|---|---|---|
| [conning-tower](https://github.com/j0yen/conning-tower) | `conning-tower` | `vicious-circle` crowns a `bon mot` each round and writes every `Verdict` to an |

| Repo | Binary | What it does |
|---|---|---|
| [ousia-guard](https://github.com/j0yen/ousia-guard) | `ousia-guard` | This is the keystone of the seed — "making ethical AI possible." `ousia-guard` |

| Repo | Binary | What it does |
|---|---|---|
| [herald-pack](https://github.com/j0yen/herald-pack) | `herald-pack` | On this laptop, a "skill" is a `SKILL.md` directory symlinked into |

| Repo | Binary | What it does |
|---|---|---|
| [assay](https://github.com/j0yen/assay) | `assay` | The booted `7.0.10-arch1-5-wintermute` kernel reports `agent_session` as 32 |

| Repo | Binary | What it does |
|---|---|---|
| [herald-market](https://github.com/j0yen/herald-market) | `herald-market` | Generates and maintains a `.claude-plugin/marketplace.json` catalog for j0yen plugins — `init`/`add`/`sync`/`lint` subcommands, pinned `git-subdir` shas, matches official Claude Code marketplace format. |
| [bon-mot](https://github.com/j0yen/bon-mot) | (library) | Shared wit core for bon-mot CLIs: TOML lexicon, `{slot}`/`{a\|b\|c}` grammar runtime, Unicode counters, seedable RNG, optional Anthropic client (`lavish` feature). |
| [coda](https://github.com/j0yen/coda) | `coda` | Summary-debt model and sweep planner for ctrace session logs; `coda plan` classifies every session log (Active/Fresh/Orphaned/Settled) and prints a debt table; exits non-zero when render work exists. |
| [relay](https://github.com/j0yen/relay) | `relay` | A helper doesn't think in taxonomy codes — they hear "she's couch-surfing with |
