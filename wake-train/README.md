# wintermute-audio: companion config and helpers

The Rust daemon (`wm-audio`) is one piece of the audio stack. To make it
useful you also need a PipeWire echo-cancel config, NoiseTorch-ng for
keyboard/fan rejection, and (later) the pretrained `wm-models` package
for wake-word + VAD + STT ONNX bundles.

This directory ships the hand-rolled pieces. Everything here is
GPL-3-compatible at runtime (NoiseTorch is GPL-3), but the scripts and
configs themselves are MIT/Apache-2.0 to match the parent repo.

## What lives here

| Path | Purpose | Install location |
|---|---|---|
| `pipewire/99-wintermute.conf` | echo-cancel drop-in (PRD §2.1) | `~/.config/pipewire/pipewire.conf.d/` |
| `bin/wm-noise` | NoiseTorch-ng `on / off / status` helper (PRD §2.2) | `~/.local/bin/` |
| `wm-models/PKGBUILD` | pretrained ONNX bundle (PRD §2.5) | `/usr/share/wintermute/models/` |

## Install

```sh
# PipeWire AEC
mkdir -p ~/.config/pipewire/pipewire.conf.d
install -m 644 contrib/pipewire/99-wintermute.conf \
  ~/.config/pipewire/pipewire.conf.d/99-wintermute.conf
systemctl --user restart pipewire

# NoiseTorch-ng (Arch / AUR)
yay -S noisetorch-ng-bin    # or paru -S, makepkg, …

# wm-noise helper
install -m 755 contrib/bin/wm-noise ~/.local/bin/wm-noise
```

After install, `wm-noise on` loads the NoiseTorch virtual source;
`wm-audio` automatically prefers it when present and falls back to the
PipeWire AEC-cancelled `wm-mic-cancelled` source otherwise.

## AEC3 detection

The drop-in requests `webrtc.aec3 = true`. If the local `pipewire`
package was built without AEC3, `wm-audio` logs a warning at startup
and the classic WebRTC AEC engages. To verify:

```sh
pw-cli list-modules | grep -i echo
journalctl --user -u wm-audio -b | grep -i aec3
```

## burst-train.sh — cloud wake-word retrain with a validated install gate

The full wake-word retrain OOM-kills on this no-swap laptop (11.2 GB peak) and
takes hours on CPU. `burst-train.sh` runs `train-wintermute.sh` on an on-demand
GPU burst pod (via `wm-burst`), pulls back the ONNX + receipts, and **refuses to
install** unless the model is shaped exactly `[1,186,40] → [1,1]`, is the
non-streaming variant, and passes the local `verify` stage. A bad run leaves the
live model untouched.

```sh
./burst-train.sh --smoke            # cheap end-to-end cloud proof (CPU shape)
./burst-train.sh                    # full GPU run → gate → atomic install
./burst-train.sh --check-shape M    # offline shape gate only
./burst-train.sh --verify-only M    # shape + verify, no swap
./burst-train.sh --rollback         # restore the prior model in one command
```

The pinned training env lives in `requirements-pinned.txt` (regenerated only when
a non-empty `pip freeze` succeeds — never truncated on a pip-less venv). Pod
lifecycle/cost/teardown is delegated to `wm-burst pod`. The
`wake-retrain-burst.service` drop-in calls this instead of a local train,
superseding the OOM-prone local retrain unit. Mocked-provider tests under
`tests/` prove AC3–AC6 with no cloud spend:
`tests/burst-pod.sh`, `tests/install-gate.sh`, `tests/rollback.sh`.

## wm-models

```sh
cd contrib/wm-models
./update-hashes.sh         # one-time: replace SKIP placeholders
makepkg -si --noconfirm
```

See `wm-models/README.md` for the source URL list and licensing.
The shipped PKGBUILD uses `SKIP` sha256sums so it parses anywhere;
`update-hashes.sh` calls `updpkgsums` (from `pacman-contrib`) to
materialize real hashes before publishing.
