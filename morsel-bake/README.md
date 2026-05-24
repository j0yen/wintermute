# morsel-bake

> morsel is the missing-middle Rust crate for embedded ML — but the load-bearing piece for adoption is the offline `bake` CLI that turns trained weights into Rust source.

## Why

morsel is the missing-middle Rust crate for embedded ML — but the load-bearing piece for adoption is the offline `bake` CLI that turns trained weights into Rust source. Without bake, every consumer crate has to hand-write const arrays from numpy dumps. Phase 0a ships only the bake CLI; the runtime nn::Lstm/Conv1d/LogMel primitives are Phase 0b. Carving out bake first means the file format and emission shape are locked before the runtime primitives are designed against them.

## Build

```sh
cargo build --release
```

Produces `target/release/morsel-bake`. Symlink into `~/.local/bin/` if you want it on `$PATH`.

## Usage

```sh
morsel-bake --help
```

## Audience

model author (the author or future-Claude on the author's behalf) running the bake CLI once per trained model, offline, after `safetensors` export. Output is a `weights.rs` file that gets committed into a consumer crate.

## Acceptance criteria

This project was scaffolded from a PRD via the `autobuilder` pipeline. The MUST-level acceptance criteria are:

- **AC1**: `morsel-bake --in <safetensors-file> --arch <name> --out <rust-file>` reads tensors from the safetensors file, emits Rust source containing one `pub const NAME: [[f32; N]; M] = ...;` (or 1-D `[f32; N]`) per tensor, plus `pub const ARCH_F...
- **AC2**: Const arrays preserve f32 precision exactly — round-tripping (write tensor → safetensors → bake → const → parse from emitted source) yields bit-identical f32 bytes. Use Rust's `{:e}` or `f32::from_bits` style if needed to avoid lossy for...
- **AC3**: The emitted `.rs` file compiles standalone (no morsel runtime deps). The test writes the output to a tempdir, wraps it in a minimal `Cargo.toml`+`lib.rs`, runs `cargo build`, and asserts success.
- **AC4**: Tensor names from safetensors that contain `.` or `/` are sanitized to uppercase Rust-identifier-safe names (`weight.ih` → `WEIGHT_IH`, `layer/0/weight` → `LAYER_0_WEIGHT`). Conflicts after sanitization → exit 4 with stderr listing the c...
- **AC5**: Tensors with dtype other than f32 → exit 5 with stderr listing the offending tensor and its dtype, and the message `only f32 supported in Phase 0a`. (f16/bf16/int8 are Phase 0b with --quant flag.)
- **AC6**: Tensors with rank > 2 are emitted as Rust nested arrays one dimension deeper per rank (`[[[f32; D2]; D1]; D0]`). Rank-0 (scalar) tensors emit `pub const NAME: f32 = ...;`. Rank > 4 → exit 6 with stderr `rank N exceeds supported max 4`.
- **AC7**: Non-existent --in file → exit 2 with stderr containing the path and `safetensors file not found`. --out path that already exists → exit 7 unless `--force` is passed (then overwrites).

Each AC has a matching integration test under `tests/acceptance_ac<n>.rs`.

## Provenance

Built via the [`autobuilder`](https://github.com/j0yen/autobuilder) pipeline (PRD intake -> intent-card -> scaffold -> iterate-and-prove). Originally consolidated as a subdir of the [`wintermute`](https://github.com/j0yen/wintermute) monorepo; this standalone repo is a fresh-init snapshot for easier consumption and distribution.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
