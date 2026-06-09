# docket

A structured ledger for standing findings. A finding reported twice is the same finding.

`docket` is a small SQLite-backed CLI that deduplicates findings by a stable key, tracks
first/last-seen timestamps, maintains a consecutive-run streak, and accumulates a typed
**evidence trail** — so every run that observes a finding contributes its proof, and
`docket show` renders the full chronological trail.

## Install

```sh
cargo install --path .
# binary lands at ~/.cargo/bin/docket
# or copy target/release/docket to ~/.local/bin/
```

## Database location

`$XDG_DATA_HOME/docket/docket.db` (defaults to `~/.local/share/docket/docket.db`).

The directory is created automatically on first use.

## Schema

### `findings` table

| column              | type    | description                                                      |
|---------------------|---------|------------------------------------------------------------------|
| `key`               | TEXT PK | stable slug — `agorabus-stale-binary`                           |
| `title`             | TEXT    | human one-liner (latest report wins)                            |
| `severity`          | TEXT    | `info` / `warn` (default) / `crit`                              |
| `status`            | TEXT    | `open` / `escalated` / `resolved`                               |
| `first_seen`        | TEXT    | RFC3339 timestamp of first report                               |
| `last_seen`         | TEXT    | RFC3339 timestamp of most recent report                         |
| `first_run`         | TEXT    | run-id of the first report                                      |
| `last_run`          | TEXT    | run-id of the most recent report                                |
| `runs_seen`         | INTEGER | count of distinct run-ids that reported this finding            |
| `consecutive_runs`  | INTEGER | current streak (distinct sequential run-ids)                    |
| `report_count`      | INTEGER | total raw report calls                                          |
| `resolved_at`       | TEXT    | RFC3339 timestamp of resolution (null if open)                  |
| `resolve_reason`    | TEXT    | reason string (null if not provided)                            |
| `escalated_at`      | TEXT    | RFC3339 timestamp of escalation (null if not escalated)         |
| `escalation_reason` | TEXT    | escalation reason (null if not escalated)                       |

### `runs` table

Append-only ledger of every distinct run-id, in the order it first appeared in a `report` call.
`sweep` uses `seq` to measure how many runs have elapsed since a finding was last seen.

| column   | type    | description                                      |
|----------|---------|--------------------------------------------------|
| `run_id` | TEXT PK | opaque run identifier (caller-supplied)          |
| `seq`    | INTEGER | monotonically increasing insertion order (1, 2…) |
| `seen_at`| TEXT    | RFC3339 timestamp of first appearance            |

Reporting a run-id that already exists is a no-op (idempotent insert). Reporting `r1`, `r2`, `r1`
again yields exactly two rows (`seq` 1 and 2 for `r1` and `r2`).

### `evidence` table (M2 — append-only)

Each `--evidence` ref is stored as a typed row, keyed by `findings.key` and the reporting `run_id`.
Evidence rows are never overwritten — every new report appends to the trail.

| column    | type    | description                                                      |
|-----------|---------|------------------------------------------------------------------|
| `id`      | INTEGER | auto-increment primary key (insertion order)                    |
| `key`     | TEXT    | FK → `findings.key`                                             |
| `run_id`  | TEXT    | run identifier of the reporter                                  |
| `kind`    | TEXT    | evidence kind (see table below)                                 |
| `ref_val` | TEXT    | the reference value (everything after the `<kind>:` prefix)    |
| `note`    | TEXT    | reserved (always null for now)                                  |
| `seen_at` | TEXT    | RFC3339 timestamp when the row was inserted                     |

### Evidence kinds

| prefix     | kind      | meaning                            | example                                |
|------------|-----------|------------------------------------|----------------------------------------|
| `recall:`  | `recall`  | recall memory ULID                 | `recall:01KSRV7R4FERPP40HQGV5RGZNT`  |
| `journal:` | `journal` | journal date + optional `#line`    | `journal:2026-05-28#7`                |
| `pid:`     | `pid`     | process id observed                | `pid:2138939`                          |
| `provfs:`  | `provfs`  | `user.prov.ts` epoch / xattr       | `provfs:1780026726`                    |
| `commit:`  | `commit`  | git SHA                            | `commit:02350fb`                       |
| `path:`    | `path`    | filesystem path                    | `path:/home/jsy/.local/bin/agorabus`  |
| (other)    | `raw`     | unknown prefix — stored as-is      | `somethingweird`                       |

Parsing is always **lenient**: malformed refs (e.g. invalid ULIDs) are stored as-given and never
cause a nonzero exit.

WAL mode is enabled; `BEGIN IMMEDIATE` transactions guard concurrent writers.

