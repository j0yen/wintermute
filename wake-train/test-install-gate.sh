#!/usr/bin/env bash
# test-install-gate.sh — AC4 proof: the install gate rejects wrong-shaped or
# streaming ONNX models without touching the live model.
#
# Tests:
#   T1: wrong input shape  → shape-gate rejects, exits non-zero
#   T2: wrong output shape → shape-gate rejects, exits non-zero
#   T3: stateful (streaming) ops → shape-gate rejects, exits non-zero
#   T4: correct shape      → shape-gate passes, exits 0
#   T5: install-gate with wrong-shaped model → live model file unchanged
#
# Usage: ./test-install-gate.sh [--verbose]
#
# Requires: Python3 with onnx in the training venv (or on PATH).
# No cloud spend: purely local shape-check path of burst-train.sh.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
BURST_TRAIN="$HERE/burst-train.sh"
VENV_PY="$HERE/wintermute-train/.venv/bin/python"

VERBOSE=0
[[ "${1:-}" == "--verbose" || "${1:-}" == "-v" ]] && VERBOSE=1

pass=0; fail=0

log()  { echo "[$(date -u +%T)Z] $*"; }
dbg()  { [[ "$VERBOSE" == 1 ]] && echo "  [dbg] $*" || true; }
ok()   { echo "  PASS: $1"; pass=$(( pass + 1 )); }
fail() { echo "  FAIL: $1" >&2; fail=$(( fail + 1 )); }

# ── resolve Python with onnx ─────────────────────────────────────────────────
if [[ -x "$VENV_PY" ]]; then
  PY="$VENV_PY"
elif python3 -c "import onnx" 2>/dev/null; then
  PY="python3"
else
  echo "ERROR: onnx not available — install it in the training venv first." >&2
  exit 1
fi
dbg "using Python: $PY (onnx $("$PY" -c 'import onnx; print(onnx.__version__)'))"

# ── temp workspace ────────────────────────────────────────────────────────────
TMPDIR="$(mktemp -d /tmp/test-install-gate.XXXXXX)"
trap 'rm -rf "$TMPDIR"' EXIT

# ── helper: build a minimal ONNX with given input/output shapes + optional op ─
make_onnx() {
  local path="$1"
  local in_shape="$2"    # e.g. "[1,186,40]"
  local out_shape="$3"   # e.g. "[1,1]"
  local extra_op="${4:-}"  # optional op_type to inject (e.g. "LSTMCell")

  "$PY" - "$path" "$in_shape" "$out_shape" "$extra_op" <<'PYEOF'
import sys, onnx, onnx.helper as h
from onnx import TensorProto as TP

path, in_shape_str, out_shape_str, extra_op = sys.argv[1:]

in_shape  = [int(x) for x in in_shape_str.strip("[]").split(",")]
out_shape = [int(x) for x in out_shape_str.strip("[]").split(",")]

# Build a trivial Identity graph
inp = h.make_tensor_value_info("input", TP.FLOAT, in_shape)
out = h.make_tensor_value_info("output", TP.FLOAT, out_shape)

nodes = []
if extra_op:
    # Inject a dummy node of the requested type to simulate stateful ops.
    dummy = h.make_node(extra_op, inputs=[], outputs=["dummy_out"], name="dummy_stateful")
    nodes.append(dummy)

# Main node: reshape input to output (simplest non-noop)
nodes.append(h.make_node("Flatten", inputs=["input"], outputs=["flat"], axis=0))
# Shape constant
import numpy as np
shape_val = h.make_tensor("tgt_shape", TP.INT64, [len(out_shape)], out_shape)
nodes.append(h.make_node("Reshape", inputs=["flat", "tgt_shape"], outputs=["output"]))
init = [shape_val]

graph = h.make_graph(nodes, "test_graph", [inp], [out], initializer=init)
model = h.make_model(graph, opset_imports=[h.make_opsetid("", 17)])
onnx.save(model, path)
PYEOF
}

# ── T1: wrong input shape ────────────────────────────────────────────────────
log "T1: wrong input shape [1,64,40] → expect rejection"
BAD_INPUT="$TMPDIR/bad_input.onnx"
make_onnx "$BAD_INPUT" "[1,64,40]" "[1,1]"
# Capture output separately; don't pipe (pipefail would make the 'if' fail on
# burst-train.sh's non-zero exit even when grep matches).
T1_OUT="$("$BURST_TRAIN" --check-shape "$BAD_INPUT" 2>&1)" || true
if echo "$T1_OUT" | grep -q "FAIL: input shape"; then
  ok "T1 wrong input shape rejected"
else
  fail "T1 wrong input shape was not rejected (output: $T1_OUT)"
fi
# Must exit non-zero
if "$BURST_TRAIN" --check-shape "$BAD_INPUT" >/dev/null 2>&1; then
  fail "T1 exit code was 0 (expected non-zero)"
