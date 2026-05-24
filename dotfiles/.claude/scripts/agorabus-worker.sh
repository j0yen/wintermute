#!/usr/bin/env bash
# agorabus-worker.sh — RPC handler for one Claude session.
#
# Subscribes to rpc.req.<sid> and dispatches per AGORABUS_RPC.md v0.1:
#   ping          → {pong_unix, session_id}        (default method)
#   self.describe → {session_id, cwd, claude_version, started_unix, methods}
#   methods.list  → {methods: [...]}                (default method)
#   delegate.run  → spawn `claude --print` in caller-specified cwd
# Unknown methods reply {ok:false, error:"unknown_method"}.
#
# Started by SessionStart hook (one per claude session); killed at SessionEnd.
# Idempotent: refuses to start if a worker for this sid is already running.

set -u

agorabus=/home/jsy/.local/bin/agorabus
sid="${1:-}"
worker_cwd="${2:-$HOME}"
if [ -z "$sid" ]; then
    echo "usage: agorabus-worker.sh <sid> [cwd]" >&2
    exit 2
fi

started_unix=$(date +%s)
claude_version=$(claude --version 2>/dev/null | head -1 || true)
METHODS='["ping","self.describe","methods.list","delegate.run"]'

[ -x "$agorabus" ] || exit 0
command -v jq >/dev/null 2>&1 || exit 0

# Recursion guard: a delegated claude must not start its own worker.
if [ "${AGORABUS_DELEGATE_DEPTH:-0}" -gt 0 ]; then
    exit 0
fi

# Idempotency: bail if another worker for this sid is already alive.
self_pid=$$
if pgrep -f "agorabus-worker.sh $sid\$" | grep -v "^${self_pid}\$" >/dev/null 2>&1; then
    exit 0
fi

log_dir=/home/jsy/.cache/agorabus/workers
mkdir -p "$log_dir" 2>/dev/null || true
log="$log_dir/${sid}.log"

worker_conn_sid="${sid}-worker"

log_event() {
    printf '[%s] %s\n' "$(date -Iseconds)" "$*" >>"$log"
}

reply() {
    local from="$1" id="$2" payload="$3"
    "$agorabus" publish --session-id "$sid" "rpc.reply.${from}" "$payload" \
        >/dev/null 2>&1 || log_event "publish-failed id=$id from=$from"
}

log_event "worker-start sid=$sid"

"$agorabus" subscribe "rpc.req.${sid}" --session-id "$worker_conn_sid" 2>/dev/null \
| while IFS= read -r line; do
    [ -z "$line" ] && continue

    method=$(printf '%s' "$line" | jq -r '.data.method // empty' 2>/dev/null || true)
    id=$(printf '%s' "$line" | jq -r '.data.id // empty' 2>/dev/null || true)
    from=$(printf '%s' "$line" | jq -r '.data.from // empty' 2>/dev/null || true)

    if [ -z "$id" ] || [ -z "$from" ]; then
        log_event "skip-malformed: $line"
        continue
    fi

    # Default convention methods (read-only / pure): handle inline.
    case "$method" in
    ping)
        log_event "recv id=$id from=$from method=ping"
        body=$(jq -nc --arg id "$id" --arg s "$sid" --arg f "$from" \
                       --argjson now "$(date +%s)" \
            '{id:$id, from:$s, to:$f, ok:true,
              result:{pong_unix:$now, session_id:$s}}')
        reply "$from" "$id" "$body"
        continue
        ;;
    self.describe)
        log_event "recv id=$id from=$from method=self.describe"
        body=$(jq -nc --arg id "$id" --arg s "$sid" --arg f "$from" \
                       --arg cwd "$worker_cwd" --arg ver "$claude_version" \
                       --argjson started "$started_unix" \
                       --argjson methods "$METHODS" \
            '{id:$id, from:$s, to:$f, ok:true,
              result:{session_id:$s, cwd:$cwd, claude_version:$ver,
                      started_unix:$started, methods:$methods}}')
        reply "$from" "$id" "$body"
        continue
        ;;
    methods.list)
        log_event "recv id=$id from=$from method=methods.list"
        body=$(jq -nc --arg id "$id" --arg s "$sid" --arg f "$from" \
                       --argjson methods "$METHODS" \
            '{id:$id, from:$s, to:$f, ok:true, result:{methods:$methods}}')
        reply "$from" "$id" "$body"
        continue
        ;;
    esac

    if [ "$method" != "delegate.run" ]; then
        log_event "unknown-method id=$id from=$from method=$method"
        body=$(jq -nc --arg id "$id" --arg s "$sid" --arg f "$from" --arg m "$method" \
            '{id:$id, from:$s, to:$f, ok:false, error:"unknown_method", detail:("got: "+$m)}')
        reply "$from" "$id" "$body"
        continue
    fi

    prompt=$(printf '%s' "$line" | jq -r '.data.params.prompt // empty')
    cwd_req=$(printf '%s' "$line" | jq -r '.data.params.cwd // empty')
    timeout_secs=$(printf '%s' "$line" | jq -r '.data.params.timeout_secs // 300')
    cwd="${cwd_req:-$HOME}"

    if [ -z "$prompt" ]; then
        body=$(jq -nc --arg id "$id" --arg s "$sid" --arg f "$from" \
            '{id:$id, from:$s, to:$f, ok:false, error:"missing_prompt"}')
        reply "$from" "$id" "$body"
        continue
    fi
    if [ ! -d "$cwd" ]; then
        body=$(jq -nc --arg id "$id" --arg s "$sid" --arg f "$from" --arg c "$cwd" \
            '{id:$id, from:$s, to:$f, ok:false, error:"bad_cwd", detail:$c}')
        reply "$from" "$id" "$body"
        continue
    fi

    log_event "run-begin id=$id from=$from cwd=$cwd timeout=${timeout_secs}s"
    start_ms=$(date +%s%3N)
    out=$(cd "$cwd" && \
        AGORABUS_DELEGATE_DEPTH=$((${AGORABUS_DELEGATE_DEPTH:-0} + 1)) \
        timeout "${timeout_secs}s" claude --print \
            --dangerously-skip-permissions --no-session-persistence \
            --output-format text -- "$prompt" 2>&1)
    rc=$?
    end_ms=$(date +%s%3N)
    dur_ms=$((end_ms - start_ms))
    log_event "run-end   id=$id rc=$rc dur=${dur_ms}ms bytes=${#out}"

    if [ $rc -eq 0 ]; then
        body=$(jq -nc --arg id "$id" --arg s "$sid" --arg f "$from" \
                      --arg out "$out" --argjson dur "$dur_ms" --arg cwd "$cwd" \
            '{id:$id, from:$s, to:$f, ok:true,
              result:{stdout:$out, exit_code:0, duration_ms:$dur, cwd:$cwd}}')
    elif [ $rc -eq 124 ]; then
        body=$(jq -nc --arg id "$id" --arg s "$sid" --arg f "$from" \
                      --arg out "$out" --argjson dur "$dur_ms" \
            '{id:$id, from:$s, to:$f, ok:false, error:"timeout",
              detail:$out, duration_ms:$dur}')
    else
        body=$(jq -nc --arg id "$id" --arg s "$sid" --arg f "$from" \
                      --arg out "$out" --argjson dur "$dur_ms" --argjson code "$rc" \
            '{id:$id, from:$s, to:$f, ok:false, error:"nonzero_exit",
              detail:$out, exit_code:$code, duration_ms:$dur}')
    fi
    reply "$from" "$id" "$body"
done

log_event "worker-exit sid=$sid"
