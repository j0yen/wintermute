#!/usr/bin/env bash
# Verify child processes inherit the parent's agent_session_id.
set -euo pipefail

if [[ ! -r /proc/self/agent_session ]]; then
	echo "SKIP: /proc/self/agent_session missing (kernel not patched)"
	exit 77
fi

HERE="$(cd "$(dirname "$0")" && pwd)"
[[ -x "$HERE/unshare-helper" ]] || cc -O2 -o "$HERE/unshare-helper" - <<'C'
#define _GNU_SOURCE
#include <sched.h>
#include <stdio.h>
#include <sys/syscall.h>
#include <unistd.h>
/* glibc's unshare(2) wrapper is int-only; CLONE_NEWAGENT lives above bit 32. */
int main(int argc, char **argv) {
	if (syscall(SYS_unshare, 0x400000000UL /* CLONE_NEWAGENT */) < 0) { perror("unshare"); return 1; }
	execvp(argv[1], argv + 1);
	perror("exec"); return 1;
}
C

# Sanity: parent (init NS) reads zero id.
init_id="$(cat /proc/self/agent_session)"
echo "init NS:  $init_id"

# Spawn a shell inside a fresh agent NS; descendants should share an id.
out=$("$HERE/unshare-helper" bash -c '
	id1=$(cat /proc/self/agent_session)
	(id2=$(cat /proc/self/agent_session); echo "child:$id2") &
	wait
	echo "parent:$id1"
')
echo "$out"

p=$(printf '%s\n' "$out" | awk -F: '/parent:/{print $2}')
c=$(printf '%s\n' "$out" | awk -F: '/child:/{print $2}')
if [[ "$p" != "$c" ]]; then
	echo "FAIL: parent ($p) != child ($c)"
	exit 1
fi
if [[ "$p" == "$init_id" ]]; then
	echo "FAIL: new NS id matches init NS id"
	exit 1
fi
echo "PASS: child inherits parent agent_session_id"
