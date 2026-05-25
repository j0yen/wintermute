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
 * CLONE_NEWAGENT lives in <uapi/linux/sched.h> — see that header for the
 * authoritative bit. It is *not* re-defined here to avoid drift; we claim
 * 0x00000100 (an unused lower-pool bit) rather than the legacy 0x40000000
 * CLONE_NEWNET alias that early drafts of this PRD proposed.
 */

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
