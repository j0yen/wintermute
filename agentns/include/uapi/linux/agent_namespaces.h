/* SPDX-License-Identifier: GPL-2.0 WITH Linux-syscall-note */
/*
 * UAPI for the wintermute Agent Namespace.
 *
 * Userspace consumers (recall, ctrace, fsstory, /proc readers) include this
 * header to talk to the kernel about agent sessions.
 */
#ifndef _UAPI_LINUX_AGENT_NAMESPACES_H
#define _UAPI_LINUX_AGENT_NAMESPACES_H

#include <linux/types.h>

/*
 * Agent-namespace CREATION is done via prctl(PR_SET_AGENT_NS), NOT via a
 * legacy clone/unshare flag. The 32-bit clone-flag space is fully exhausted
 * (CSIGNAL..CLONE_IO are all claimed; CLONE_NEWTIME 0x80 took the last low
 * bit), so there is no free bit reachable by unshare(2). An early draft
 * #define'd CLONE_NEWAGENT as 0x00000100 — which IS CLONE_VM — so every
 * unshare(CLONE_NEWAGENT) returned EINVAL and every kthread fork was
 * misread as requesting a new agent NS. See assay-agentns for the diagnosis.
 *
 * A CLONE_NEWAGENT bit still exists in <uapi/linux/sched.h> as a 64-bit
 * clone3()-only flag (0x400000000ULL) for the clone3 inheritance path, but
 * it is unreachable from unshare(2) and is NOT the creation mechanism. Do
 * not #define CLONE_NEWAGENT == 0x100 anywhere on the creation path.
 */

/* prctl options exposed under PR_AGENT_* — chosen from the high-end pool */
#define PR_AGENT_BASE			0x41544E53  /* "ATNS" */
#define PR_GET_AGENT_SESSION_ID		(PR_AGENT_BASE + 1)
#define PR_SET_AGENT_INTENT_TAG		(PR_AGENT_BASE + 2)
#define PR_GET_AGENT_INTENT_TAG		(PR_AGENT_BASE + 3)
#define PR_SET_AGENT_BUDGET_LIMITS	(PR_AGENT_BASE + 4)
#define PR_GET_AGENT_COUNTERS		(PR_AGENT_BASE + 5)
#define PR_GET_AGENT_PARENT_ID		(PR_AGENT_BASE + 6)
/*
 * PR_SET_AGENT_NS: create a fresh agent namespace (fresh 128-bit session id,
 * zeroed counters, cleared intent tag) and install it on the calling task's
 * nsproxy. arg2..arg5 are ignored (pass 0). Returns 0 on success, -EPERM if
 * the caller lacks CAP_SYS_ADMIN in its user_ns and is in the init agent NS,
 * -ENOMEM on allocation failure. This is the unshare-equivalent creation
 * path, callable from plain userspace without any clone flag.
 */
#define PR_SET_AGENT_NS			(PR_AGENT_BASE + 7)

#define AGENT_NS_ID_BYTES		16
#define AGENT_NS_INTENT_MAX		63

struct agent_session_id_uapi {
	__u8 bytes[AGENT_NS_ID_BYTES];
};

struct agent_ns_counters_uapi {
	__u64 total_syscalls;
	__u64 openat_count;
	__u64 write_bytes;
	__u64 connect_count;
	__u64 unlink_count;
	__u64 fork_count;
	__u64 elapsed_ns;
};

struct agent_ns_budget_uapi {
	__u64 max_syscalls;
	__u64 max_write_bytes;
	__u64 max_elapsed_ns;
	__u32 action;		/* 0=log, 1=SIGTERM, 2=SIGKILL */
	__u32 _reserved;
};

#endif /* _UAPI_LINUX_AGENT_NAMESPACES_H */