## DOCKET — escalation lifecycle

A finding moves through a defined lifecycle as it recurs or disappears across runs:

```
            report (streak ≥ threshold)
   open ───────────────────────────────► escalated
    ▲  │                                      │
    │  └──────── report (streak < threshold) ─┤ (stays escalated once tripped,
    │                                         │  until resolved/swept)
    └── report (reopen) ── resolved ◄─────────┘
                              ▲
                              │ sweep: not seen in last K runs
                            open/escalated
```

**Transitions:**

| Event | From | To | Side-effect |
|---|---|---|---|
| `report` (first time) | — | `open` | streak=1 |
| `report` (same run-id) | `open` | `open` | `report_count++` only |
| `report` (new run-id, streak < threshold) | `open` | `open` | streak++ |
| `report` (new run-id, streak ≥ threshold) | `open` | `escalated` | writes `escalated_at`, `escalation_reason` |
| `report` (new run-id, any streak) | `escalated` | `escalated` | stays escalated; streak++ |
| `report` (any run) | `resolved` | `open` | streak reset to 1, `resolved_at` cleared |
| `resolve` | `open`/`escalated` | `resolved` | writes `resolved_at`, `resolve_reason` |
| `sweep` (absent ≥ K runs) | `open`/`escalated` | `resolved` | `resolve_reason="stale: …"` |
| `sweep` (gap, absent < K runs) | `open` | `open` | streak reset to 0 |

**Escalation is sticky.** Once a finding reaches `escalated`, additional `report` calls keep it
escalated — the threshold is not re-evaluated downward. Only `resolve` or `sweep` can leave the
escalated state.

**Escalation threshold knobs** (both default to 3):

| Method | Syntax | Priority |
|---|---|---|
| Default | built-in | lowest |
| Env var | `DOCKET_ESCALATE_THRESHOLD=N` | middle |
| CLI flag | `docket report --escalate-threshold N` | highest (overrides env) |

Similarly for the sweep staleness window:

| Method | Syntax | Priority |
|---|---|---|
| Default | built-in (3) | lowest |
| Env var | `DOCKET_STALE_AFTER=N` | middle |
| CLI flag | `docket sweep --stale-after N` | highest |

## Run model

A **run** is a caller-supplied opaque string (e.g. `2026-05-29.1`) passed via `--run`.

- Reporting the same key **twice in the same run-id**: `report_count` increments, `runs_seen`
  and `consecutive_runs` do not. Idempotent for streak purposes.
- Reporting the same key in a **new run-id**: both `runs_seen` and `consecutive_runs` increment.
- Reporting a **resolved** finding: it is reopened (`status=open`, streak reset to 1,
  `resolved_at` cleared).
- A **gap** (key seen at r1, then r3, absent at r2) breaks `consecutive_runs` — the streak at r3
  reflects only the unbroken tail, not the total count.
- Runs are registered in the `runs` ledger in arrival order; `sweep` uses this ledger to count
  elapsed runs since a finding's `last_run`.

## Commands

### `report`

Upsert a finding. `--evidence` is repeatable; pass it once per ref.

```sh
docket report \
  --run <id> \
  --key <slug> \
  --title <text> \
  [--severity info|warn|crit] \
  [--escalate-threshold <N>] \
  [--evidence <kind>:<ref>] \
  [--evidence <kind>:<ref>] ...
```

- Creates the finding if new (`status=open`, streak=1, `runs_seen=1`, `report_count=1`).
- Bumps an existing open finding with the same run-id: increments `report_count` only.
- Bumps an existing open finding with a new run-id: increments `runs_seen`, `consecutive_runs`,
  `report_count`, updates `last_seen`/`last_run`.
- Reopens a resolved finding: resets `consecutive_runs=1`, clears `resolved_at`/`resolve_reason`.
- Each `--evidence` ref appends one row to the `evidence` table tagged with the `run_id`.
- After updating the streak, if `consecutive_runs >= escalate_threshold` and `status=open`, the
  finding is escalated: `status=escalated`, `escalated_at` set, `escalation_reason` written.
  Threshold defaults to 3; overridden by `DOCKET_ESCALATE_THRESHOLD` env or `--escalate-threshold`.

### `list`

List findings.

```sh
docket list [--open|--resolved|--escalated|--all] [--format text|json] [--severity info|warn|crit]
```

Default: `--open --format text`. `--severity` is a minimum filter (inclusive).

JSON output includes `evidence_count` (aggregate) per finding but not the full trail.

### `show`

Show a single finding's full record, including its complete evidence trail.

```sh
docket show <key> [--format text|json]
```

