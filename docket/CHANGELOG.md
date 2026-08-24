# Changelog

## v0.5.1 — 2026-06-13

Adds `docket stuck` subcommand (PRD-litmus-stuck-detector).

New `stuck` command lists open findings where `report_count >= N` (default 6)
AND `runs_seen >= M` (default 5) AND `resolved_at IS NULL`. These are
"probe-suspect" findings — probes that persistently fire without ever resolving,
suggesting the probe logic may not match reality.

Flags: `--min-reports N`, `--min-runs N`, `--format text|json`, `--key <substr>`
filter, `--include-acked`. Acked findings are excluded by default.

JSON output includes `suspect_reason` field explaining why the finding is
flagged. Text output prints a concise one-liner per finding. Empty result exits
0 with "no probe-suspect findings". Pure read operation — no db writes.

All 7 acceptance criteria covered by 7 new integration tests (67 total pass).

## v0.4.0 — 2026-05-30

Adds typed, accumulated evidence trail for findings (PRD-docket-evidence).

New `evidence` table (M2 migration, append-only) stores typed refs keyed to
finding + run. `docket report --evidence <kind>:<ref>` (repeatable) parses
known prefixes (`recall:`, `journal:`, `pid:`, `provfs:`, `commit:`, `path:`)
into `(kind, ref_val)` rows; unknown prefixes stored as `raw`. Malformed refs
never fail the report.

`docket show <key>` text output gains an *Evidence* section grouped by run;
`--format json` includes `evidence_trail` array. `docket list --format json`
includes `evidence_count` per finding. Migration is idempotent on existing DBs.

All 10 acceptance criteria covered by 10 new integration tests (45 total pass).

## v0.2.0 — 2026-05-30

Adds escalation lifecycle and `docket sweep` command (PRD-docket-escalate).

Findings now graduate from `open` → `escalated` when `consecutive_runs` reaches
the threshold (default 3, overridable via `--escalate-threshold` or
`DOCKET_ESCALATE_THRESHOLD`). Escalation is sticky; only `resolve`/`sweep` exits
it. New `escalated_at` and `escalation_reason` columns record when and why.

New `runs` table maintains an ordered ledger of distinct run-ids, enabling
`docket sweep --run <id> [--stale-after <K>]` to automatically resolve findings
not seen in the last K runs as `resolved(stale)`. Sweep also resets
`consecutive_runs` to 0 on gap detection.

`docket list --escalated` filters to exactly the escalated set.

Migration is idempotent on docket-core (pre-escalate) databases. All 10
acceptance criteria covered by 10 new integration tests (35 total pass).

## v0.3.0 — 2026-05-30

Adds `docket digest [--format text|json] [--severity <min>]` — a compact
rollup of open/escalated findings for SessionStart banners and health checks.

Text output is a ≤3-line banner (open count, crit, escalated, oldest key).
JSON output is a `wm.health.*`-compatible envelope matching companion-degrade's
`ComponentHealth` shape (component / status / summary / detail).

Status mapping: 0 findings → ok; open-only → ok; any escalated → degraded;
any escalated crit → down. All branches covered by tests.

`--severity warn` excludes info-severity findings from counts and summary.
