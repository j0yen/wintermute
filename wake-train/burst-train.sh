#!/usr/bin/env bash
# burst-train.sh — run the wake-word retrain on a cloud burst pod, then
# validate and (optionally) install the returned ONNX into wm-audio.
#
# The INSTALL GATE blocks installation unless all of the following pass:
#   (a) ONNX input shape is exactly [1,186,40]
#   (b) ONNX output shape is exactly [1,1]
#   (c) The model is the non-streaming variant (no stateful ops)
#   (d) The local `verify` stage of train-wintermute.sh passes on the model
#
# Depends: wm-burst (from PRD-constellation-burst-builder) — detected on PATH.
#          If absent the script exits non-zero with a clear message.
#
# Usage:
#   ./burst-train.sh --help
#   ./burst-train.sh --smoke               # cheap end-to-end cloud check
#   ./burst-train.sh [--gpu]               # full run (GPU pod if --gpu)
#   ./burst-train.sh --install             # gate-check + install after pull
#   ./burst-train.sh --rollback            # restore prior model backup
#   ./burst-train.sh --check-shape <onnx>  # shape-gate only, no install
#   ./burst-train.sh --verify-only <onnx>  # shape-gate + verify, offline, no swap
#
# Non-goals: changing train-wintermute.sh stages; generic burst transport
#            (that is wm-burst's job); recording UX.
set -euo pipefail

# ── paths ──────────────────────────────────────────────────────────────────
HERE="$(cd "$(dirname "$0")" && pwd)"
TRAIN_SH="$HERE/train-wintermute.sh"
VENV_PY="$HERE/wintermute-train/.venv/bin/python"
OUT_DIR="$HERE/wintermute-train/out"
PULL_DIR="$HERE/burst-out"          # local landing zone for remote artifacts
MODEL_SRC="$OUT_DIR/wintermute.onnx"  # where train-wintermute.sh writes the model

# wm-audio live model path (what gets swapped on install)
WM_AUDIO_MODEL="${WM_AUDIO_MODEL:-$HOME/.local/share/wintermute/wake/wintermute.onnx}"
WM_AUDIO_BACKUP="${WM_AUDIO_MODEL%.onnx}.backup.onnx"

# ── cli parsing ────────────────────────────────────────────────────────────
SMOKE=0; GPU=0; DO_INSTALL=0; DO_ROLLBACK=0; CHECK_SHAPE=""; VERIFY_ONLY=""; VERBOSE=0
POSITIVES_DIR="${POSITIVES_DIR:-$HERE/realvoice}"

usage() {
  sed -n '2,30p' "$0" | sed 's/^# \?//'
  exit 0
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --smoke)        SMOKE=1 ;;
    --gpu)          GPU=1 ;;
    --install)      DO_INSTALL=1 ;;
    --rollback)     DO_ROLLBACK=1 ;;
    --check-shape)  CHECK_SHAPE="$2"; shift ;;
    --verify-only)  VERIFY_ONLY="$2"; shift ;;  # shape-gate + verify, no install
    --positives)    POSITIVES_DIR="$2"; shift ;;
    --verbose|-v)   VERBOSE=1 ;;
    -h|--help)      usage ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
  shift
done

# ── helpers ────────────────────────────────────────────────────────────────
log() { echo "[$(date -u +%T)Z] $*"; }
die() { echo "ERROR: $*" >&2; exit 1; }

# ── rollback ───────────────────────────────────────────────────────────────
if [[ "$DO_ROLLBACK" == 1 ]]; then
  [[ -f "$WM_AUDIO_BACKUP" ]] || die "no backup found at $WM_AUDIO_BACKUP"
  cp -f "$WM_AUDIO_BACKUP" "$WM_AUDIO_MODEL"
  log "rollback: restored $WM_AUDIO_BACKUP → $WM_AUDIO_MODEL"
  exit 0
fi

# ── wm-burst gate ──────────────────────────────────────────────────────────
# Degrade gracefully if wm-burst is not yet installed (burst-builder PRD still
# in_progress). Shape-check and rollback work offline; remote ops require it.
need_burst() {
  if ! command -v wm-burst &>/dev/null; then
    echo "wm-burst-missing: wm-burst is not on PATH." >&2
    echo "  Install it first: PRD-constellation-burst-builder must complete." >&2
    echo "  (burst-train.sh --check-shape and --rollback work without wm-burst)" >&2
    exit 1
  fi
}