- Text output includes an *Evidence* section grouped by run, each line `[<run_id>] <kind>: <ref>`.
- JSON output includes an `evidence_trail` array of typed `EvidenceRow` objects.
- Exits nonzero if the key is unknown.

### `resolve`

Mark a finding as resolved.

```sh
docket resolve <key> [--reason <text>]
```

Idempotent. Exits nonzero if the key is unknown.

### `sweep`

Auto-resolve stale open/escalated findings not seen in recent runs.

```sh
docket sweep --run <current-run-id> [--stale-after <N>]
```

Default `stale-after`: 3 (overridden by `DOCKET_STALE_AFTER` env var, then `--stale-after` flag).

`sweep` uses the `runs` ledger to count how many recorded runs have elapsed since each finding's
`last_run`. A finding whose `last_run` is absent from the current run and where at least `N` runs
have been recorded after it is resolved as `stale`. A finding absent for fewer than `N` runs has
its streak reset to 0 but is **not** resolved — it is merely cooling.

**Semantics:**

- `--run <id>` is registered in the `runs` ledger (same as `report`).
- Every `open`/`escalated` finding whose `last_run != <id>` is examined.
- If `runs_elapsed_since(last_run) >= stale-after`: set `status=resolved`,
  `resolve_reason="stale: not seen in <N> runs (swept at <id>)"`.
- Otherwise: reset `consecutive_runs=0` (gap breaks the streak).

Findings seen in the **current** run (`last_run == <id>`) are never swept.

## Worked example — multi-run evidence trail

The real value of typed evidence is that a single finding accumulates proof across multiple
self-review runs. Here is the `agorabus-stale-binary` case that motivated this feature:

```sh
# Run 18 (ULID 01KSRV7R4FERPP40HQGV5RGZNT) sees the binary and a running pid
docket report \
  --run 01KSRV7R4FERPP40HQGV5RGZNT \
  --key agorabus-stale-binary \
  --title "agorabus binary is stale (running a deleted inode)" \
  --severity crit \
  --evidence recall:01KSRV7R4FERPP40HQGV5RGZNT \
  --evidence pid:2138939 \
  --evidence provfs:1780026726

# Run 19 (ULID 01KSS21WFN5H6V42JF723Z8K2J) — still there, plus a journal entry
docket report \
  --run 01KSS21WFN5H6V42JF723Z8K2J \
  --key agorabus-stale-binary \
  --title "agorabus binary is stale (running a deleted inode)" \
  --evidence recall:01KSS21WFN5H6V42JF723Z8K2J \
  --evidence journal:2026-05-29#7 \
  --evidence path:/proc/2138939/exe

# Show the full trail
docket show agorabus-stale-binary
# [crit] agorabus-stale-binary (crit)
#   title: agorabus binary is stale (running a deleted inode)
#   ...  runs_seen: 2  consecutive_runs: 2  report_count: 2
#   Evidence:
#     [01KSRV7R4FERPP40HQGV5RGZNT] recall: 01KSRV7R4FERPP40HQGV5RGZNT
#     [01KSRV7R4FERPP40HQGV5RGZNT] pid: 2138939
#     [01KSRV7R4FERPP40HQGV5RGZNT] provfs: 1780026726
#     [01KSS21WFN5H6V42JF723Z8K2J] recall: 01KSS21WFN5H6V42JF723Z8K2J
#     [01KSS21WFN5H6V42JF723Z8K2J] journal: 2026-05-29#7
#     [01KSS21WFN5H6V42JF723Z8K2J] path: /proc/2138939/exe

# Machine-readable evidence trail
docket show agorabus-stale-binary --format json | jq '.evidence_trail[] | "\(.run_id) \(.kind):\(.ref_val)"'

# List with evidence counts
docket list --format json | jq '.[] | "\(.key): \(.evidence_count) evidence refs"'
```

## Worked example — escalation and sweep

This example walks the full lifecycle: a finding recurs across 3 runs (triggering escalation),
then disappears and is auto-resolved by `sweep`.

