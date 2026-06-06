#!/usr/bin/env bash
# AC3 + AC6 — mocked-provider verification of the burst pod path.
#
# Proves, with NO real cloud spend, that:
#   AC3 — stage logs stream locally and the remote exit code is faithfully
#         propagated: a forced remote failure exits burst-train.sh non-zero
#         with the failing stage named.
#   AC6 — pod lifecycle (create → run → teardown) is delegated to the burst
#         transport and no pod is left running after completion/failure
#         (the mock records up/down events; we assert balanced teardown).
#
# Mechanism: inject a fake `wm-burst` via WM_BURST=. The mock writes a
# lifecycle ledger ($LEDGER) recording POD_UP / RUN / POD_DOWN, runs the
# requested command locally so stage logs stream, and propagates its exit code.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
BURST="$ROOT/burst-train.sh"

PASS=0; FAIL=0
ok()   { echo "  PASS: $*"; PASS=$((PASS+1)); }
bad()  { echo "  FAIL: $*"; FAIL=$((FAIL+1)); }

WORK="$(mktemp -d /tmp/ac3-burst-pod.XXXXXX)"
trap 'rm -rf "$WORK"' EXIT

LEDGER="$WORK/ledger.log"

# ── build the mock wm-burst ──────────────────────────────────────────────────
# Supports the two subcommands burst-train.sh uses: `pod up ... -- CMD...` and
# `exec -- CMD...`. `pod up` records POD_UP, runs CMD, records POD_DOWN
# (always — even on failure, mirroring a real teardown), and propagates CMD's
# exit code. The CMD it runs is the remote train invocation; we shim
# train-wintermute.sh on PATH so we control success/failure per scenario.
MOCKBIN="$WORK/bin"
mkdir -p "$MOCKBIN"
cat > "$MOCKBIN/wm-burst" <<EOF
#!/usr/bin/env bash
set -uo pipefail
LEDGER="$LEDGER"
sub="\${1:-}"; shift || true
case "\$sub" in
  pod)
    # skip 'up' and the cost/duration flags until the -- separator
    while [[ \$# -gt 0 && "\$1" != "--" ]]; do shift; done
    [[ "\${1:-}" == "--" ]] && shift
    echo "POD_UP" >> "\$LEDGER"
    "\$@"; rc=\$?
    echo "POD_DOWN rc=\$rc" >> "\$LEDGER"
    exit \$rc
    ;;
  exec)
    while [[ \$# -gt 0 && "\$1" != "--" ]]; do shift; done
    [[ "\${1:-}" == "--" ]] && shift
    echo "EXEC" >> "\$LEDGER"
    "\$@"; exit \$?
    ;;
  *) echo "mock wm-burst: unknown subcommand \$sub" >&2; exit 2 ;;
esac
EOF
chmod +x "$MOCKBIN/wm-burst"

# A train-wintermute.sh shim whose behaviour is driven by $TRAIN_BEHAVIOR:
#   "ok"   -> emits stage logs, writes a stub ONNX to /tmp/burst-out, exits 0
#   "fail" -> emits stage logs up to the 'augment' stage, then exits 42
mk_train_shim() {
  cat > "$MOCKBIN/train-wintermute.sh" <<'EOF'
#!/usr/bin/env bash
set -uo pipefail
echo "stage: deps    OK"
echo "stage: sync    OK"
echo "stage: augment OK"
if [[ "${TRAIN_BEHAVIOR:-ok}" == "fail" ]]; then
  echo "stage: train   FAILED (synthetic)" >&2
  exit 42
fi
echo "stage: train   OK"
echo "stage: export  OK"
echo "stage: verify  OK"
mkdir -p /tmp/burst-out
# tiny placeholder artifact for the pull step
printf 'STUB-ONNX' > /tmp/burst-out/wintermute.onnx
exit 0
EOF
  chmod +x "$MOCKBIN/train-wintermute.sh"
}
mk_train_shim

run_burst() {
  # $1 = TRAIN_BEHAVIOR ; remaining args forwarded to burst-train.sh
  local behavior="$1"; shift
  : > "$LEDGER"
  PATH="$MOCKBIN:$PATH" \
  WM_BURST="wm-burst" \
  TRAIN_BEHAVIOR="$behavior" \
  WM_AUDIO_MODEL="$WORK/live/wintermute.onnx" \
    bash "$BURST" "$@"
}

# ─────────────────────────────────────────────────────────────────────────────
echo "[$(date -u +%T)Z] T1: forced remote failure → wrapper exits non-zero, names failing stage (AC3)"
out="$(run_burst fail --smoke 2>&1)"; rc=$?
echo "$out" | sed 's/^/    | /'
[[ $rc -ne 0 ]] && ok "T1 wrapper propagated non-zero exit (rc=$rc)" \
                || bad "T1 wrapper should have exited non-zero, got rc=$rc"
# the wrapper's die() names the failing stage / exit code
if echo "$out" | grep -qiE 'exited 42|failing stage|train-wintermute.sh exited'; then
  ok "T1 error message names the remote failure (exit code / failing stage)"
else
  bad "T1 error message did not name the remote failure"
fi
# stage logs streamed locally
echo "$out" | grep -qi 'stage: augment' && ok "T1 remote stage logs streamed locally" \
                                        || bad "T1 stage logs did not stream"

echo "[$(date -u +%T)Z] T2: pod torn down even on failure — no pod left running (AC6)"
ups="$(grep -c '^POD_UP'   "$LEDGER" 2>/dev/null || echo 0)"
downs="$(grep -c '^POD_DOWN' "$LEDGER" 2>/dev/null || echo 0)"
[[ "$ups" == "$downs" && "$ups" -ge 1 ]] \
  && ok "T2 pod lifecycle balanced on failure (POD_UP=$ups POD_DOWN=$downs)" \
  || bad "T2 unbalanced pod lifecycle on failure (POD_UP=$ups POD_DOWN=$downs)"

echo "[$(date -u +%T)Z] T3: successful run completes through pod path; pod torn down (AC3+AC6)"
# Use --smoke so the install gate's heavy verify isn't required; we only assert
# the pod path ran and tore down. burst-train.sh's install_gate will run on the
# stub artifact and is EXPECTED to fail the shape check — that is fine: by then
# the pod has already been torn down, which is what AC6 asserts.
out="$(run_burst ok --smoke 2>&1)"; rc=$?
echo "$out" | sed 's/^/    | /'
ups="$(grep -c '^POD_UP'   "$LEDGER" 2>/dev/null || echo 0)"
downs="$(grep -c '^POD_DOWN' "$LEDGER" 2>/dev/null || echo 0)"
[[ "$ups" -ge 1 ]] && ok "T3 pod was created via burst transport (POD_UP=$ups)" \
                   || bad "T3 pod was never created (POD_UP=$ups)"
[[ "$ups" == "$downs" ]] \
  && ok "T3 pod torn down (POD_UP=$ups == POD_DOWN=$downs) — none left running" \
  || bad "T3 pod left running (POD_UP=$ups != POD_DOWN=$downs)"
# the remote train succeeded → its OK stages streamed and pod_run reported success
echo "$out" | grep -qi 'pod run completed successfully' \
  && ok "T3 wrapper reported successful pod run" \
  || bad "T3 wrapper did not report a successful pod run"

echo
echo "Results: $PASS passed, $FAIL failed"
if [[ $FAIL -eq 0 ]]; then
  echo "PASS: all AC3/AC6 burst-pod tests green"
  exit 0
else
  echo "FAIL: AC3/AC6 burst-pod tests failed"
  exit 1
fi
