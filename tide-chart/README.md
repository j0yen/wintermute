# tide-chart

> Have an instrument-like, glanceable view of the laptop-day's rhythm so the shape of a day (focus/fragmentation/velocity/surprise) becomes legible information instead of merely lived experience.

## Why

Have an instrument-like, glanceable view of the laptop-day's rhythm so the shape of a day (focus/fragmentation/velocity/surprise) becomes legible information instead of merely lived experience. Phase 0 proves the data pipeline + signal calibration in terminal before committing to e-ink hardware (Phase 2).

## Build

```sh
cargo build --release
```

Produces `target/release/tide-chart`. Symlink into `~/.local/bin/` if you want it on `$PATH`.

## Usage

```sh
tide-chart --help
```

## Audience

the author on the wintermute laptop, running it after a day's work to inspect the four signals plotted over the day's hours. Runs from terminal; consumed as ASCII via stdout.

## Acceptance criteria

This project was scaffolded from a PRD via the `autobuilder` pipeline. The MUST-level acceptance criteria are:

- **AC1**: `tide-chart collect --root <dir>` reads ctrace-style ndjson event files under <dir>/events/, aggregates events into hour buckets across the last 24 hours, computes four per-bucket scalars (focus, fragmentation, velocity, surprise) z-scor...
- **AC2**: `tide-chart chart --today --root <dir>` reads `<dir>/tide.db` and prints a 4-line ASCII chart for the last 24 hours to stdout: one line per signal, each line 24 columns wide, using one of `▁▂▃▄▅▆▇█` or `.+*#` characters to encode normali...
- **AC3**: Calling `tide-chart chart` against a `--root` directory that has no `tide.db` exits non-zero with stderr message containing both the missing path and the suggestion `run collect first`.
- **AC4**: Both subcommands respect `--root <dir>` so two concurrent invocations under separate roots never write to each other's database or read each other's events. Default root is `$XDG_DATA_HOME/tide-chart` (or `~/.local/share/tide-chart` if u...
- **AC5**: `tide-chart collect` is idempotent over the same input events: running it twice produces identical rows in `signals` (no duplicates, no drift). The collector uses a stable INSERT OR REPLACE keyed on bucket_iso.

Each AC has a matching integration test under `tests/acceptance_ac<n>.rs`.

## Provenance

Built via the [`autobuilder`](https://github.com/j0yen/autobuilder) pipeline (PRD intake -> intent-card -> scaffold -> iterate-and-prove). Originally consolidated as a subdir of the [`wintermute`](https://github.com/j0yen/wintermute) monorepo; this standalone repo is a fresh-init snapshot for easier consumption and distribution.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
