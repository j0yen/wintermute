#!/usr/bin/env bash
# Phase 4: budget enforcement. Creates a new agent namespace, sets a very
# low max_elapsed_ns budget with action=SIGTERM, busy-waits, and confirms
# the reaper signals the task.
#
# Requires the wintermute kernel (CONFIG_AGENT_NS=y) and CAP_SYS_ADMIN to
# unshare(CLONE_NEWAGENT). Exits 0 PASS, 1 FAIL, 77 SKIP.
set -u

if [[ ! -f /proc/self/agent_session ]]; then
    echo "/proc/self/agent_session missing — stock kernel, skipping"; exit 77
fi
if (( EUID != 0 )); then
    echo "needs root for unshare(CLONE_NEWAGENT) and dmesg; skipping"; exit 77
fi

HERE=$(dirname "$0")
BIN="$HERE/budget_victim"
SRC="$HERE/budget_victim.c"
if [[ ! -x "$BIN" ]]; then
    cat > "$SRC" <<'EOF'
#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <sched.h>
#include <sys/prctl.h>
#include <sys/syscall.h>
#include <linux/agent_namespaces.h>

int main(void) {
    if (unshare(CLONE_NEWAGENT) != 0) { perror("unshare"); return 1; }
    struct agent_ns_budget_uapi b = {0};
    b.max_elapsed_ns = 500ULL * 1000 * 1000;  /* 500 ms */
    b.action = 1; /* SIGTERM */
    if (prctl(PR_SET_AGENT_BUDGET_LIMITS, (unsigned long)&b, 0, 0, 0) != 0) {
        perror("prctl SET_BUDGET"); return 1;
    }
    fprintf(stderr, "victim pid=%d unshared+budgeted; sleeping past expiry...\n", getpid());
    sleep(120);  /* reaper runs every 60s — wait long enough */
    fprintf(stderr, "victim survived sleep — budget NOT enforced\n");
    return 99;
}
EOF
    cc -O2 -Wall -Wextra -o "$BIN" "$SRC" || { echo "compile failed"; exit 1; }
fi

dmesg -C >/dev/null 2>&1 || true
"$BIN" &
victim=$!
echo "test: launched victim pid=$victim; waiting up to 130s for reaper to act..."

# wait up to 130s for the victim to die
for i in $(seq 1 130); do
    if ! kill -0 "$victim" 2>/dev/null; then
        wait "$victim" 2>/dev/null
        rc=$?
        echo "test: victim exited after ~${i}s, rc=$rc"
        # confirm dmesg notes the budget action
        if dmesg | tail -50 | grep -qE 'agent_ns: budget.*exceeded.*SIGTERM'; then
            echo "test: PASS — dmesg recorded budget SIGTERM"
            exit 0
        else
            echo "test: FAIL — victim died but dmesg shows no agent_ns budget message"
            exit 1
        fi
    fi
    sleep 1
done

kill -9 "$victim" 2>/dev/null
echo "test: FAIL — victim did not die in 130s, reaper did not enforce budget"
exit 1
