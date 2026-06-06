#!/usr/bin/env bash
# agorabus-session-start.sh — SessionStart hook for cross-session pub/sub.
#
# Idempotently:
#   - Starts the agorabus daemon if not already running.
#   - Spawns a long-lived `subscribe` so this session appears in `peers`
#     and receives published events.
#   - Appends incoming events to ~/.cache/agorabus/sessions/<sid>.ndjson
#     so the model can tail its own inbox.
#
# Emits a brief banner listing other peers on the bus (if any). Never
# blocks Claude startup; always exits 0.
#
# --- 2026-05-25 journal §Notable excerpt (verbatim) ---
# "PID 917's two subscribers (1888, 2091) are alive and the daemon binary
# IS post-fix, so the cause is a daemon-not-ready race at boot, not the
# pre-fix collision bug. The agorabus_orphan_subscriber playbook detected
# the state but had to escalate because no programmatic re-attach exists.
# Extending the hook's socket-wait to ~0.5s×N or adding a peer-record-
# explicit re-announce after the subscribe handshake would prevent this."
#
# This comment block is kept so future Claudes reading the script
# understand why the wait windows (10×0.3s for socket, 10×0.3s+5×0.3s
# for peer-record polling) are what they are (AC9).
# --- end excerpt ---

set -uo pipefail

agorabus=/home/jsy/.local/bin/agorabus
cache=/home/jsy/.cache/agorabus
sessions="$cache/sessions"
sock="$cache/sock"
daemon_log="$cache/daemon.log"
handshake_dir="$cache/handshake"

[ -x "$agorabus" ] || exit 0
mkdir -p "$sessions" "$handshake_dir" 2>/dev/null || exit 0

# --- Handshake log helpers ---
# We write to a tmp log first (before we know $sid), then finalize once $sid
# is derived.
_tmp_log="$handshake_dir/.tmp-$$"
_handshake_log="$_tmp_log"
_log_sid="unknown"

# Bounded log rotation: delete handshake log files older than 14 days.
find "$handshake_dir" -maxdepth 1 -name '*.log' -mtime +14 -delete 2>/dev/null || true

# log_handshake phase attempt_n result elapsed_ms
log_handshake() {
    local phase="$1" attempt_n="$2" result="$3" elapsed_ms="$4"
    local now
    now=$(date -u +%Y%m%dT%H%M%SZ)
    printf '{"ts":"%s","sid":"%s","phase":"%s","attempt_n":%s,"result":"%s","elapsed_ms":%s}\n' \
        "$now" "$_log_sid" "$phase" "$attempt_n" "$result" "$elapsed_ms" \
        >> "$_handshake_log"
}

# --- Bring up the daemon if missing (extended socket-wait: 10×0.3s ≈ 3s) ---
_phase_start=$(date -u +%s%3N 2>/dev/null || echo 0)
if ! pgrep -f 'agorabus daemon' >/dev/null 2>&1; then
    nohup "$agorabus" daemon >"$daemon_log" 2>&1 &
    disown
    for _ in $(seq 1 10); do
        [ -S "$sock" ] && break
        sleep 0.3
    done
    _phase_end=$(date -u +%s%3N 2>/dev/null || echo 0)
    _elapsed=$(( _phase_end - _phase_start ))
    if [ -S "$sock" ]; then
        log_handshake "daemon_up" 1 "ok" "$_elapsed"
    else
        log_handshake "daemon_up" 1 "fail" "$_elapsed"
        # Fail-open: log and continue; self-review will detect.
        exit 0
    fi
else
    log_handshake "daemon_up" 1 "ok" 0
fi

# Find Claude's PID. Hook may be invoked directly (PPID=claude) or via
# a wrapper shell (PPID=sh, grandparent=claude). Walk up one if needed.
root="$PPID"
if [ -r "/proc/$PPID/comm" ]; then
    parent_comm=$(cat "/proc/$PPID/comm" 2>/dev/null || true)
    if [ "$parent_comm" != "claude" ]; then
        grand=$(awk '{print $4}' "/proc/$PPID/stat" 2>/dev/null || true)
        if [ -n "$grand" ] && [ "$grand" != "1" ]; then
            root="$grand"
        fi
    fi
