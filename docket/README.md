# docket

A structured ledger for standing findings. A finding reported twice is the same finding.

`docket` is a small SQLite-backed CLI that deduplicates findings by a stable key, tracks
first/last-seen timestamps, and maintains a consecutive-run streak — so a recurring
discovery that appears across many self-review runs is counted once with a growing streak
rather than cluttering a journal as seven separate entries.

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

Single table `findings`:

| column             | type    | description                                                   |
|--------------------|---------|---------------------------------------------------------------|
| `key`              | TEXT PK | stable slug — `agorabus-stale-binary`                         |
| `title`            | TEXT    | human one-liner (latest report wins)                          |
| `severity`         | TEXT    | `info` / `warn` (default) / `crit`                           |
| `status`           | TEXT    | `open` / `resolved`                                           |
| `first_seen`       | TEXT    | RFC3339 timestamp of first report                             |
| `last_seen`        | TEXT    | RFC3339 timestamp of most recent report                       |
| `first_run`        | TEXT    | run-id of the first report                                    |
| `last_run`         | TEXT    | run-id of the most recent report                              |
| `runs_seen`        | INTEGER | count of distinct run-ids that reported this finding          |
| `consecutive_runs` | INTEGER | current streak (distinct sequential run-ids)                  |
| `report_count`     | INTEGER | total raw report calls                                        |
| `resolved_at`      | TEXT    | RFC3339 timestamp of resolution (null if open)                |
| `resolve_reason`   | TEXT    | reason string (null if not provided)                          |
| `evidence`         | TEXT    | raw evidence reference string (null if not provided)          |

WAL mode is enabled; `BEGIN IMMEDIATE` transactions guard concurrent writers.

## Run model

A **run** is a caller-supplied opaque string (e.g. `2026-05-29.1`) passed via `--run`.

- Reporting the same key **twice in the same run-id**: `report_count` increments, `runs_seen`
  and `consecutive_runs` do not. Idempotent for streak purposes.
- Reporting the same key in a **new run-id**: both `runs_seen` and `consecutive_runs`
  increment.
- Reporting a **resolved** finding: it is reopened (`status=open`, streak reset to 1,
  `resolved_at` cleared).

Streak gap-detection (did the previous run miss this finding?) is intentionally deferred
to `docket-escalate`.

## Commands

### `report`

Upsert a finding.

```sh
docket report --run <id> --key <slug> --title <text> [--severity info|warn|crit] [--evidence <ref>]
```

- Creates the finding if new (`status=open`, streak=1, `runs_seen=1`, `report_count=1`).
- Bumps an existing open finding with the same run-id: increments `report_count` + updates `last_seen` only.
- Bumps an existing open finding with a new run-id: increments `runs_seen`, `consecutive_runs`, `report_count`, updates `last_seen`/`last_run`.
- Reopens a resolved finding: resets `consecutive_runs=1`, clears `resolved_at`/`resolve_reason`.

### `list`

List findings.

```sh
docket list [--open|--resolved|--all] [--format text|json] [--severity info|warn|crit]
```

Default: `--open --format text`. `--severity` is a minimum filter (inclusive).

### `show`

Show a single finding's full record.

```sh
docket show <key> [--format text|json]
```

Exits nonzero if the key is unknown.

### `resolve`

Mark a finding as resolved.

```sh
docket resolve <key> [--reason <text>]
```

Idempotent. Exits nonzero if the key is unknown.

## Worked example

```sh
# Report a finding in run 2026-05-29.1
docket report --run 2026-05-29.1 --key agorabus-stale-binary \
  --title "agorabus daemon binary is stale (hash mismatch)" --severity warn

# List open findings
docket list --open
# [open] agorabus-stale-binary (warn)
#   title: agorabus daemon binary is stale (hash mismatch)
#   first_seen: 2026-05-29T...  last_seen: 2026-05-29T...
#   ...  runs_seen: 1  consecutive_runs: 1  report_count: 1

# Show JSON record
docket show agorabus-stale-binary --format json | jq .

# Report again in a later run
docket report --run 2026-05-29.2 --key agorabus-stale-binary \
  --title "agorabus daemon binary is stale (hash mismatch)"
# consecutive_runs is now 2

# Resolve
docket resolve agorabus-stale-binary --reason "rebuilt and restarted"

# Confirm resolved
docket list --resolved --format json | jq '.[0].resolve_reason'
# "rebuilt and restarted"
```

## License

MIT OR Apache-2.0