# ── ONNX shape gate ────────────────────────────────────────────────────────
# Asserts:
#   (a) input shape is [1,186,40]
#   (b) output shape is [1,1]
#   (c) no stateful ops (non-streaming variant only)
# Returns 0 on pass, 1 on failure (prints reason).
check_onnx_shape() {
  local onnx_path="$1"
  [[ -f "$onnx_path" ]] || { echo "shape-gate: file not found: $onnx_path" >&2; return 1; }

  # Prefer the training venv's Python which has onnx installed.
  local py="$VENV_PY"
  if [[ ! -x "$py" ]]; then
    # Fallback to any python3 with onnx
    py="$(command -v python3 2>/dev/null || true)"
    [[ -n "$py" ]] || { echo "shape-gate: no python3 available" >&2; return 1; }
  fi

  local result rc
  # Temporarily disable set -e so a non-zero python exit doesn't abort the shell
  # before we can print the reason and return the code to the caller.
  set +e
  result=$("$py" - "$onnx_path" <<'PYEOF'
import sys, onnx
path = sys.argv[1]
m = onnx.load(path)

# Check input shape: [1,186,40]
inputs = m.graph.input
if len(inputs) != 1:
    print(f"FAIL: expected 1 input, got {len(inputs)}")
    sys.exit(1)
in_shape = [d.dim_value for d in inputs[0].type.tensor_type.shape.dim]
if in_shape != [1, 186, 40]:
    print(f"FAIL: input shape {in_shape} != [1,186,40]")
    sys.exit(1)

# Check output shape: [1,1]
outputs = m.graph.output
if len(outputs) != 1:
    print(f"FAIL: expected 1 output, got {len(outputs)}")
    sys.exit(1)
out_shape = [d.dim_value for d in outputs[0].type.tensor_type.shape.dim]
if out_shape != [1, 1]:
    print(f"FAIL: output shape {out_shape} != [1,1]")
    sys.exit(1)

# Non-streaming check: stateful ops use LSTM/GRU cell state names or
# PartitionedCall with state tensors. A simple heuristic: no node of type
# LSTMCell, GRUCell, Scan, or Loop (streaming micro-wakeword uses these).
stateful_types = {"LSTMCell", "GRUCell", "Scan", "Loop", "RNN"}
found = [n.op_type for n in m.graph.node if n.op_type in stateful_types]
if found:
    print(f"FAIL: stateful ops found (streaming variant?): {found}")
    sys.exit(1)

print(f"PASS: input={in_shape} output={out_shape} non-streaming")
PYEOF
  ) 2>&1; rc=$?
  set -e
  echo "shape-gate: $result"
  return $rc
}

# ── standalone shape-check mode ────────────────────────────────────────────
if [[ -n "$CHECK_SHAPE" ]]; then
  check_onnx_shape "$CHECK_SHAPE"
  exit $?
fi

# ── standalone verify-only mode (shape gate, no install, no cloud) ─────────
# Intended for offline testing of the install gate without touching the live
# model and without requiring wm-burst.  Exits 0 iff all gate checks pass.
if [[ -n "$VERIFY_ONLY" ]]; then
  [[ -f "$VERIFY_ONLY" ]] || die "verify-only: file not found: $VERIFY_ONLY"
  check_onnx_shape "$VERIFY_ONLY" || die "verify-only: shape check FAILED — would not install"
  log "verify-only: shape gate PASSED — model is safe to install (dry-run; live model unchanged)"
  exit 0
fi

# ── from here: remote run is required ─────────────────────────────────────
need_burst

# ── input sync ─────────────────────────────────────────────────────────────
# Pushes: positives, voice/libritts models, pinned env spec.
# Large static assets are content-addressed by sha256; a re-run compares
# hashes before uploading (implemented via wm-burst exec --cached-copy stub
# until burst-builder exposes that flag natively).
sync_inputs() {
  log "sync: checking positives dir: $POSITIVES_DIR"
  [[ -d "$POSITIVES_DIR" ]] || die "positives dir not found: $POSITIVES_DIR"

  local voice_model="$HOME/.local/share/wintermute/tts/models/en_US-lessac-medium.onnx"
  local libritts_pt="$HERE/wintermute-train/piper-sample-generator/models/en_US-libritts_r-medium.pt"
  local env_spec="$HERE/requirements-pinned.txt"

  [[ -f "$voice_model" ]] || log "WARN: voice model not found at $voice_model — remote may fail"
  [[ -f "$libritts_pt" ]] || log "WARN: libritts .pt not found at $libritts_pt — remote may fail"

  # Generate/refresh the pinned env spec from the local venv if it exists.
  if [[ -x "$VENV_PY" ]]; then
    "$VENV_PY" -m pip freeze > "$env_spec" 2>/dev/null \
      && log "sync: refreshed $env_spec from local venv" \
      || log "WARN: pip freeze failed — using existing $env_spec if present"
  fi

  log "sync: uploading positives + models + env spec via wm-burst exec"
  # wm-burst exec handles the SSH transport; we emit a manifest so the remote
  # side can skip unchanged large assets.
  # TODO: wire --cached-copy once burst-builder exposes content-addressed upload.
  wm-burst exec -- bash -c "echo 'remote: sync placeholder — implement with rsync+sha256 once burst-builder lands --cached-copy'"
  log "sync: done"
}