fi

cwd="${CLAUDE_PROJECT_DIR:-$PWD}"
project=$(basename "$cwd")

# Derive session-id from the kernel agentns when available (wintermute kernel).
# The kernel writes a stable 32-char hex id into /proc/self/agent_session once
# unshare(CLONE_NEWAGENT) has been called (via agentns-claude). Falls back to
# the PID-based synthesis on stock kernels or when the id reads all zeros.
sid_kernel=$(cat /proc/self/agent_session 2>/dev/null || true)
if [[ -n "$sid_kernel" ]] && [[ "$sid_kernel" != "00000000000000000000000000000000" ]]; then
    # Use first 16 hex chars (64 bits) as a compact stable prefix.
    sid="claude-${sid_kernel:0:16}-${project}"
else
    # Fallback: PID-based synthesis (stock kernels / pre-agentns-claude sessions).
    sid="claude-${root}-${project}"
fi

# Now we know $sid — finalize the handshake log file.
_handshake_log="$handshake_dir/${sid}.log"
_log_sid="$sid"

# Flush the tmp log (daemon_up entry) to the real file.
if [ -f "$_tmp_log" ]; then
    cat "$_tmp_log" >> "$_handshake_log"
    rm -f "$_tmp_log"
fi

# Helper: check if a session_id appears in agorabus peers.
_peer_present() {
    local target_sid="$1"
    "$agorabus" peers 2>/dev/null \
        | jq -e --arg s "$target_sid" 'any(.[]; .session_id == $s)' >/dev/null 2>&1
}

# --- Verified subscriber attach ---
_sub_log="$sessions/${sid}.ndjson"

if pgrep -f "agorabus subscribe --session-id $sid" >/dev/null 2>&1 \
   && _peer_present "$sid"; then
    log_handshake "sub_attach" 1 "already_attached:ok" 0
else
    # Spawn subscriber if not already running.
    if ! pgrep -f "agorabus subscribe --session-id $sid" >/dev/null 2>&1; then
        setsid bash -c "exec '$agorabus' subscribe --session-id '$sid' '' >>'$_sub_log' 2>&1" </dev/null &
        disown
        sleep 0.2
    fi
    # Poll for peer record (10×0.3s = 3s).
    _phase_start=$(date -u +%s%3N 2>/dev/null || echo 0)
    _sub_ok=0
    for _attempt in $(seq 1 10); do
        if _peer_present "$sid"; then
            _phase_end=$(date -u +%s%3N 2>/dev/null || echo 0)
            _elapsed=$(( _phase_end - _phase_start ))
            log_handshake "sub_attach" "$_attempt" "ok" "$_elapsed"
            _sub_ok=1
            break
        fi
        sleep 0.3
    done
    if [ "$_sub_ok" -eq 0 ]; then
        _phase_end=$(date -u +%s%3N 2>/dev/null || echo 0)
        _elapsed=$(( _phase_end - _phase_start ))
        log_handshake "sub_attach" 10 "fail" "$_elapsed"
        # Re-spawn once and poll another 5×0.3s.
        setsid bash -c "exec '$agorabus' subscribe --session-id '$sid' '' >>'$_sub_log' 2>&1" </dev/null &
        disown
        sleep 0.2
        _respawn_start=$(date -u +%s%3N 2>/dev/null || echo 0)
        _sub_ok=0
        for _attempt in $(seq 11 15); do
            if _peer_present "$sid"; then
                _respawn_end=$(date -u +%s%3N 2>/dev/null || echo 0)
                _elapsed=$(( _respawn_end - _respawn_start ))
                log_handshake "sub_attach" "$_attempt" "ok" "$_elapsed"
                _sub_ok=1
                break
            fi
            sleep 0.3
        done
        if [ "$_sub_ok" -eq 0 ]; then
            _respawn_end=$(date -u +%s%3N 2>/dev/null || echo 0)
            _elapsed=$(( _respawn_end - _respawn_start ))
            log_handshake "sub_attach" 15 "fail:exhausted" "$_elapsed"
            # Fail-open: startup is not blocked.
        fi
    fi