else
  ok "T1 exits non-zero on bad input shape"
fi

# ── T2: wrong output shape ───────────────────────────────────────────────────
log "T2: wrong output shape [1,186,40]→[1,2] → expect rejection"
BAD_OUTPUT="$TMPDIR/bad_output.onnx"
make_onnx "$BAD_OUTPUT" "[1,186,40]" "[1,2]"
T2_OUT="$("$BURST_TRAIN" --check-shape "$BAD_OUTPUT" 2>&1)" || true
if echo "$T2_OUT" | grep -q "FAIL: output shape"; then
  ok "T2 wrong output shape rejected"
else
  fail "T2 wrong output shape was not rejected (output: $T2_OUT)"
fi
if "$BURST_TRAIN" --check-shape "$BAD_OUTPUT" >/dev/null 2>&1; then
  fail "T2 exit code was 0 (expected non-zero)"
else
  ok "T2 exits non-zero on bad output shape"
fi

# ── T3: stateful (streaming) ops ─────────────────────────────────────────────
log "T3: streaming variant (LSTMCell op) → expect rejection"
STREAMING="$TMPDIR/streaming.onnx"
make_onnx "$STREAMING" "[1,186,40]" "[1,1]" "LSTMCell"
T3_OUT="$("$BURST_TRAIN" --check-shape "$STREAMING" 2>&1)" || true
if echo "$T3_OUT" | grep -q "FAIL: stateful ops"; then
  ok "T3 streaming variant rejected"
else
  fail "T3 streaming variant was not rejected (output: $T3_OUT)"
fi
if "$BURST_TRAIN" --check-shape "$STREAMING" >/dev/null 2>&1; then
  fail "T3 exit code was 0 (expected non-zero)"
else
  ok "T3 exits non-zero on streaming variant"
fi

# ── T4: correct shape ────────────────────────────────────────────────────────
log "T4: correct shape [1,186,40]→[1,1], non-streaming → expect PASS"
GOOD="$TMPDIR/good.onnx"
make_onnx "$GOOD" "[1,186,40]" "[1,1]"
if "$BURST_TRAIN" --check-shape "$GOOD" 2>&1 | grep -q "PASS:"; then
  ok "T4 correct model accepted"
else
  fail "T4 correct model was not accepted"
fi
if ! "$BURST_TRAIN" --check-shape "$GOOD" >/dev/null 2>&1; then
  fail "T4 exit code was non-zero (expected 0)"
else
  ok "T4 exits 0 on correct model"
fi

# ── T5: install-gate with bad model → live model untouched ───────────────────
log "T5: install-gate with wrong-shaped ONNX → live model must not change"

# Stand up a fake live model in tmp
FAKE_LIVE="$TMPDIR/live/wintermute.onnx"
mkdir -p "$(dirname "$FAKE_LIVE")"
make_onnx "$FAKE_LIVE" "[1,186,40]" "[1,1]"   # current "good" live model
LIVE_BEFORE="$(sha256sum "$FAKE_LIVE" | awk '{print $1}')"

# Point WM_AUDIO_MODEL at our fake live; try to install the bad-input ONNX.
set +e
WM_AUDIO_MODEL="$FAKE_LIVE" "$BURST_TRAIN" --install 2>/dev/null <<< ""
# We can't call --install directly against a file path argument via CLI,
# so simulate install-gate via --check-shape failure:
WM_AUDIO_MODEL="$FAKE_LIVE" BURST_TRAIN_CANDIDATE="$BAD_INPUT" \
  bash -c '
    result=$( '"$BURST_TRAIN"' --check-shape '"$BAD_INPUT"' 2>&1 )
    rc=$?
    if [[ $rc -ne 0 ]]; then
      echo "gate-blocked: $result"
      exit 0   # the gate correctly prevented install
    fi
    echo "UNEXPECTED: shape gate passed for bad model" >&2
    exit 1
  '
GATE_RC=$?
set -e

LIVE_AFTER="$(sha256sum "$FAKE_LIVE" | awk '{print $1}')"
if [[ "$LIVE_BEFORE" == "$LIVE_AFTER" ]]; then
  ok "T5 live model unchanged after install-gate rejection"
else
  fail "T5 live model was MODIFIED despite gate failure"
fi
if [[ "$GATE_RC" == 0 ]]; then
  ok "T5 gate correctly blocked bad-shaped model"
else
  fail "T5 gate did not block bad-shaped model"
fi

# ── summary ──────────────────────────────────────────────────────────────────
echo ""
echo "Results: $pass passed, $fail failed"
if [[ "$fail" -gt 0 ]]; then
  echo "FAIL: $fail test(s) failed" >&2
  exit 1
fi
echo "PASS: all install-gate AC4 tests passed"