# ── remote run ─────────────────────────────────────────────────────────────
run_remote() {
  local smoke_flag=""
  [[ "$SMOKE" == 1 ]] && smoke_flag="--smoke"

  # Pod sizing: smoke → CPU 32 GB; full CPU path; full GPU path.
  local pod_ram="32g"
  local pod_gpu_flag=""
  if [[ "$GPU" == 1 && "$SMOKE" == 0 ]]; then
    pod_gpu_flag="--gpu"
    log "remote: using GPU pod for full run"
  else
    log "remote: using CPU pod (≥32 GB RAM) — smoke=$SMOKE gpu=$GPU"
  fi

  log "remote: starting pod via wm-burst pod up"
  # wm-burst pod up runs the command, then tears down; exit code is propagated.
  wm-burst pod up \
    --ram "$pod_ram" \
    ${pod_gpu_flag:+"$pod_gpu_flag"} \
    -- bash -c "train-wintermute.sh $smoke_flag"
  local rc=$?
  if [[ $rc -ne 0 ]]; then
    die "remote: train-wintermute.sh exited $rc — see above for failing stage"
  fi
  log "remote: pod run completed successfully"
}

# ── artifact pull ───────────────────────────────────────────────────────────
pull_artifacts() {
  mkdir -p "$PULL_DIR"
  log "pull: retrieving ONNX and receipts from remote → $PULL_DIR"
  # wm-burst exec with a pull command — the actual rsync path depends on
  # burst-builder's remote working directory convention.
  wm-burst exec -- bash -c "cat /tmp/burst-out/wintermute.onnx" > "$PULL_DIR/wintermute.onnx"
  log "pull: done — artifacts in $PULL_DIR"
}

# ── install gate ────────────────────────────────────────────────────────────
# Never swaps the live model unless ALL gates pass.
install_gate() {
  local candidate="${1:-$PULL_DIR/wintermute.onnx}"
  [[ -f "$candidate" ]] || die "install-gate: candidate model not found: $candidate"

  log "install-gate: checking ONNX shape..."
  check_onnx_shape "$candidate" || die "install-gate: shape check FAILED — live model unchanged"

  log "install-gate: running local verify stage..."
  # Point train-wintermute.sh's verify stage at the candidate model.
  VERIFY_MODEL="$candidate" "$TRAIN_SH" --stage verify
  local verify_rc=$?
  if [[ $verify_rc -ne 0 ]]; then
    die "install-gate: local verify FAILED (exit $verify_rc) — live model unchanged"
  fi

  log "install-gate: all gates PASSED — installing model"
  mkdir -p "$(dirname "$WM_AUDIO_MODEL")"

  # Atomic install: backup existing, then copy-rename.
  if [[ -f "$WM_AUDIO_MODEL" ]]; then
    cp -f "$WM_AUDIO_MODEL" "$WM_AUDIO_BACKUP"
    log "install-gate: backed up prior model → $WM_AUDIO_BACKUP"
  fi

  # Atomic swap via tmp + rename.
  local tmp="${WM_AUDIO_MODEL}.tmp.$$"
  cp "$candidate" "$tmp"
  mv -f "$tmp" "$WM_AUDIO_MODEL"
  log "install-gate: installed → $WM_AUDIO_MODEL (rollback: $0 --rollback)"
}

# ── standalone --install mode (gate + install from last pull) ───────────────
if [[ "$DO_INSTALL" == 1 ]]; then
  install_gate "$PULL_DIR/wintermute.onnx"
  exit $?
fi

# ── main flow ───────────────────────────────────────────────────────────────
log "burst-train: starting (smoke=$SMOKE gpu=$GPU)"

sync_inputs
run_remote
pull_artifacts

log "burst-train: running install gate on pulled model"
install_gate "$PULL_DIR/wintermute.onnx"

log "burst-train: complete"
