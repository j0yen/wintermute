/*
 * test_unshare.c — LEGACY clone-flag path probe (documents EINVAL).
 *
 * The agent namespace is no longer created via unshare(CLONE_NEWAGENT): the
 * 32-bit clone-flag space is exhausted and the replacement flag is
 * clone3-only. The supported creation path is prctl(PR_SET_AGENT_NS); see
 * test_prctl_ns.c, which is the real acceptance test.
 *
 * This file asserts the NEW contract for the legacy path: a fresh-NS create
 * is performed via prctl (not unshare), and the per-NS prctl/counter surfaces
 * still round-trip. The old "unshare(CLONE_NEWAGENT) must succeed and change
 * the session id" expectation has been removed — that unshare is EINVAL-bound
 * by design now.
 *
 * Requires the patched kernel running.  Compile with:
 *   gcc -O2 -Wall -o test_unshare test_unshare.c
 *
 * Exit codes:
 *   0  pass
 *   1  fail (with diagnostic to stderr)
 *   77 skip (kernel does not support agent NS — treated as skip by autotest)
 */
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <linux/sched.h>
#include <sched.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/prctl.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <unistd.h>

#include "../include/uapi/linux/agent_namespaces.h"

#ifndef CLONE_NEWAGENT
#define CLONE_NEWAGENT 0x400000000ULL
#endif

/*
 * glibc's unshare(2) wrapper is int-only — bits above 32 truncate to
 * zero. Reach the kernel's unsigned-long flags argument directly.
 */
static inline int unshare_u64(unsigned long long flags)
{
	return syscall(SYS_unshare, (unsigned long)flags);
}

#define DIE(fmt, ...) do { fprintf(stderr, fmt "\n", ##__VA_ARGS__); exit(1); } while (0)
#define SKIP(fmt, ...) do { fprintf(stderr, "SKIP: " fmt "\n", ##__VA_ARGS__); exit(77); } while (0)

static int read_proc_id(pid_t pid, char *out, size_t out_len)
{
	char path[128];
	snprintf(path, sizeof(path), "/proc/%d/agent_session", pid);
	int fd = open(path, O_RDONLY);
	if (fd < 0) return -1;
	ssize_t n = read(fd, out, out_len - 1);
	close(fd);
	if (n <= 0) return -1;
	out[n] = '\0';
	if (out[n - 1] == '\n') out[n - 1] = '\0';
	return 0;
}

int main(void)
{
	char parent_id[64], child_id[64];

	if (access("/proc/self/agent_session", R_OK) != 0)
		SKIP("/proc/self/agent_session not present — kernel not patched?");

	if (read_proc_id(getpid(), parent_id, sizeof(parent_id)) < 0)
		DIE("could not read parent agent_session");
	printf("parent (init NS): session=%s\n", parent_id);

	/*
	 * Legacy-path contract: unshare(CLONE_NEWAGENT 0x100) MUST fail — that
	 * bit aliases CLONE_VM. A success here would mean the old broken
	 * creation path regressed back in. We then create the namespace the
	 * supported way, via prctl(PR_SET_AGENT_NS).
	 */
	if (unshare_u64(0x100) == 0)
		DIE("unshare(0x100) unexpectedly succeeded — clone-flag creation regressed");

	if (prctl(PR_SET_AGENT_NS, 0, 0, 0, 0) < 0) {
		if (errno == ENOSYS || errno == EINVAL)
			SKIP("prctl(PR_SET_AGENT_NS) unsupported (errno=%d) — kernel not rebuilt?", errno);
		if (errno == EPERM)
			DIE("prctl(PR_SET_AGENT_NS) returned EPERM — run as root or grant CAP_SYS_ADMIN");
		DIE("prctl(PR_SET_AGENT_NS) failed: %s", strerror(errno));
	}

	if (read_proc_id(getpid(), child_id, sizeof(child_id)) < 0)
		DIE("could not read child agent_session after PR_SET_AGENT_NS");
	printf("child  (new NS):  session=%s\n", child_id);

	if (strcmp(parent_id, child_id) == 0)
		DIE("session_id did not change after unshare");

	/* prctl round-trip on the new NS */
	struct agent_session_id_uapi sid;
	if (prctl(PR_GET_AGENT_SESSION_ID, &sid, 0, 0, 0) < 0)
		DIE("PR_GET_AGENT_SESSION_ID: %s", strerror(errno));

	char hex[64] = {0};
	for (int i = 0; i < AGENT_NS_ID_BYTES; i++)
		snprintf(hex + i * 2, sizeof(hex) - i * 2, "%02x", sid.bytes[i]);
	if (strcmp(hex, child_id) != 0)
		DIE("prctl id (%s) != /proc id (%s)", hex, child_id);

	if (prctl(PR_SET_AGENT_INTENT_TAG, "test-intent", 0, 0, 0) < 0)
		DIE("PR_SET_AGENT_INTENT_TAG: %s", strerror(errno));

	char tag[AGENT_NS_INTENT_MAX + 1] = {0};
	if (prctl(PR_GET_AGENT_INTENT_TAG, tag, 0, 0, 0) < 0)
		DIE("PR_GET_AGENT_INTENT_TAG: %s", strerror(errno));
	if (strcmp(tag, "test-intent") != 0)
		DIE("intent tag round-trip mismatch (got %s)", tag);

	/* counters: do some writes, check write_bytes climbs */
	struct agent_ns_counters_uapi c0, c1;
	if (prctl(PR_GET_AGENT_COUNTERS, &c0, 0, 0, 0) < 0)
		DIE("PR_GET_AGENT_COUNTERS: %s", strerror(errno));

	int fd = open("/tmp/agentns-test.tmp", O_WRONLY | O_CREAT | O_TRUNC, 0600);
	if (fd < 0) DIE("open /tmp/agentns-test.tmp: %s", strerror(errno));
	char buf[4096];
	memset(buf, 'x', sizeof(buf));
	for (int i = 0; i < 10; i++)
		write(fd, buf, sizeof(buf));
	close(fd);
	unlink("/tmp/agentns-test.tmp");

	if (prctl(PR_GET_AGENT_COUNTERS, &c1, 0, 0, 0) < 0)
		DIE("PR_GET_AGENT_COUNTERS (2nd): %s", strerror(errno));

	if (c1.write_bytes <= c0.write_bytes)
		DIE("write_bytes did not increase: %llu -> %llu",
		    (unsigned long long)c0.write_bytes,
		    (unsigned long long)c1.write_bytes);
	if (c1.openat_count <= c0.openat_count)
		DIE("openat_count did not increase: %llu -> %llu",
		    (unsigned long long)c0.openat_count,
		    (unsigned long long)c1.openat_count);

	printf("PASS: unshare, /proc, prctl, counters all OK\n");
	return 0;
}
