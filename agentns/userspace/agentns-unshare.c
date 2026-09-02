/* SPDX-License-Identifier: GPL-2.0 WITH Linux-syscall-note */
/*
 * agentns-unshare — create a fresh agent namespace via
 * prctl(PR_SET_AGENT_NS), then exec the given command inside it.
 *
 * This is the working replacement for agent-wrap's EINVAL-bound
 * unshare(CLONE_NEWAGENT) path. The 32-bit clone-flag space is exhausted,
 * so the agent namespace is created through prctl, NOT a clone/unshare flag.
 *
 * Needs CAP_SYS_ADMIN in the caller's user_ns when starting from the init
 * agent NS. The intended deployment is file caps on the installed binary:
 *     sudo setcap cap_sys_admin+ep /home/jsy/.local/bin/agentns-unshare
 *
 * On a kernel without PR_SET_AGENT_NS (returns EINVAL because the prctl
 * range is unhandled, or ENOSYS) we warn on stderr and exec the target
 * anyway, so the wrapper is harmless to leave in place across kernel
 * switches.
 *
 * If $AGENT_INTENT is set and non-empty, it is applied via
 * prctl(PR_SET_AGENT_INTENT_TAG, ...) after entering the new namespace.
 *
 * Exit codes:
 *   2    usage error
 *   1    fatal (EPERM, or exec failure surfaced below)
 *   127  execvp failed
 */
#define _GNU_SOURCE
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/prctl.h>
#include <unistd.h>

#include "../include/uapi/linux/agent_namespaces.h"

int main(int argc, char **argv)
{
	if (argc < 2) {
		fprintf(stderr,
			"usage: %s <cmd> [args...]\n"
			"  set AGENT_INTENT=<tag> to label the new session.\n",
			argv[0]);
		return 2;
	}

	if (prctl(PR_SET_AGENT_NS, 0, 0, 0, 0) < 0) {
		int e = errno;
		if (e == ENOSYS || e == EINVAL) {
			fprintf(stderr,
				"agentns-unshare: kernel lacks "
				"PR_SET_AGENT_NS; execing %s in current "
				"namespace\n",
				argv[1]);
		} else if (e == EPERM) {
			fprintf(stderr,
				"agentns-unshare: prctl(PR_SET_AGENT_NS) "
				"EPERM — needs CAP_SYS_ADMIN (try `sudo "
				"setcap cap_sys_admin+ep %s`)\n",
				argv[0]);
			return 1;
		} else {
			fprintf(stderr,
				"agentns-unshare: prctl(PR_SET_AGENT_NS): "
				"%s\n",
				strerror(e));
			return 1;
		}
	} else {
		const char *intent = getenv("AGENT_INTENT");
		if (intent && *intent) {
			if (prctl(PR_SET_AGENT_INTENT_TAG,
				  (unsigned long)intent, 0, 0, 0) < 0) {
				fprintf(stderr,
					"agentns-unshare: prctl(intent_tag): "
					"%s (continuing)\n",
					strerror(errno));
			}
		}
	}

	execvp(argv[1], argv + 1);
	fprintf(stderr, "agentns-unshare: execvp %s: %s\n",
		argv[1], strerror(errno));
	return 127;
}
