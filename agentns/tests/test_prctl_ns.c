/*
 * test_prctl_ns.c — agent namespace creation via prctl(PR_SET_AGENT_NS).
 *
 * Replaces the EINVAL-bound unshare(CLONE_NEWAGENT) contract from
 * test_unshare.c. The clone-flag space is exhausted, so creation now goes
 * through prctl(PR_SET_AGENT_NS) (see PRD-agentns-clone-flag-fix).
 *
 * Requires the patched kernel running. Compile with:
 *   gcc -O2 -Wall -o test_prctl_ns test_prctl_ns.c
 *
 * Exit codes:
 *   0  pass
 *   1  fail (with diagnostic to stderr)
 *   77 skip (kernel does not support PR_SET_AGENT_NS — treated as skip)
 */
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/prctl.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <unistd.h>

#include "../include/uapi/linux/agent_namespaces.h"

#define DIE(fmt, ...) do { fprintf(stderr, fmt "\n", ##__VA_ARGS__); exit(1); } while (0)
#define SKIP(fmt, ...) do { fprintf(stderr, "SKIP: " fmt "\n", ##__VA_ARGS__); exit(77); } while (0)

static int read_proc_id(pid_t pid, char *out, size_t out_len)
{
	char path[128];
	snprintf(path, sizeof(path), "/proc/%d/agent_session", pid);
	int fd = open(path, O_RDONLY);
	if (fd < 0)
		return -1;
	ssize_t n = read(fd, out, out_len - 1);
	close(fd);
	if (n <= 0)
		return -1;
	out[n] = '\0';
	if (out[n - 1] == '\n')
		out[n - 1] = '\0';
	return 0;
}

static int id_is_zero(const char *s)
{
	for (; *s; s++)
		if (*s != '0')
			return 0;
	return 1;
}

int main(void)
{
	char parent_id[64], child_id[64];

	if (access("/proc/self/agent_session", R_OK) != 0)
		SKIP("/proc/self/agent_session not present — kernel not patched?");

	if (read_proc_id(getpid(), parent_id, sizeof(parent_id)) < 0)
		DIE("could not read parent agent_session");
	printf("parent (init NS): session=%s\n", parent_id);

	/* Create-and-enter a fresh agent NS via prctl — NOT a clone flag. */
	if (prctl(PR_SET_AGENT_NS, 0, 0, 0, 0) < 0) {
		if (errno == ENOSYS || errno == EINVAL)
			SKIP("prctl(PR_SET_AGENT_NS) unsupported (errno=%d) — kernel not rebuilt?", errno);
		if (errno == EPERM)
			DIE("prctl(PR_SET_AGENT_NS) EPERM — run as root or grant CAP_SYS_ADMIN");
		DIE("prctl(PR_SET_AGENT_NS) failed: %s", strerror(errno));
	}

	if (read_proc_id(getpid(), child_id, sizeof(child_id)) < 0)
		DIE("could not read child agent_session after PR_SET_AGENT_NS");
	printf("child  (new NS):  session=%s\n", child_id);

	/* The core contract this PRD restores: a non-zero 128-bit session id. */
	if (id_is_zero(child_id))
		DIE("session_id is still zero after PR_SET_AGENT_NS (creation failed)");
	if (strcmp(parent_id, child_id) == 0)
		DIE("session_id did not change after PR_SET_AGENT_NS");

	/* prctl round-trip: PR_GET must agree with /proc. */
	struct agent_session_id_uapi sid;
	if (prctl(PR_GET_AGENT_SESSION_ID, &sid, 0, 0, 0) < 0)
		DIE("PR_GET_AGENT_SESSION_ID: %s", strerror(errno));

	char hex[64] = {0};
	for (int i = 0; i < AGENT_NS_ID_BYTES; i++)
		snprintf(hex + i * 2, sizeof(hex) - i * 2, "%02x", sid.bytes[i]);
	if (strcmp(hex, child_id) != 0)
		DIE("prctl id (%s) != /proc id (%s)", hex, child_id);

	/* intent tag round-trip on the new NS */
	if (prctl(PR_SET_AGENT_INTENT_TAG, "test-prctl", 0, 0, 0) < 0)
		DIE("PR_SET_AGENT_INTENT_TAG: %s", strerror(errno));
	char tag[AGENT_NS_INTENT_MAX + 1] = {0};
	if (prctl(PR_GET_AGENT_INTENT_TAG, tag, 0, 0, 0) < 0)
		DIE("PR_GET_AGENT_INTENT_TAG: %s", strerror(errno));
	if (strcmp(tag, "test-prctl") != 0)
		DIE("intent tag round-trip mismatch (got %s)", tag);

	/* a second PR_SET_AGENT_NS must mint a *different* session id */
	char second_id[64];
	if (prctl(PR_SET_AGENT_NS, 0, 0, 0, 0) < 0)
		DIE("second PR_SET_AGENT_NS: %s", strerror(errno));
	if (read_proc_id(getpid(), second_id, sizeof(second_id)) < 0)
		DIE("could not read agent_session after 2nd PR_SET_AGENT_NS");
	if (strcmp(second_id, child_id) == 0)
		DIE("2nd PR_SET_AGENT_NS reused the same session id (%s)", second_id);

	printf("PASS: prctl(PR_SET_AGENT_NS) create-and-enter, /proc, prctl all OK\n");
	return 0;
}