```sh
# Run r1 — finding first seen; status=open, streak=1
docket report --run r1 --key agentns-zero-ids --title "agentns agent_session all-zeros"

# Run r2 — still present; streak=2, still open (< threshold of 3)
docket report --run r2 --key agentns-zero-ids --title "agentns agent_session all-zeros"

# Run r3 — third consecutive run; streak=3 ≥ threshold → escalated
docket report --run r3 --key agentns-zero-ids --title "agentns agent_session all-zeros"

docket show agentns-zero-ids --format json | jq '{status, consecutive_runs, escalated_at, escalation_reason}'
# {
#   "status": "escalated",
#   "consecutive_runs": 3,
#   "escalated_at": "2026-05-29T10:00:00Z",
#   "escalation_reason": "recurred 3 consecutive runs (≥3); durable handling justified per self-review SKILL.md §359"
# }

# Run r4 — a 4th report keeps it escalated (sticky)
docket report --run r4 --key agentns-zero-ids --title "agentns agent_session all-zeros"
docket show agentns-zero-ids --format json | jq '.status'
# "escalated"

# List only escalated findings
docket list --escalated
# [escalated] agentns-zero-ids — agentns agent_session all-zeros

# Runs r5 and r6 happen; the finding is NOT reported (it resolved itself)
# Sweep at r7 with stale-after=2 — r5 and r6 have elapsed since r4 → resolved
docket sweep --run r7 --stale-after 2

docket show agentns-zero-ids --format json | jq '{status, resolve_reason}'
# {
#   "status": "resolved",
#   "resolve_reason": "stale: not seen in 2 runs (swept at r7)"
# }
```

**Using a lower threshold via env or flag:**

```sh
# Trip escalation after just 2 runs using the env var
DOCKET_ESCALATE_THRESHOLD=2 docket report --run r1 --key my-key --title "..."
DOCKET_ESCALATE_THRESHOLD=2 docket report --run r2 --key my-key --title "..."
# status=escalated after r2

# Flag overrides env: env=2 but flag=5 → stays open at 2 runs
DOCKET_ESCALATE_THRESHOLD=2 docket report --run r1 --key my-key2 --title "..." --escalate-threshold 5
DOCKET_ESCALATE_THRESHOLD=2 docket report --run r2 --key my-key2 --title "..." --escalate-threshold 5
docket show my-key2 --format json | jq '.status'
# "open"  ← flag wins, threshold is 5
```

## digest

`docket digest` produces a compact rollup of the open/escalated finding set — one line for
a SessionStart banner, or a `wm.health.*`-compatible JSON envelope for machine consumers.

```sh
docket digest [--format text|json] [--severity info|warn|crit]
```

- **text** (default): a one-to-three line summary safe to inline in a banner.
  Empty store → single clean line, exit 0.
  ```
  docket: 4 open (1 crit), 1 escalated · oldest: agorabus-stale-binary (12 runs)
    escalated: agorabus-stale-binary — recurred 3+ runs, durable handling justified
  ```
- **json**: a `wm.health.*` envelope (see "Status mapping" below).
  ```json
  {
    "component": "docket",
    "status": "degraded",
    "summary": "4 open, 1 escalated",
    "detail": {
      "open": 4, "escalated": 1, "crit": 1,
      "oldest_key": "agorabus-stale-binary", "oldest_runs": 12,
      "escalated_keys": ["agorabus-stale-binary"]
    }
  }
  ```
- **`--severity <min>`**: exclude findings below the specified severity from all counts and
  summary output (e.g. `--severity warn` excludes `info`-severity findings).

### Status mapping

The `wm.health.*` envelope `status` field is determined as follows:

| Condition                              | `status`   |
|----------------------------------------|------------|
| no findings (empty store)              | `ok`       |
| open findings only, none escalated     | `ok`       |
| any escalated findings (non-crit)      | `degraded` |
| any escalated finding with `crit` severity | `down` |

**Oldest finding** is selected by `consecutive_runs` (run-age), not wall-clock time.
The selection is stable across `--format text` vs `--format json`.

### `wm.health.*` envelope — source of truth

The `wm.health.*` envelope schema is **owned by companion-degrade** and consumed by
vision-kin's health digest and homestead's readiness-beacon. `docket digest --format json`
reuses that exact field shape; it does **not** define a parallel envelope.

If companion-degrade ships a Rust crate/type for the envelope, `docket` will depend on it.
Until then, field names are matched exactly to companion-degrade's published JSON contract
and a test asserts conformance.

### SessionStart hook snippet

The following shows how to wire `docket digest` into a SessionStart banner.
**This is documented for user reference only — this PRD does NOT modify the live hook.**
Uncomment and add to your SessionStart hook script when ready:

```sh
# --- docket digest (add to SessionStart hook) ---
# _docket_digest=$(docket digest --format text 2>/dev/null)
# if [ -n "$_docket_digest" ]; then
#   echo "$_docket_digest"
# fi
# -------------------------------------------------
```

Integrating this adds one-to-three lines to the banner showing open/escalated finding counts,
the oldest finding's run-age, and any escalated keys — identical information to what
`docket list` shows, collapsed to banner-safe width.

## License

MIT OR Apache-2.0