fi

# Spawn the RPC worker so this session auto-replies to ping / self.describe /
# methods.list / delegate.run on rpc.req.<sid>. Worker is idempotent.
# Verified attach: poll for ${sid}-worker peer record.
worker="$HOME/.claude/scripts/agorabus-worker.sh"
worker_sid="${sid}-worker"

if [ -x "$worker" ]; then
    if pgrep -f "agorabus-worker.sh $sid\$" >/dev/null 2>&1 \
       && _peer_present "$worker_sid"; then
        log_handshake "worker_attach" 1 "already_attached:ok" 0
    else
        if ! pgrep -f "agorabus-worker.sh $sid\$" >/dev/null 2>&1; then
            workers="$cache/workers"
            mkdir -p "$workers" 2>/dev/null || true
            spawn_log="$workers/${sid}.spawn.log"
            setsid bash -c "exec '$worker' '$sid' '$cwd' >>'$spawn_log' 2>&1" </dev/null &
            disown
        fi
        # Poll for worker peer record (10×0.3s = 3s).
        _phase_start=$(date -u +%s%3N 2>/dev/null || echo 0)
        _worker_ok=0
        for _attempt in $(seq 1 10); do
            if _peer_present "$worker_sid"; then
                _phase_end=$(date -u +%s%3N 2>/dev/null || echo 0)
                _elapsed=$(( _phase_end - _phase_start ))
                log_handshake "worker_attach" "$_attempt" "ok" "$_elapsed"
                _worker_ok=1
                break
            fi
            sleep 0.3
        done
        if [ "$_worker_ok" -eq 0 ]; then
            _phase_end=$(date -u +%s%3N 2>/dev/null || echo 0)
            _elapsed=$(( _phase_end - _phase_start ))
            log_handshake "worker_attach" 10 "fail" "$_elapsed"
            # Re-spawn once and poll another 5×0.3s.
            workers="$cache/workers"
            mkdir -p "$workers" 2>/dev/null || true
            spawn_log="$workers/${sid}.spawn.log"
            setsid bash -c "exec '$worker' '$sid' '$cwd' >>'$spawn_log' 2>&1" </dev/null &
            disown
            _respawn_start=$(date -u +%s%3N 2>/dev/null || echo 0)
            _worker_ok=0
            for _attempt in $(seq 11 15); do
                if _peer_present "$worker_sid"; then
                    _respawn_end=$(date -u +%s%3N 2>/dev/null || echo 0)
                    _elapsed=$(( _respawn_end - _respawn_start ))
                    log_handshake "worker_attach" "$_attempt" "ok" "$_elapsed"
                    _worker_ok=1
                    break
                fi
                sleep 0.3
            done
            if [ "$_worker_ok" -eq 0 ]; then
                _respawn_end=$(date -u +%s%3N 2>/dev/null || echo 0)
                _elapsed=$(( _respawn_end - _respawn_start ))
                log_handshake "worker_attach" 15 "fail:exhausted" "$_elapsed"
                # Fail-open: startup is not blocked.
            fi
        fi
    fi
fi

# Peer banner (other sessions only).
peers=$("$agorabus" peers 2>/dev/null)
case "$peers" in
    ""|"[]") exit 0 ;;
esac

other_count=$(printf '%s' "$peers" \
    | jq --arg sid "$sid" '[.[] | select(.session_id != $sid)] | length' 2>/dev/null)
if [ "${other_count:-0}" -gt 0 ]; then
    printf '\n=== agorabus: %s peer(s) on bus ===\n' "$other_count"
    printf '%s\n' "$peers" \
        | jq -r --arg sid "$sid" \
            '.[] | select(.session_id != $sid) | "- \(.session_id) pid=\(.pid) cwd=\(.cwd) intent=\(.intent)"' \
            2>/dev/null
    printf 'this session: %s\n' "$sid"
    printf '=== /agorabus ===\n'
fi

exit 0
