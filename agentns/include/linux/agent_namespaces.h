/* SPDX-License-Identifier: GPL-2.0 */
/*
 * Agent Namespaces — per-session identity, intent, and counters.
 *
 * Out-of-tree vendor patch for the wintermute kernel.
 * See PRD-agent-namespace.md for design rationale.
 */
#ifndef _LINUX_AGENT_NAMESPACES_H
#define _LINUX_AGENT_NAMESPACES_H

#include <linux/atomic.h>
#include <linux/kref.h>
#include <linux/ns_common.h>
#include <linux/nsproxy.h>
#include <linux/percpu.h>
#include <linux/spinlock.h>
#include <linux/uidgid.h>
#include <linux/types.h>
#include <uapi/linux/sched.h>	/* CLONE_NEWAGENT (clone3-only 0x400000000ULL) */

#define AGENT_NS_ID_BYTES		16   /* 128-bit opaque session id */
#define AGENT_NS_INTENT_MAX		63   /* nul-terminated, fits in 64-byte cacheline */
#define AGENT_NS_DEFAULT_LIFETIME_S	86400 /* 24h reaper default */

/*
 * ns_type tag for __ns_common_init(). This is the value stamped into
 * ns_common.ns_type; it is NOT reachable as an unshare(2) clone flag. We use
 * the 64-bit clone3-only CLONE_NEWAGENT (0x400000000ULL) as the identity tag,
 * named explicitly so the ns_common identity never depends on a low clone
 * bit that could alias CLONE_VM.
 */
#define AGENT_NS_TYPE			CLONE_NEWAGENT

struct agent_session_id {
	u8 bytes[AGENT_NS_ID_BYTES];
};

struct agent_ns_counters_pcp {
	u64 total_syscalls;
	u64 openat_count;
	u64 write_bytes;
	u64 connect_count;
	u64 unlink_count;
	u64 fork_count;
} ____cacheline_aligned;

struct agent_ns_counters_snapshot {
	u64 total_syscalls;
	u64 openat_count;
	u64 write_bytes;
	u64 connect_count;
	u64 unlink_count;
	u64 fork_count;
	u64 elapsed_ns;
};

struct agent_ns_budget {
	u64 max_syscalls;	/* 0 = unlimited */
	u64 max_write_bytes;
	u64 max_elapsed_ns;
	u32 action;		/* 0=log, 1=SIGTERM, 2=SIGKILL */
};

struct agent_namespace {
	struct ns_common	ns;
	struct kref		kref;
	struct user_namespace	*user_ns;
	struct agent_session_id	session_id;
	struct agent_session_id parent_session_id;
	char			intent_tag[AGENT_NS_INTENT_MAX + 1];
	spinlock_t		intent_lock;
	unsigned long		intent_last_set_jiffies;
	u64			created_ns;	/* ktime_get_boot_ns at creation */
	struct agent_ns_counters_pcp __percpu *counters;
	struct agent_ns_budget	budget;
	atomic_t		live_tasks;
};

extern struct agent_namespace init_agent_ns;
extern const struct proc_ns_operations agentns_operations;

#ifdef CONFIG_AGENT_NS

struct agent_namespace *copy_agent_ns(unsigned long flags,
				      struct user_namespace *user_ns,
				      struct agent_namespace *old);
void free_agent_ns(struct kref *kref);

static inline struct agent_namespace *get_agent_ns(struct agent_namespace *ns)
{
	if (ns)
		kref_get(&ns->kref);
	return ns;
}

static inline void put_agent_ns(struct agent_namespace *ns)
{
	if (ns)
		kref_put(&ns->kref, free_agent_ns);
}

/* fast-path inline helpers — called from syscall entry tracepoints */
void agent_ns_count_syscall(void);
void agent_ns_count_openat(void);
void agent_ns_count_write(size_t bytes);
void agent_ns_count_connect(void);
void agent_ns_count_unlink(void);
void agent_ns_count_fork(void);

void agent_ns_snapshot(struct agent_namespace *ns,
		       struct agent_ns_counters_snapshot *out);

/* /proc helpers */
ssize_t agent_ns_proc_id_show(struct seq_file *m, void *v);
ssize_t agent_ns_proc_intent_show(struct seq_file *m, void *v);
ssize_t agent_ns_proc_counters_show(struct seq_file *m, void *v);

/* sysctl knobs */
extern int sysctl_agent_ns_enabled;
extern unsigned int sysctl_agent_ns_max_lifetime;

/* task hook called from do_exit */
void agent_ns_task_exit(struct task_struct *tsk);

/* prctl PR_AGENT_* dispatcher */
long agent_ns_prctl(struct task_struct *me, int option, unsigned long arg2,
		    unsigned long arg3, unsigned long arg4, unsigned long arg5);

/*
 * Create a fresh agent_ns and install it on @current's nsproxy (the
 * unshare-equivalent create-and-enter path used by PR_SET_AGENT_NS).
 * Implemented in kernel/nsproxy.c because it needs the static nsproxy
 * allocator / create_new_namespaces(). Returns 0 or a negative errno.
 */
int agent_ns_reproxy_current(void);

/* reaper for the max-lifetime safety net */
void agent_ns_reaper_kick(void);

/* helper for hex/uuid-ish formatting */
int agent_session_id_format(const struct agent_session_id *id, char *buf, size_t len);

#else /* !CONFIG_AGENT_NS */

static inline struct agent_namespace *copy_agent_ns(unsigned long flags,
						    struct user_namespace *user_ns,
						    struct agent_namespace *old)
{
	if (flags & CLONE_NEWAGENT)
		return ERR_PTR(-EINVAL);
	return old;
}
static inline struct agent_namespace *get_agent_ns(struct agent_namespace *ns) { return ns; }
static inline void put_agent_ns(struct agent_namespace *ns) {}
static inline void agent_ns_count_syscall(void) {}
static inline void agent_ns_count_openat(void) {}
static inline void agent_ns_count_write(size_t bytes) {}
static inline void agent_ns_count_connect(void) {}
static inline void agent_ns_count_unlink(void) {}
static inline void agent_ns_count_fork(void) {}
static inline void agent_ns_task_exit(struct task_struct *tsk) {}

#endif /* CONFIG_AGENT_NS */

#endif /* _LINUX_AGENT_NAMESPACES_H */
