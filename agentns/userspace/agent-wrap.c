/* SPDX-License-Identifier: GPL-2.0 WITH Linux-syscall-note */
/*
 * agent-wrap — launch a command inside a fresh CLONE_NEWAGENT.
 *
 * Needs CAP_SYS_ADMIN. The intended deployment is file caps on the
 * installed binary:
 *     sudo setcap cap_sys_admin+ep /home/jsy/.local/bin/agent-wrap
 *
 * On a kernel without CLONE_NEWAGENT, the unshare returns ENOSYS;
 * we warn on stderr and exec the target anyway so the wrapper is
 * harmless to leave in place across kernel switches.
 *
 * If $AGENT_INTENT is set and non-empty, it is passed through
 * prctl(PR_SET_AGENT_INTENT_TAG, ...) before exec.
 */
#define _GNU_SOURCE
#include <errno.h>
#include <sched.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/prctl.h>
#include <unistd.h>

#include "../include/uapi/linux/agent_namespaces.h"

#ifndef CLONE_NEWAGENT
#define CLONE_NEWAGENT 0x00000100
#endif

int main(int argc, char **argv)
{
	if (argc < 2) {
		fprintf(stderr,
			"usage: %s <cmd> [args...]\n"
			"  set AGENT_INTENT=<tag> to label the new session.\n",
			argv[0]);
		return 2;
	}

	if (unshare(CLONE_NEWAGENT) < 0) {
		int e = errno;
		if (e == ENOSYS) {
			fprintf(stderr,
				"agent-wrap: kernel lacks CLONE_NEWAGENT; "
				"execing %s in current namespace\n",
				argv[1]);
		} else if (e == EPERM) {
			fprintf(stderr,
				"agent-wrap: unshare EPERM — needs "
				"CAP_SYS_ADMIN (try `sudo setcap "
				"cap_sys_admin+ep %s`)\n",
				argv[0]);
			return 1;
		} else {
			fprintf(stderr,
				"agent-wrap: unshare(CLONE_NEWAGENT): %s\n",
				strerror(e));
			return 1;
		}
	} else {
		const char *intent = getenv("AGENT_INTENT");
		if (intent && *intent) {
			if (prctl(PR_SET_AGENT_INTENT_TAG,
				  (unsigned long)intent, 0, 0, 0) < 0) {
				fprintf(stderr,
					"agent-wrap: prctl(intent_tag): "
					"%s (continuing)\n",
					strerror(errno));
			}
		}
	}

	execvp(argv[1], argv + 1);
	fprintf(stderr, "agent-wrap: execvp %s: %s\n",
		argv[1], strerror(errno));
	return 127;
}
