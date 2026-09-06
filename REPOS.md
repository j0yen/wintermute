# Wintermute ecosystem repos

The wintermute project is a collection of small Rust CLIs and supporting
tooling for running Claude Code agents locally. Each piece is now its
own GitHub repo; this index lists them with one-line descriptions.

The [`bootstrap/install.sh`](bootstrap/install.sh) script in this repo
clones, builds, and wires up everything on a fresh machine.

## Pipeline / meta

| Repo | Binary | What it does |
|---|---|---|
| [consign](https://github.com/j0yen/consign) | `consign` | Accurate fleet push-debt enumerator: walks every wintermute git repo and classifies push-debt into named buckets (clean/ahead/no-upstream/no-remote/diverged); fixes the systematic undercount in self-review that silently dropped repos with no upstream tracking branch. |
| [atlas](https://github.com/j0yen/atlas) | `atlas` | Queryable node graph of the wintermute PRD corpus: parse every PRD's frontmatter + both skill manifests + REPOS.md into typed vision/prd/repo nodes; `atlas nodes` + `atlas show <vision>` with `--format json`. Read-only; ~23 ms cold run over 100+ PRDs. |
| [autobuilder](https://github.com/j0yen/autobuilder) | `autobuilder` | Claude Code skill + Rust companion binary that turns a PRD into a vetted Rust project — intent-cards, iterate-and-prove loop, 25-receipt release gate. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/autobuilder/main/skill/install.sh \| bash`. |
| [autobuilder-metric-harness](https://github.com/j0yen/autobuilder-metric-harness) | — | Unfakeable-scalar metric collector the autobuilder loop polls each iteration. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/autobuilder-metric-harness/main/install.sh \| bash`. |
| [ac-judge](https://github.com/j0yen/ac-judge) | `ac-judge` | LLM-based semantic AC judge for the autobuilder pipeline: pairs each PRD acceptance criterion to its test, asks Claude Sonnet 4.6 whether the test actually exercises the AC's stated behavior, and emits a Stage-4 receipt (`ac-semantic-judge.json`). |
| [cradle](https://github.com/j0yen/cradle) | `cradle` | Self-trained-model pipeline: harvest labeled data from Claude transcripts, orchestrate train (Python shellout) + bake (morsel shellout, receipt-7 accuracy gate) into Rust crates via [morsel](https://github.com/j0yen/morsel). v0.1.1 ships full harvest → train → bake pipeline end-to-end. |
| [learning-db](https://github.com/j0yen/learning-db) (aka `database0`) | — | Educational, configurable DBMS — every subsystem (buffer pool, indexes, joins, MVCC) is a swappable implementation. Companion to CMU 15-445/645. TypeScript / pnpm workspace; see the repo README for install. |
| [vibecode-kit](https://github.com/j0yen/vibecode-kit) | — | Portable Claude Code skill kit: `/dream` (PRD fleets from evidence), `/build` (Python-only reference router), `/vibeloop` (one orient→dream→build→digest cycle with multi-action and parallel build modes), plus `pybuilder`; installs via `install.sh`. Markdown + Python. |
| [synthorg](https://github.com/j0yen/synthorg) | `synthorg` | Synthetic-consumer harness for mcphost: researched panel personas exercise the live or fake endpoint (`consume --measure`, proxy/truth tiers), score sessions on timeliness/accuracy/helpfulness into measure.json, golden replays pin the scorer, `lift` compares runs. Python. |

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
| [mcphost](https://github.com/j0yen/mcphost) | `mcphost` | Streamable-HTTP MCP host: `mcphost serve` lets an agent sign up with one unauthenticated call, get a tenant key, then publish/list/call its own tools (`host.*` control plane, `Kind` trait + built-in `echo` kind) with no human step; admin tools meter and manage tenants over the same endpoint. |
| [mcphost-deploy](https://github.com/j0yen/mcphost-deploy) | `mcphost-deploy` | Puts the `mcphost` binary on a Hetzner box as a systemd service behind Caddy in one command (`install`); `redeploy` ships a new version and rolls back automatically on a failed `probe`; `probe`/`backup`/`logs` round out the ops surface. Python/typer. |
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
| [fleet-ctl](https://github.com/j0yen/fleet-ctl) | `fleet` | One command answers "what state is the fleet in": `fleet status` renders one row per node (reachable, disk %, load, live Claude sessions, drift) by probing every node concurrently under a per-node 5 s deadline, and `fleet probe <node> <probe>` runs one named read-only probe (`disk`/`load`/`sessions`/`units`/`locks`/`drift`/`leaks`) — `locks` finds orphaned flock holders under `~/wintermute` with pid/ppid/age, `leaks` names the fleet's known leak signatures. `--format json` on everything; exit 0/1/2 so timers can gate on it. Rides existing Tailscale ssh + muster/fleet-sync; no daemon, no agent install; v0.1 mutates nothing. |
| [fleet-janitor](https://github.com/j0yen/fleet-janitor) | `janitor` | Hourly patrol that reaps known leaked-process signatures within hard guardrails, so a leaked `bwrap` test sandbox or a foreign-lock-holding `sccache` daemon gets cleared before it stalls a gate for hours, not found by hand. `scan` evaluates the checked-in signature table (bwrap-test-sandbox, sccache-foreign-lock, root-tracer, stale-wm-build-session-dir) and reports evidence; `sweep` reaps only where signature-allowlist + orphan proof (`PPID==1`, re-verified immediately before the kill) + age floor all hold, no override flag; `log --since` renders the append-only `~/brain/state/janitor.jsonl` audit trail. Real `flock` concurrency guard; ships as an hourly systemd user timer gated by `wm-node should-run`. |
| [muster](~/wintermute/muster) | `muster` | v0.8.0 — Process census, verdict, and reap for the wintermute fleet's Claude sessions. `census` enumerates live Claude processes with origin attribution and agorabus subtree info (`sub=N work=M dead=K`); `verdict` classifies each session as live/duplicate/orphan/stale, ranks same-slug duplicates by activity recency, and annotates sessions with `subtree-rot` (deleted-exe or surplus worker generations); `reap` proposes or executes (--confirm) cleanup of orphan/stale roots and, with `--subtree`, the rotten children of live sessions (grace-gated, never targets session root). |
| [chaff](https://github.com/j0yen/chaff) | `chaff` | Honest tracked-build-artifact enumerator: walks wintermute git repos, identifies tracked build junk (`target/`, `node_modules/`, `.venv/`, etc.), classifies each repo by strain (no-gitignore, gitignore-stale, gitignore-gap), and reports byte estimates. |
| [adopt](https://github.com/j0yen/adopt) | `adopt` | Detect shipped wintermute artifacts that never entered the live system: `adopt scan` walks every wintermute repo, checks PATH/~/.local/bin/~/.cargo/bin, and emits per-artifact verdicts (`not-installed`/`installed-stale`/`installed-current`) with copy-pasteable `fix_cmd`. Fills the gap binstale can't cover (binaries never installed ≠ running stale). `--format json` for pipeline use. |
| [anchor](~/wintermute/anchor) | `anchor` | Declared watch-root manifest and pure reconcile plan: `anchor plan` diffs a versioned `roots.toml` against the live watchman state and prints which roots are watched/missing/stale/undeclared; exits non-zero if any declared root is missing. Foundation for anchor-probe/anchor-reconcile/anchor-boot. |
| [bpolicy](https://github.com/j0yen/bpolicy) | `bpolicy` | eBPF-LSM write enforcer with versioned home: Rust control-plane CLI replacing the original Python script; `load`/`unload`/`enforce`/`release`/`status`/`log` subcommands with byte-identical JSON output; BPF source vendored in-repo; back-compat anchor for the warden fleet. |
| [binstale](https://github.com/j0yen/binstale) | `binstale` | Running-binary staleness detector: classifies each process's executing binary as `fresh \| deleted-exe \| inode-drift \| prov-stale` using `/proc` kernel signals and provfs xattrs. Detection only — never restarts anything. |
| [drydock-survey](https://github.com/j0yen/drydock-survey) | `drydock-survey` | v0.4.0. Drift inventory + classification + auto-lane executor + convergence ledger. Subcommands: `survey` (collect from binstale/adopt/kernel-probe, emit JSON or table), `classify` (route each item to a lane: auto/window/reboot/approval with hard floors for voice daemons and kernel-pkgs; `--explain <item>`), `apply` (drain auto-lane only, dry-run by default; `--apply` to execute; refuses when a build is in-flight), `ledger record/check/escalate/delta` (append-only JSONL at `~/.local/share/drydock/ledger.jsonl`; carries `first_seen` forward, exits non-zero on auto-lane regression). |
| [drydock-digest](https://github.com/j0yen/drydock-digest) | `drydock-digest` | Ranked, escalation-flagged digest renderer for drydock-classify output: groups items by lane (auto → window → reboot → approval), sorts by `age_days` descending, flags items older than `--escalate-days` (default 7) with `!`, shows per-lane Δ when a ledger is provided. Outputs terminal table, `--format md` for self-review paste, or `--format json`. |
| [ctrace-orphan-reap](https://github.com/j0yen/ctrace-orphan-reap) | `ctrace-orphan-reap` | Reconcile orphaned ctrace tracer state against live PIDs: classifies into `healthy`/`orphaned-tracer`/`stale-marker`/`no-tracer`; with `--apply` stops the orphan and renders its log. Read-only by default. |
| [ctrace-scribe](https://github.com/j0yen/ctrace-scribe) | `ctrace-scribe` | Cross-session daily trace digest: reads ctrace JSON logs and emits per-tool/per-session summaries; `rollup` subcommand aggregates across sessions for self-review. |
| [docket](https://github.com/j0yen/docket) | `docket` | SQLite-backed CLI ledger for standing findings — deduplicates recurring self-review discoveries by key, tracks first/last-seen timestamps and consecutive-run streak, exposes `report`/`list`/`show`/`resolve` commands. |
| [keel](https://github.com/j0yen/keel) | `keel` | Brain tier-ladder health probe: `keel status` reports the effective tier ceiling and how long the brain has been floored; `keel beacon` emits `wm.keel.degraded`/`wm.keel.refloat` on ceiling changes (edge-triggered, not a heartbeat) so the operator reacts live instead of at the next self-review. |
| [rollout](https://github.com/j0yen/rollout) | `rollout` | Safe rolling restart for the live daemon fleet: `rollout install <bin> --dest <path>` atomically installs a freshly-built binary and restarts the owning systemd-user unit (agorabus-reload path or systemctl); `plan`/`apply` subcommands for bulk stale-binary remediation driven by binstale JSON. Closes the install-without-restart gap for recalld, wmd, wm-audio, and the voice fleet. |
| [quicken](https://github.com/j0yen/quicken) | `quicken` | Wintermute kernel primitive liveness checker: `quicken probe` classifies memlog/agentns/warden/provfs as `Live \| LiveDegraded \| InstalledNotActivated \| StagedNotInstalled \| Inert \| Unknown` with structured evidence. Sibling of `binstale` (stale-but-running axis) for the never-activated axis. `--json` output usable as a pipeline gate. |
| [plumb](~/wintermute/plumb) | `plumb` | Probe-oracle calibration tool: `plumb check <probe-id>` pairs each self-review probe with an independent ground-truth oracle, reports `agree\|disagree\|error`. Catches lying instruments before they park wrong findings on the docket. Seed probes: `memlog-active` (reproduces the live SKILL.md multiline-capture bug), `ctrace-backfill-wired`, `adopt-report-exists`. |
| [wm-hardware-drift](https://github.com/j0yen/wm-hardware-drift) | `wm-hardware-drift` | Sweep CLI that runs both mock and `--features=real-hardware` cargo test sets, diffs per-test outcomes, and emits a `hardware-drift.json` receipt; `/self-review` surfaces any `drift_count > 0` as a finding. |
| [changeover](https://github.com/j0yen/changeover) | `changeover` | Measure the agorabus restart deafness window: `changeover probe --daemon <name>` triggers a `systemctl --user restart`, publishes a heartbeat stream, and reports `deafness_ms` + `events_missed_window`. `--dry-run` gives a synthetic offline report; `--format json` for pipeline use. |
| [tokenmeter](https://github.com/j0yen/tokenmeter) | `tokenmeter` | Per-tool token cost estimator for Claude Code sessions: parses session JSONL transcripts and estimates input/output token cost by tool name. `summary`/`session <id>`/`top --n N` subcommands; `--json` output; configurable pricing via `--input-price`/`--output-price`. |
| [memlog-capture-selfcheck](https://github.com/j0yen/memlog-capture-selfcheck) | `memlog-capture-selfcheck` | Cross-references the PreCompact writer's firing log against the memlog ring write count to detect the "firing-but-empty" failure class. Emits GREEN/AMBER/RED/MISSING verdicts; exits 3 on RED so self-review Phase B.5 can gate on it; `--docket` line for standing-findings ingestion. |
| [trim](https://github.com/j0yen/trim) | `trim` | Honest memory & swap pressure enumerator: walks `/proc/meminfo`, every `/proc/<pid>/status`, cgroup-v2 memory surfaces, and `/proc/pressure/memory` to attribute resident and swapped bytes to process and cgroup. `trim survey` ranked human table or `--format json`; `--by swap\|rss --top N`. |
| [fleet-beacon](https://github.com/j0yen/fleet-beacon) | `beacon`, `beacon-retain` | Per-node hourly heartbeat publisher + fleet-liveness digest over agorabus, so a stalled loop or a quiet node surfaces at the next session start instead of when someone happens to look. `beacon pulse` gathers disk/load/janitor/per-loop progress-file age and publishes to `fleet.beacon.<node>` (still writes local state and reports the failure if the bus is down); `beacon digest [--brief\|--format json]` reports quiet nodes (placement-aware for may-be-off nodes), stalled loops, and missing progress files, exit 1 on any non-suppressed finding; `beacon history <node>` renders its 48h ring. `beacon-retain` is the hub-side subscriber keeping a latest-per-node cache so digest reads all nodes in one fetch instead of per-node ssh. SessionStart hook wiring and live hub deployment of `beacon-retain` are open follow-on work. |

## Memory layer

| Repo | Binary | What it does |
|---|---|---|
| [recall](https://github.com/j0yen/recall) | `recall` | Local-first agentic memory for Claude Code: file-backed memories, FTS5 + semantic (BGE-small/fastembed) hybrid index, four hook scripts that wire the braid correlator into a live session. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/recall/main/install.sh \| bash`. |
| [recall-doctor](https://github.com/j0yen/recall-doctor) | `recall-doctor` | Health checker for the recall store (fsck for memories). One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/recall-doctor/main/install.sh \| bash`. |
| [recall-io](https://github.com/j0yen/recall-io) | `recall-io` | Frontmatter parser + serializer used as the memory file I/O contract. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/recall-io/main/install.sh \| bash`. |
| [recall-ops](https://github.com/j0yen/recall-ops) | `recall-ops` | Bulk ops over the recall store (move/relabel/dedupe). One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/recall-ops/main/install.sh \| bash`. |
| [recall-memory-linter](https://github.com/j0yen/recall-memory-linter) | `recall-lint` | Style + structure linter for individual memory files. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/recall-memory-linter/main/install.sh \| bash`. |
| [memory-reliquary](https://github.com/j0yen/memory-reliquary) | — | Annual book-of-memories renderer; pulls from recall, lays out a printable artifact. One-liner install: `curl -fsSL https://raw.githubusercontent.com/j0yen/memory-reliquary/main/install.sh \| bash`. |

## Wiki / vault

| Repo | Binary | What it does |
|---|---|---|
| [summa](https://github.com/j0yen/summa) | `summa` | CLI mechanics layer for an LLM-maintained wiki on Obsidian vaults: `ingest` (PDF/URL/MD → JSON stub), `index` (regenerate index.md between anchors), `log` (append-only timeline), `links` (orphan/dangling/malformed wikilink graph), `page` (mint entity/source-summary/answer pages). The `/summa` skill calls these for mechanics and adds LLM synthesis on top. |

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
| [threshold](https://github.com/j0yen/threshold) | `threshold` | Session arrival briefing synthesizer — one prioritized briefing from ten raw hooks. |

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

## Corpus (self-state coherence)

| Repo | Binary | What it does |
|---|---|---|
| [corpus-attest](https://github.com/j0yen/corpus-attest) | `corpus-attest` | Fleet membership attestation — proves a node is a legitimate limb of the self via Ed25519 signing; `whoami`, `enroll`, `verify`, `present` subcommands; SKIP-honest not-enrolled behavior. Root of the corpus dependency graph. |
| [corpus-converge](https://github.com/j0yen/corpus-converge) | `corpus-converge` | Self-state convergence primitive: version vector (per-node monotonic seq map), persist/load, rejoin protocol that computes per-channel gap after partition, and a freshness gate (`fresh?`) other daemons can call before acting on possibly-stale state. |
| [corpus-arbiter](https://github.com/j0yen/corpus-arbiter) | `corpus-arbiter` | Fleet-wide advisory lease registry — single-writer discipline across nodes; `acquire`/`release`/`renew`/`status`/`with` subcommands with injected-clock registry, deny-by-default attestation, and fail-safe no-arbiter mode. |
| [corpus-introspect](https://github.com/j0yen/corpus-introspect) | `corpus-introspect` | Multinode self-mirror — synthesises attest, roster, converge, arbiter, and tether into a single `WholeSelf` JSON record and human-readable self-portrait; `--format selfreview` feeds the self-review playbook. |

## Constellation fleet

Multi-machine expansion of the wintermute ecosystem — fleet provisioning, mesh networking, and coordinated deployment.

| Repo | Binary | Description |
|------|--------|-------------|
| [constellation](~/wintermute/constellation) | — | Fleet provisioning repo: Ansible playbooks (base/desktop/voice roles), greetd autologin + i3 graphical-session bridge, host_vars per-node config, isobuild archiso profile, and localrepo scripts — one ISO + one playbook run = an identical wintermute node. |
| [agorabus-nats-bridge](https://github.com/j0yen/agorabus-nats-bridge) | `wm-busbridge` | agorabus ↔ NATS bridge daemon: mirrors allowlisted `wm.fleet.*` events between the local agorabus UDS and a NATS leaf node, loop-guarded and bandwidth-selective — the keystone that makes the local wintermute bus fleet-wide. |
| [constellation-burst-builder](https://github.com/j0yen/constellation-burst-builder) | `wm-burst` | Mesh-free cloud-burst CLI: points `cargo` at a cheap dedicated box + shared sccache so heavy compiles stop pinning local cores; hard-fails on toolchain drift; enforces monthly pod budget cap. |
| [wm-node](https://github.com/j0yen/wm-node) | `wm-node` | Node identity + placement CLI: reads `~/.config/wintermute/node.toml` (name/roles/fleet) and `placement.toml`; `id`/`role`/`should-run`/`env` subcommands for systemd `ExecCondition=` and `EnvironmentFile=` gating. Turns implicit install-location placement into declarative per-node policy. |
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

## Ballast (disk reclamation)

Read-only measurement and safe-reclaim fleet for disk weight management on a laptop that lives at 94% full.

| Repo | Binary | What it does |
|---|---|---|
| [ballast-survey](https://github.com/j0yen/ballast-survey) | `ballast-survey` | v0.3.0 — Read-only disk inventory: walks roots, finds reclaimable subtrees (target/ dirs, cargo caches, node_modules, ~/.cache), sizes with mtime/age, emits structured JSON sorted by reclaimable bytes. Ground truth for ballast-reap and ballast-guard. |
| [ballast-guard](https://github.com/j0yen/ballast-guard) | `ballast-guard` | v0.1.0 — Autonomous disk SLO guard: watches high/low-water marks (configurable guard.toml), invokes ballast-reap fossil-first until usage drops below low-water, emits structured JSON events. Exit codes: 0/2/3/4. |
| [ballast-trend](https://github.com/j0yen/ballast-trend) | `ballast-trend` | v0.1.0 — Disk growth rate tracker: snapshots ballast-survey --json output into a bounded ring (~/.local/state/ballast/trend/), diffs pairs of snapshots to compute per-path bytes/day growth rates, projects ETA to a configurable high-water mark, emits JSON for downstream tools. |
| [ballast-pilot](https://github.com/j0yen/ballast-pilot) | — | v0.1.0 — Systemd timer + default config that wires ballast-guard to run hourly: guard.toml (mode=report, high-water=90, low-water=80, advisory=85), oneshot service, hourly Persistent timer, idempotent install.sh. Observes before it ever reaps. |
| [ballast-digest](https://github.com/j0yen/ballast-digest) | `ballast-digest` | v0.1.0 — Read-only disk health synthesizer: fuses ballast-survey (reclaimable paths + fossil/stale/warm class), ballast-trend (growth rates + ETA), and guard-events.jsonl (SLO band history, reclaimed bytes) into a single ranked digest block. `--json` for downstream embedding; `--top-k`, `--now` for tests. |
| [careen-guard](https://github.com/j0yen/careen-guard) | `careen-guard` | v0.1.0 — SLO-triggered sweep of live Rust target dirs: ballast-guard complement that handles binary-current dirs ballast cannot reap; sweeps largest live targets via careen-sweep on breach, emits ballast-compatible Event JSON (Ok/Warn/Breach/BreachUnresolved). Exit codes: 0/2/3/4. |
| [careen-ledger](https://github.com/j0yen/careen-ledger) | `careen-ledger` | v0.1.0 — Was-it-worth-it accounting for Cargo target/ sweeps: append-only JSONL ledger pairing each sweep's reclaimed bytes with the rebuild cost it provoked. Subcommands: record (ingest sweep summary → entry id), attribute (pair rebuild cost via explicit counts or cargo --message-format=json log), verdict (fold history → worth_careening bool). Feedback loop for careen-guard. |

## AtScale tooling

| Repo | Binary | What it does |
|---|---|---|
| [mqo-catalog-embed](https://github.com/j0yen/mqo-catalog-embed) | `mqo-catalog-embed` | Semantic (embedding) retrieval over AtScale model catalog columns — offline synonym-aware complement to keyword `search_columns`: `index`/`search`/`hybrid`/`serve` subcommands, pluggable hash (no-network default) or subprocess embedder, cosine + BM25 hybrid blend with `--alpha`, MCP tool server via stdin/stdout. |
| [mqo-measure-lint](https://github.com/j0yen/mqo-measure-lint) | `mqo-measure-lint` | Lint semantic models for AI-hostile naming, gaps, and redundancy: flags ambiguous names (M001), numeric dimensions (M002), versioned measure names (M003), redundant measures ≥85% similar (M004), binding-unfriendly name formats (M005), and sibling-model coverage gaps (M006); `--format sarif` for CI/IDE; `serve` MCP mode; `--rules` TOML for per-project suppress/extend. |
| [mqo-ai-coverage](https://github.com/joeyen-atscale/mqo-ai-coverage) | `mqo-ai-coverage` | Score a model's AI-queryability and reveal dark corners: scores every element on discoverability (description richness + BFO grounding), bindability (name NL patterns + embed-index rank), and queryability (grain, sensitivity); bottom quintile flagged as dark corners with `top_issue`; `--format html` emits a sortable author dashboard; `serve` MCP mode. |
| [mqo-chart-caption](https://github.com/joeyen-atscale/mqo-chart-caption) | `mqo-chart-caption` | Auto-generates natural-language captions for AtScale MQO chart outputs: maps measure/dimension structure to a concise summary sentence with trend detection, period-over-period delta annotation, and `--format json` for agent pipeline use; `serve` MCP mode. |
| [mqo-result-cache](https://github.com/j0yen/mqo-result-cache) | `mqo-result-cache` | Content-addressed result cache for AtScale MQO queries: SHA-256 keyed on canonicalized `BoundMqo` (sorted keys, cosmetic fields stripped), `key`/`get`/`put`/`purge`/`stats`/`serve` subcommands, max-age eviction, `--no-store` sensitivity interlock, JSON-RPC stdin `serve` mode. |
| [mqo-time-intelligence](https://github.com/joeyen-atscale/mqo-time-intelligence) | `mqo-time-intelligence` | Time phrase resolver and period-over-period MQO deriver for AtScale: `derive` subcommand takes a base MQO JSON + spec (yoy/qoq/mom/wow/rolling:N/mtd/qtd/ytd/pop:<grain>) and emits a derived MQO bundle with shifted date filters; `serve` mode accepts NDJSON tool requests on stdin for MCP server integration. |
| [mqo-sensitivity-scan](https://github.com/joeyen-atscale/mqo-sensitivity-scan) | `mqo-sensitivity-scan` | PII/sensitive-field scanner and redactor for AtScale MQO result rows and model metadata: flags fields by tag map, name patterns (ssn/email/phone/account/…), and value patterns (email regex, SSN-like, Luhn card); `--redact` masks values before they reach the model context window; `serve` mode for MCP integration. |
| [mqo-aggregate-advisor](https://github.com/joeyen-atscale/mqo-aggregate-advisor) | `mqo-aggregate-advisor` | Aggregate cost tier advisor for AtScale MQO queries: estimates whether a query will hit a pre-computed aggregate (`fast`/`medium`/`slow` via `advise --columns`), or scores a full MQO against aggregate definitions (`aggregate_hit`/`partial`/`base_scan` via `estimate`); `recommend` ranks covering aggregates by workload acceleration benefit; `serve` mode for MCP tool integration. Cost tiers are relative estimates, not wall-clock promises. |
| [mqo-session-budget](https://github.com/j0yen/mqo-session-budget) | `mqo-session-budget` | Per-session governor for agent query loops: tracks queries issued, estimated scan cost, and elapsed wall-time against a declared budget ceiling; `gate` refuses the next step when a ceiling is crossed; `charge` debits the ledger (consuming `mqo-aggregate-advisor` estimates or explicit cost values); kernel agentns enforcement opt-in with graceful userspace fallback; `serve` mode for MCP integration. |
| [mqo-synonym-seed](https://github.com/j0yen/mqo-synonym-seed) | `mqo-synonym-seed` | Generate NL synonym sets for AtScale model measures/dimensions to improve AI retrieval: rule-based `generate` (snake_case/suffix/prefix expansion — `_AMT`→amount/total/sum, `AVG_`→average/mean, `ROLLING_`→rolling-window paraphrases), `apply` to write approved synonyms into description fields, `serve` JSON-RPC stdio mode; `--planner-brain` opt-in to Claude API with graceful fallback. Author-facing companion to `mqo-catalog-embed`. |

## Concord (argument analysis)

Perspective-diverse source gathering and stance-tagged corpus for contested claims.

| Repo | Binary | What it does |
|---|---|---|
| [concord](https://github.com/j0yen/concord) | `concord` | Gather perspective-diverse sources for a contested claim, tag by stance, dedup near-identical framings, score credibility, emit a structured Corpus — foundation of the concord workspace. |
| [cogito](https://github.com/j0yen/cogito) | `cogito` | Operational TBox CLI for the wintermute box — forges `cogito.owl` (OWL 2 DL, BFO-grounded) from a declarative TOML spec; classes: Daemon, Socket, Bus, Tool, Repo, Session, KernelPrimitive, Healthcheck, BusHealthcheck; object properties: dependsOn (transitive), registersOn, writesTo, backedBy, providesCapability, healthcheckedBy, ownedByVision; IRI base `https://wintermute.local/cogito#`. |
| [doxa](https://github.com/j0yen/doxa) | `doxa` | v0.5.0 — Framework-neutral BFO-grounded moral TBox for comparative ethics: 15 moral-domain classes + 8 object properties compiled to OWL 2 DL. Three normative framework modules (consequentialism, deontology, virtue-ethics). `doxa reason <fw> --scenario <abox.ttl>` evaluates moral scenarios under a chosen framework via ousia-reason (with fixture fallback); `doxa compare` fans out N frameworks; `doxa guard --policy <policy>` aggregates verdicts into allow/flag/deny. 90 tests. |
| [ousia-forge](https://github.com/j0yen/ousia-forge) | `ousia-forge` | Build the World Ontology (OWL 2 DL / RDF/XML) from a declarative TOML spec; subcommands: build, check, stats. Gate tool for the full ousia toolchain. |
| [ousia-sparql](https://github.com/j0yen/ousia-sparql) | `ousia-sparql` | SPARQL 1.1 query layer over the materialized World Ontology: load OWL + ABox into an oxigraph store, materialize entailments via ousia-reason, and query with `load`/`query`/`ask`/`serve`; ships a canned pack (dignity-bearers, rights-violations, unaccountable-authority, just-societies, etc.) that turns the ethical structure into runnable demo queries. |
| [ousia-atscale](https://github.com/j0yen/ousia-atscale) | `ousia-atscale` | v0.5.0 — BFO grounding bridge for the AtScale semantic layer: maps AtScale model elements (measures, dimensions, column-groups) onto BFO 2020 categories and emits annotation overlays with §4.4 vocabulary (philosophicalGrounding, domainModule, aristotelianDefinition). Subcommands: ground, annotate, report, export, diff, validate, **serve** (MCP stdio server exposing ground_model, coverage_report, diff_models, validate_model over JSON-RPC 2.0). |
| [ousia-mqo](https://github.com/j0yen/ousia-mqo) | `ousia-mqo` | v0.2.0 — BFO-grounded AtScale model overlays as a reusable Rust library + CLI. `Grounder` lib wraps `ousia-atscale annotate` with SHA-256 content-hash caching; `ousia-mqo diff` compares two model overlays by `(iri, domain_module)` key — same key = `Agree` regardless of column name; same name + different key = `Diverge`; exits non-zero on divergences for CI gating. Subcommands: ground, bind, diff. |
| [ousia-mcp](https://github.com/j0yen/ousia-mcp) | `ousia-mcp` | MCP server exposing ousia-reason, ousia-sparql, and ousia-guard as five read-only tools (classify, query, ask_canned, guard_check, explain) over stdio; ontology loaded once at startup, answers from in-memory reasoned store. |
| [recourse](https://github.com/j0yen/recourse) | `recourse` | Durable, PII-free verdict receipt layer for ousia-guard: `receipt emit` hashes the action (blake3, never raw), appends NDJSON receipt; `show`/`ls` for audit and pipeline integration. First crate in the verdict-recourse chain. |
| [tribunal](https://github.com/j0yen/tribunal) | `tribunal` `tribunal-bench` `tribunal-gate` | Ethics engine conformance, bench, and gate harness. `tribunal conformance` checks OWL 2 DL compliance, single-inheritance, TBox-only, and 10-axiom encoding; `tribunal-bench` runs ousia-guard over the 64-case corpus with confusion matrix and false-allow detection; `tribunal-gate` composes both into a publish precondition (conformance + accuracy + zero-false-allow). |
| [lattice-registry](https://github.com/j0yen/lattice-registry) | `lattice-registry` | Local catalog of BFO-grounded ontologies for the ethical lattice: fetch, verify BFO grounding, and index ontologies from OBO Foundry and other registries. `sync`/`add`/`list`/`show`/`path` with etag-based caching under `~/.cache/lattice/registry/`. |
| [lattice-ground](https://github.com/j0yen/lattice-ground) | `lattice-ground` | Ground natural-language mentions to BFO-anchored ontological classes across the federated lattice. `resolve` ranks candidates with lexical+context scoring and surfaces cross-ontology bridge consequences (patient→dignity); `link` expands a known IRI's bridge/subsumption neighborhood. Output is structured JSON consumable by `ousia-guard`. |
| [rosetta-prov](https://github.com/j0yen/rosetta-prov) | `rosetta-prov` | Translate ousia-guard ethical verdicts into W3C PROV-O linked data (Turtle + JSON-LD). Maps verdict + axiom chain + provenance (agent session, ontology version) onto `prov:Activity`/`prov:Entity`/`prov:Agent`; each `rules_fired` id becomes a `prov:wasDerivedFrom` derived entity with its axiom chain as `rosetta:axiom` literals. Foundation of the rosetta semantic-web interoperability fleet. |
| [rosetta-credential](https://github.com/j0yen/rosetta-credential) | `rosetta-credential` | Wrap ousia-guard verdicts and PROV-O graphs as W3C Verifiable Credentials (JSON-LD), signed with inoculate-signet's Ed25519 key. `issue`/`verify`/`inspect` subcommands; v1 signs sorted-key JSON-LD (documented simplification of RDFC-1.0). Makes an ethical clearance cryptographically verifiable by any third party offline. |
| [rosetta-shacl](https://github.com/j0yen/rosetta-shacl) | `rosetta-shacl` | Express ousia-guard's four ethical rules as W3C SHACL shapes and validate RDF action-graphs against them, emitting a standard `sh:ValidationReport`. Ships a SHACL-core subset engine (no general SHACL crate needed); `validate`/`shapes`/`rules` subcommands; Turtle + JSON-LD report output. Makes the same ethical constraints portable to any SHACL engine. |
| [rosetta-serve](https://github.com/j0yen/rosetta-serve) | `rosetta-serve` | Local HTTP server for dereferenceable IRIs and a W3C SPARQL 1.1 Protocol endpoint over the oxigraph lattice store. `up --store <path>` serves `GET/POST /sparql?query=…` (SELECT/CONSTRUCT/DESCRIBE/ASK; UPDATE → 405) and `GET /{prefix}/{local}` (bounded DESCRIBE, content-negotiated Turtle/JSON-LD/HTML). Binds `127.0.0.1:7180` by default with configurable per-query timeout. |
| [rosetta-attest](https://github.com/joeyen-atscale/rosetta-attest) | `rosetta-attest` | Capstone attestation of the rosetta ethical-AI chain (v0.1.0). Drives a real action through all five rosetta tools (lattice-ground → ousia-guard → rosetta-prov → rosetta-shacl → rosetta-credential → rosetta-serve → SPARQL round-trip) and asserts they compose end-to-end, emitting a committed receipt.json with per-step status and overall verdict. |

## Warrant suite

Close-claim corpus reader and assertion runner — parses, classifies, and re-verifies PRD close notes so false "outcome achieved by a different mechanism" closes are caught before the symptom recurs from scratch.

| Repo | Binary | What it does |
|---|---|---|
| [warrant](https://github.com/j0yen/warrant) | `warrant` | Root workspace: close-claim domain model (`CloseClaim`, `ClaimKind`, `Warrant`, `AssertionSpec`, `WarrantVerdict`), pure `classify()` function (zero IO), `CloseSource` trait + `FakeSource`; `warrant list [--format json]` and `warrant list-sources` CLI. Foundation for warrant-audit and warrant-docket. |

## Answerable suite

Semantic audit trail for autonomous agent actions — an append-only human-readable ledger and guard layer.

| Repo | Binary | What it does |
|---|---|---|
| [answerable](https://github.com/j0yen/answerable) | `answerable` | Append-only JSONL ledger of high-consequence autonomous actions (`record`/`log`/`stats`). Library API (`Action`, `ActionKind`, `Ledger::append/read/since`) for sibling PRDs. SIGPIPE-safe; O_APPEND atomic writes. |

## License

Each repo is dual-licensed MIT or Apache-2.0 at the user's option.

## MQO tools

| Repo | Binary | What it does |
|---|---|---|
| [mqo-scorecard](https://github.com/j0yen/mqo-scorecard) | `mqo-scorecard` | Aggregates MQO tool check results (grounding coverage, anomaly count, unit violations, parity mismatches, semantic regressions) into a single letter-grade scorecard; `score` and `from-json` subcommands; text + JSON output. |
| [mqo-anomaly-scan](https://github.com/j0yen/mqo-anomaly-scan) | `mqo-anomaly-scan` | Statistical outlier detection for MQO query result rowsets: z-score, IQR, and MAD methods; per-group `--by` dimension support; `serve` JSON dispatcher for agent pipelines. Returns only ranked outlier rows so a model gets "row 47 is 3.2σ above the mean" without paging all rows. |
| [mqo-insight-extract](https://github.com/j0yen/mqo-insight-extract) | `mqo-insight-extract` | Signal extractor for MQO metric answers: given result rows + optional anomaly/parity/confidence inputs, identifies top-N findings (period-over-period deltas, anomaly crossings, parity gaps) ranked by `magnitude × confidence`; `--min-magnitude` noise filter; `serve` MCP mode. |
| [mqo-clarify](https://github.com/joeyen-atscale/mqo-clarify) | `mqo-clarify` | Detect ambiguous field bindings and emit disambiguation questions before MQO execution: `ask --candidates <file>` checks per-field candidate sets for within-margin ties, emits `{ambiguous, questions}` JSON; `serve` mode handles `clarify_binding` tool calls; `--fail-if-ambiguous` exit-code gate for agent pipelines. |
| [mqo-error-explain](https://github.com/joeyen-atscale/mqo-error-explain) | `mqo-error-explain` | Maps raw backend query faults (XMLA HRESULT, DAX/MDX/SQL errors) to structured `{matched, category, cause, suggested_fix}` from a data-driven TOML catalog; `explain` subcommand with `--backend` narrowing, `--catalog` override, `--format json`; `serve` mode for MCP tool dispatch; unknown faults return generic guidance, never a fabricated cause. |
| [mqo-engine-parity](https://github.com/joeyen-atscale/mqo-engine-parity) | `mqo-engine-parity` | Numeric parity checker between AtScale and direct SQL query engines: aligns two result rowsets by dimension key, compares each measure cell within a relative tolerance, and reports `parity: ok\|drift` with `mismatched_rows`, `only_in_a`, `only_in_b`. `check` subcommand; `--format json` for pipeline use. |
| [mqo-textsql-baseline](https://github.com/j0yen/mqo-textsql-baseline) | `mqo-textsql-baseline` | Deliberately-naive rule-based text-to-SQL control binder: `bind` maps NL questions to raw physical tables/columns with `failure_flags` (`double_count_risk`, `semantic_measure_absent`, `pii_column_selected`); `strip` derives raw-only schema from a full semantic model; `serve` is a JSON-RPC 2.0 stdin/stdout subprocess for `mqo-bench run --binder`. The control's *mistakes* are the measurement — its BoundMqo predictions feed `mqo-bench compare` to make the "+N% vs text-to-SQL" delta honest. |
| [mqo-bench](https://github.com/joeyen-atscale/mqo-bench) | `mqo-bench` | Lightweight benchmark harness for MQO queries: runs a command N times with optional warmup, records wall-clock timing, and reports mean, median, p95, p99, min, max in text or JSON. `run --cmd "..." --iterations N --warmup W --format json`. |
| [mqo-semantic-regression](https://github.com/j0yen/mqo-semantic-regression) | `mqo-semantic-regression` | CI gate for AtScale BFO grounding changes: `check --old --new` compares two overlay snapshots and classifies each element change as `Breaking` (continuant↔occurrent boundary crossed) or `Minor` (within same top-level); `--fail-on-breaking` exits non-zero for CI gates; `--format json` for pipeline use. |
| [mqo-unit-guard](https://github.com/joeyen-atscale/mqo-unit-guard) | `mqo-unit-guard` | Value-semantics firewall for MQO measure combinations: detects `additive_mismatch` (currency+ratio summed), `currency_mismatch` (different currency codes), `ratio_summed` (ratio under sum aggregation), `scale_mismatch` (count on currency axis); `check` subcommand with `--units`/`--from-model` source; `serve` MCP dispatcher; exits non-zero on error-severity violations for CI gating. |
| [mqo-access-policy](https://github.com/j0yen/mqo-access-policy) | `mqo-access-policy` | Authorization gate: routes agent identities to the model variant they are cleared for. `check` returns allowed/routed_model/denied_columns; `enforce` gates a bound MQO before execution; `variants` audits which models have `_no_pii` twins; `serve` exposes check/enforce as MCP tools. TOML policy file, deny-by-default, cluster-free. |
| [mqo-demo-runner](https://github.com/j0yen/mqo-demo-runner) | `mqo-demo-runner` | Guided demo runner for the AtScale MQO ethical-AI toolchain. Simple mode: `run [--model] [--dry-run] [--format text\|json]` and `list` walk the five ousia-atscale steps; missing binaries are skipped gracefully. Scenario pipeline mode: calls catalog-embed, binding-confidence, clarify, time-intelligence, engine-parity, sensitivity-scan, ousia-grounding, and rosetta-credential in order; `run --mock` works with no sibling binaries for CI; `--format md` renders a human demo script; `serve` exposes the pipeline as an MCP tool. |
| [mqo-trace-harvest](https://github.com/joeyen-atscale/mqo-trace-harvest) | `mqo-trace-harvest` | MQO execution trace harvester for column usage and error pattern analysis: `harvest` parses JSONL trace logs and emits success/error/timeout counts, p95 duration, top columns, and error patterns (text or JSON); `top-columns` ranks columns by query frequency. |
| [mqo-goldgrow](https://github.com/j0yen/mqo-goldgrow) | `mqo-goldgrow` | Human-gated curation of harvested NL→MQO golden-set candidates: `review` emits a work-list of candidates not already in the golden set or rejection ledger (near-duplicate filtering via Jaccard threshold); `accept --reviewer <id>` appends to the golden set with mandatory provenance; `reject --reason <s>` appends to an append-only rejection ledger; `stats` reports by-source composition and accept/reject ratio; `serve` exposes all tools as MCP tool calls. No anonymous ground truth; no auto-accept flag. |
| [mqo-decision-log](https://github.com/j0yen/mqo-decision-log) | `mqo-decision-log` | Durable append-only JSONL log of every mqo-agent decision: `append`/`query`/`summary`/`verify`/`serve` subcommands; shared schema with mqo-agent's answer.json; provfs xattr tamper-evidence; source for mqo-trace-harvest and mqo-scorecard trend deltas. |
| [mqo-replay](https://github.com/j0yen/mqo-replay) | `mqo-replay` | Behavioral regression replay gate for mqo-agent: `run` re-invokes the agent on logged questions, `diff` classifies deltas as `plan_drift`/`bind_drift`/`outcome_drift`/`value_drift`/`unchanged`, `report --fail-on` gates CI on specific drift classes; numeric tolerance matches mqo-engine-parity; cluster-free test suite with fixture mock agents; `serve` MCP tool server. |
| [mqo-narrative-compose](https://github.com/j0yen/mqo-narrative-compose) | `mqo-narrative-compose` | Compose extracted insights into audience-targeted prose: `compose --insights <file> --audience executive\|analyst\|technical` fills built-in mustache templates with headline finding, dimension breakdown, and anomaly callout; `--template` override for custom voice; `--planner-brain` opt-in routes to Claude API for free-form prose; `serve` MCP mode. No LLM required on the default path. |
| [mqo-report-pack](https://github.com/j0yen/mqo-report-pack) | `mqo-report-pack` | Bundle metric answers, charts, and narrative into shareable reports: `pack` assembles answer JSON + chart refs + narrative prose into a versioned report bundle (HTML/JSON/MD); `serve` MCP mode for agent pipelines. |

## Recently shipped (auto)

| Repo | Binary | What it does |
|---|---|---|
| [headway](https://github.com/j0yen/headway) | `headway` | The fleet has no reusable primitive that recompiles a crate the *sanctioned* |

| Repo | Binary | What it does |
|---|---|---|
| [colophon](https://github.com/j0yen/colophon) | `colophon` | The booted `7.0.11-arch1-1-wintermute` kernel stamps a structured |

| Repo | Binary | What it does |
|---|---|---|
| [tether-link](https://github.com/j0yen/tether-link) | `tether-link` | The problem: agorabus is a Unix-domain socket — it cannot reach another machine |

| Repo | Binary | What it does |
|---|---|---|
| [tether-tools](https://github.com/j0yen/tether-tools) | `tether-tools` | The problem: the wintermute toolkit (`recall`, `ctrace`, `procstat`, `wchg`, |

| Repo | Binary | What it does |
|---|---|---|
| [hold-anchor](https://github.com/j0yen/hold-anchor) | `hold-anchor` | Establish one shared CARGO_TARGET_DIR for the wintermute fleet: writes `~/wintermute/.cargo/config.toml` with `build.target-dir`, verifies cargo picks it up, supports `apply`/`status`/`unset`; idempotent, conflict-safe, TOML-key-preserving. |
| [inoculate](https://github.com/j0yen/inoculate) | `inoculate` | Distills CLAUDE_SELF.md Values+Boundaries + redline.toml into a versioned, blake3-hashable ethics strain; `strain`, `hash`, `version` subcommands; library crate `inoculate-core` for sibling crates. |
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

| Repo | Slug | What it does |
|---|---|---|
| [persona-work](https://github.com/j0yen/persona-work) | `persona-work` | Work-machine Claude Code identity (`CLAUDE_WORK.md`) for Joe's AtScale laptop — install/validate scripts, no voice/agorabus/auto-publish. |
| [persona-deploy-doctor](https://github.com/j0yen/persona-deploy-doctor) | — | Persona drift health checker for the Jocelyn elder persona: asserts `forbidden_terms` non-empty, `redline` active, and `self_name` matches expected warm name; exits 0/1/2; `--json` output; daily systemd timer publishes `wm.persona.drift` bus event on drift. |
| [persona-deploy-jocelyn](https://github.com/j0yen/persona-deploy-jocelyn) | — | Idempotent shell installer for the Jocelyn elder persona: applies the `jocelyn` profile to `brain.toml`, sets `self_name`/`wake_word`/redline, snapshots for rollback, restarts wm-brain. |
| [persona-redline-eval](https://github.com/j0yen/persona-redline-eval) | — | Held-out eval harness: 45 naturalistic technophobe-trigger prompts (independent of redline.rs test strings) drive the live brain pipeline and report raw vs. post-enforce forbidden-term leak rates. |

| Repo | Binary | What it does |
|---|---|---|
| [mqo-binding-confidence](https://github.com/joeyen-atscale/mqo-binding-confidence) | `mqo-binding-confidence` | Calibrated 0–1 confidence scorer for MQO bound fields — name match, margin, uniqueness, and structural fit combine into a deterministic signal with `high\|medium\|low` bucket and ranked alternatives; `score` subcommand with `--fail-below` CI gate; `serve` MCP-style dispatcher. |
| [mqo-agent](https://github.com/j0yen/mqo-agent) | `mqo-agent` | Adaptive reference agent that plans which MQO pillars to call for any NL question: deterministic rule-based planner (catalog-embed → binding-confidence → clarify? → time-intelligence? → engine-parity? → sensitivity-scan → rosetta-credential); `ask`/`plan` subcommands; `--mock` CI mode; clarify loop with `--answer` resume; `serve` MCP dispatcher. |

## Session orchestrator

| Repo | Binary | What it does |
|---|---|---|
| [roundtable](https://github.com/j0yen/roundtable) v0.5.0 | `roundtable` | Daily session orchestrator: chains the-lunch → vicious-circle → conning-tower with XDG-compliant ledger, dedup guard, and dry-run mode. v0.4.0 adds `bind` (new-yorker issue+cover), `games` (debate transcripts vs Wordsmith/Pedant/Contrarian), `weekly` (ISO-week digest in text/markdown/json), and `--with-games` on session. v0.5.0 adds `cadence` subcommand: `cadence show [--format text|json]` and `cadence next [--from DATE]` display and compute the Mon–Fri session / Sun bind / Mon digest schedule using pure JDN date arithmetic. |

## MQO template management

| Repo | Binary | What it does |
|---|---|---|
| [mqo-template](https://github.com/joeyen-atscale/mqo-template) | `mqo-template` | Parameterized reusable MQO query templates: save/get/list/delete/instantiate JSON templates with `{{slot}}` placeholders. |
| [mqo-grounding-advisor](https://github.com/j0yen/mqo-grounding-advisor) | `mqo-grounding-advisor` | Suggest BFO 2020 grounding for ungrounded AtScale model elements — rule-based pattern matching with plain-English rationale. |

| [tether-gossip](https://github.com/j0yen/tether-gossip) | `wm-tether-gossip` | Bidirectional gossip.md mirror over fleet bus — one gossip log across machines. |
| [tether-recall](https://github.com/j0yen/tether-recall) | `wm-tether-recall` | Fleet bus proxy for recall memory queries — makes the laptop's recall store legible from the work node via NATS `wm.fleet.recall.*` subjects; read-only in v1. |
