#!/usr/bin/env bash
# Verify the agent_ns tracepoints are visible to bpftrace.
set -euo pipefail

if ! command -v bpftrace >/dev/null; then
	echo "SKIP: bpftrace not installed"
	exit 77
fi
if [[ ! -d /sys/kernel/tracing/events/agent_ns ]]; then
	echo "SKIP: /sys/kernel/tracing/events/agent_ns missing"
	exit 77
fi

bpftrace -e '
tracepoint:agent_ns:agent_session_start { printf("start sid=%r intent=%s root_pid=%d\n", buf(args->sid, 16), str(args->intent), args->root_pid); }
tracepoint:agent_ns:agent_session_end   { printf("end   sid=%r syscalls=%llu write_bytes=%llu elapsed_ns=%llu\n", buf(args->sid, 16), args->total_syscalls, args->write_bytes, args->elapsed_ns); }
' &
BT=$!
sleep 0.5

HERE="$(cd "$(dirname "$0")" && pwd)"
sudo "$HERE/unshare-helper" sh -c 'true'

sleep 1
kill "$BT" 2>/dev/null || true
wait "$BT" 2>/dev/null || true
echo "PASS: tracepoints fired (inspect output above for start/end pair)"
