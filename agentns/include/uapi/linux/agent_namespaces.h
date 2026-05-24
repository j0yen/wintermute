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
 * CLONE_NEWAGENT is allocated from the high bit range, intentionally above
 * the currently-used CLONE_* set so it round-trips through clone3().
 *
 * Bit 0x40000000 was historically CLONE_NEWUSER pre-3.8 and is no longer
 * in use anywhere in mainline. We claim it for vendor-fork purposes.
 */
#define CLONE_NEWAGENT			0x40000000

/* prctl options exposed under PR_AGENT_* — chosen from the high-end pool */
#define PR_AGENT_BASE			0x41544E53  /* "ATNS" */
#define PR_GET_AGENT_SESSION_ID		(PR_AGENT_BASE + 1)
#define PR_SET_AGENT_INTENT_TAG		(PR_AGENT_BASE + 2)
#define PR_GET_AGENT_INTENT_TAG		(PR_AGENT_BASE + 3)
#define PR_SET_AGENT_BUDGET_LIMITS	(PR_AGENT_BASE + 4)
#define PR_GET_AGENT_COUNTERS		(PR_AGENT_BASE + 5)
#define PR_GET_AGENT_PARENT_ID		(PR_AGENT_BASE + 6)

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
