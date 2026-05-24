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

set -u

agorabus=/home/jsy/.local/bin/agorabus
cache=/home/jsy/.cache/agorabus
sessions="$cache/sessions"
sock="$cache/sock"
daemon_log="$cache/daemon.log"

[ -x "$agorabus" ] || exit 0
mkdir -p "$sessions" 2>/dev/null || exit 0

# Bring up the daemon if missing.
if ! pgrep -f 'agorabus daemon' >/dev/null 2>&1; then
    nohup "$agorabus" daemon >"$daemon_log" 2>&1 &
    disown
    for _ in 1 2 3 4 5; do
        [ -S "$sock" ] && break
        sleep 0.1
    done
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
sid="claude-${root}-${project}"

# Avoid double-attaching if a subscriber for this sid already exists.
if ! pgrep -f "agorabus subscribe --session-id $sid" >/dev/null 2>&1; then
    log="$sessions/${sid}.ndjson"
    setsid bash -c "exec '$agorabus' subscribe --session-id '$sid' '' >>'$log' 2>&1" </dev/null &
    disown
    # Brief moment for the subscriber's announce to land.
    sleep 0.2
fi

# Spawn the RPC worker so this session auto-replies to ping / self.describe /
# methods.list / delegate.run on rpc.req.<sid>. Worker is idempotent.
worker="$HOME/.claude/scripts/agorabus-worker.sh"
if [ -x "$worker" ] \
   && ! pgrep -f "agorabus-worker.sh $sid\$" >/dev/null 2>&1; then
    workers="$cache/workers"
    mkdir -p "$workers" 2>/dev/null || true
    spawn_log="$workers/${sid}.spawn.log"
    setsid bash -c "exec '$worker' '$sid' '$cwd' >>'$spawn_log' 2>&1" </dev/null &
    disown
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
