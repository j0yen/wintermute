// SPDX-License-Identifier: GPL-2.0
/*
 * Agent Namespaces — per-session identity and observability.
 *
 * Vendor patch for the wintermute kernel. See PRD-agent-namespace.md.
 *
 * Design notes:
 *   - The session_id is 128 bits, randomly generated from get_random_bytes()
 *     with the leading byte forced non-zero so id==0 stays reserved for the
 *     init agent ns ("not in a session").
 *   - Counters are per-cpu to make the hot syscall path lock-free; the
 *     snapshot path sums across CPUs.
 *   - Lifetime: refcounted by kref; last reference drop emits the
 *     agent_session_end tracepoint and frees the namespace.
 *   - The reaper is a kworker fired by an htimer at boot; it walks the live
 *     namespace list once a minute, killing tasks in any NS older than
 *     sysctl_agent_ns_max_lifetime.
 */

#include <linux/agent_namespaces.h>
#include <linux/cred.h>
#include <linux/err.h>
#include <linux/jiffies.h>
#include <linux/ktime.h>
#include <linux/list.h>
#include <linux/proc_ns.h>
#include <linux/random.h>
#include <linux/sched.h>
#include <linux/sched/signal.h>
#include <linux/seq_file.h>
#include <linux/signal.h>
#include <linux/slab.h>
#include <linux/spinlock.h>
#include <linux/sysctl.h>
#include <linux/user_namespace.h>
#include <linux/workqueue.h>

#define CREATE_TRACE_POINTS
#include <trace/events/agent_ns.h>

int sysctl_agent_ns_enabled __read_mostly = 1;
unsigned int sysctl_agent_ns_max_lifetime __read_mostly = AGENT_NS_DEFAULT_LIFETIME_S;

static DEFINE_SPINLOCK(agent_ns_list_lock);
static LIST_HEAD(agent_ns_list);

/* every live agent_namespace is on this list while it has tasks */
struct agent_ns_link {
	struct list_head node;
	struct agent_namespace *ns;
};

static struct kmem_cache *agent_ns_cache;
static struct kmem_cache *agent_ns_link_cache;

/* init ns: bytes are all zero (reserved "no session"); intent_tag empty. */
struct agent_namespace init_agent_ns = {
	.ns		= { .ops = &agentns_operations },
	.kref		= KREF_INIT(2),
	.user_ns	= &init_user_ns,
	.intent_lock	= __SPIN_LOCK_UNLOCKED(init_agent_ns.intent_lock),
};
EXPORT_SYMBOL_GPL(init_agent_ns);

/* forward decls for the namespace operations table */
static struct ns_common *agentns_get(struct task_struct *task);
static void agentns_put(struct ns_common *ns);
static int agentns_install(struct nsset *nsset, struct ns_common *ns);
static struct user_namespace *agentns_owner(struct ns_common *ns);

const struct proc_ns_operations agentns_operations = {
	.name	= "agent",
	.type	= CLONE_NEWAGENT,
	.get	= agentns_get,
	.put	= agentns_put,
	.install = agentns_install,
	.owner	= agentns_owner,
};
EXPORT_SYMBOL_GPL(agentns_operations);

static void generate_session_id(struct agent_session_id *id)
{
	get_random_bytes(id->bytes, AGENT_NS_ID_BYTES);
	/* keep id != 0 (zero is reserved for init_agent_ns) */
	if (unlikely(id->bytes[0] == 0))
		id->bytes[0] = 1;
}

int agent_session_id_format(const struct agent_session_id *id, char *buf, size_t len)
{
	if (len < AGENT_NS_ID_BYTES * 2 + 1)
		return -EINVAL;
	return scnprintf(buf, len, "%*phN", AGENT_NS_ID_BYTES, id->bytes);
}
EXPORT_SYMBOL_GPL(agent_session_id_format);

static struct agent_ns_link *find_link_locked(struct agent_namespace *ns)
{
	struct agent_ns_link *l;
	list_for_each_entry(l, &agent_ns_list, node)
		if (l->ns == ns)
			return l;
	return NULL;
}

static int add_to_live_list(struct agent_namespace *ns)
{
	struct agent_ns_link *l = kmem_cache_alloc(agent_ns_link_cache, GFP_KERNEL);
	if (!l)
		return -ENOMEM;
	l->ns = ns;
	spin_lock(&agent_ns_list_lock);
	list_add(&l->node, &agent_ns_list);
	spin_unlock(&agent_ns_list_lock);
	return 0;
}

static void remove_from_live_list(struct agent_namespace *ns)
{
	struct agent_ns_link *l;
	spin_lock(&agent_ns_list_lock);
	l = find_link_locked(ns);
	if (l)
		list_del(&l->node);
	spin_unlock(&agent_ns_list_lock);
	if (l)
		kmem_cache_free(agent_ns_link_cache, l);
}

static struct agent_namespace *create_agent_ns(struct user_namespace *user_ns,
					       struct agent_namespace *parent)
{
	struct agent_namespace *ns;
	int err;

	if (!sysctl_agent_ns_enabled)
		return ERR_PTR(-ENOSYS);

	ns = kmem_cache_zalloc(agent_ns_cache, GFP_KERNEL);
	if (!ns)
		return ERR_PTR(-ENOMEM);

	err = ns_alloc_inum(&ns->ns);
	if (err)
		goto fail_inum;

	ns->ns.ops = &agentns_operations;
	kref_init(&ns->kref);
	ns->user_ns = get_user_ns(user_ns);
	spin_lock_init(&ns->intent_lock);
	ns->created_ns = ktime_get_boot_ns();
	atomic_set(&ns->live_tasks, 0);

	ns->counters = alloc_percpu(struct agent_ns_counters_pcp);
	if (!ns->counters) {
		err = -ENOMEM;
		goto fail_counters;
	}

	generate_session_id(&ns->session_id);
	if (parent)
		ns->parent_session_id = parent->session_id;
	/* intent_tag default: empty; userspace sets via prctl */

	err = add_to_live_list(ns);
	if (err)
		goto fail_live;

	trace_agent_session_start(&ns->session_id, &ns->parent_session_id,
				  ns->intent_tag, current->tgid);
	return ns;

fail_live:
	free_percpu(ns->counters);
fail_counters:
	put_user_ns(ns->user_ns);
	ns_free_inum(&ns->ns);
fail_inum:
	kmem_cache_free(agent_ns_cache, ns);
	return ERR_PTR(err);
}

struct agent_namespace *copy_agent_ns(unsigned long flags,
				      struct user_namespace *user_ns,
				      struct agent_namespace *old)
{
	if (!(flags & CLONE_NEWAGENT))
		return get_agent_ns(old);
	/* CLONE_NEWAGENT requires CAP_SYS_ADMIN in the current user_ns OR
	 * already being inside a non-init agent_ns.  v0.1 is conservative.
	 */
	if (!ns_capable(user_ns, CAP_SYS_ADMIN) && old == &init_agent_ns)
		return ERR_PTR(-EPERM);
	return create_agent_ns(user_ns, old);
}
EXPORT_SYMBOL_GPL(copy_agent_ns);

void agent_ns_snapshot(struct agent_namespace *ns,
		       struct agent_ns_counters_snapshot *out)
{
	int cpu;
	memset(out, 0, sizeof(*out));
	if (ns == &init_agent_ns || !ns->counters)
		return;
	for_each_possible_cpu(cpu) {
		struct agent_ns_counters_pcp *c = per_cpu_ptr(ns->counters, cpu);
		out->total_syscalls += READ_ONCE(c->total_syscalls);
		out->openat_count   += READ_ONCE(c->openat_count);
		out->write_bytes    += READ_ONCE(c->write_bytes);
		out->connect_count  += READ_ONCE(c->connect_count);
		out->unlink_count   += READ_ONCE(c->unlink_count);
		out->fork_count     += READ_ONCE(c->fork_count);
	}
	out->elapsed_ns = ktime_get_boot_ns() - ns->created_ns;
}
EXPORT_SYMBOL_GPL(agent_ns_snapshot);

void free_agent_ns(struct kref *kref)
{
	struct agent_namespace *ns = container_of(kref, struct agent_namespace, kref);
	struct agent_ns_counters_snapshot snap;

	agent_ns_snapshot(ns, &snap);
	trace_agent_session_end(&ns->session_id, &snap);

	remove_from_live_list(ns);
	free_percpu(ns->counters);
	put_user_ns(ns->user_ns);
	ns_free_inum(&ns->ns);
	kmem_cache_free(agent_ns_cache, ns);
}
EXPORT_SYMBOL_GPL(free_agent_ns);

void agent_ns_task_exit(struct task_struct *tsk)
{
	struct agent_namespace *ns;
	if (!tsk->nsproxy)
		return;
	ns = tsk->nsproxy->agent_ns;
	if (!ns || ns == &init_agent_ns)
		return;
	atomic_dec(&ns->live_tasks);
}
EXPORT_SYMBOL_GPL(agent_ns_task_exit);

/* ---- counter fast path ---- */
static __always_inline struct agent_ns_counters_pcp *cur_counters(void)
{
	struct nsproxy *np;
	struct agent_namespace *ns;
	if (unlikely(!sysctl_agent_ns_enabled))
		return NULL;
	np = current->nsproxy;
	if (!np)
		return NULL;
	ns = np->agent_ns;
	if (!ns || ns == &init_agent_ns || !ns->counters)
		return NULL;
	return this_cpu_ptr(ns->counters);
}

void agent_ns_count_syscall(void)
{
	struct agent_ns_counters_pcp *c = cur_counters();
	if (c) this_cpu_inc(c->total_syscalls);
}
EXPORT_SYMBOL_GPL(agent_ns_count_syscall);

void agent_ns_count_openat(void)
{
	struct agent_ns_counters_pcp *c = cur_counters();
	if (c) this_cpu_inc(c->openat_count);
}
EXPORT_SYMBOL_GPL(agent_ns_count_openat);

void agent_ns_count_write(size_t bytes)
{
	struct agent_ns_counters_pcp *c = cur_counters();
	if (c) this_cpu_add(c->write_bytes, bytes);
}
EXPORT_SYMBOL_GPL(agent_ns_count_write);

void agent_ns_count_connect(void)
{
	struct agent_ns_counters_pcp *c = cur_counters();
	if (c) this_cpu_inc(c->connect_count);
}
EXPORT_SYMBOL_GPL(agent_ns_count_connect);

void agent_ns_count_unlink(void)
{
	struct agent_ns_counters_pcp *c = cur_counters();
	if (c) this_cpu_inc(c->unlink_count);
}
EXPORT_SYMBOL_GPL(agent_ns_count_unlink);

void agent_ns_count_fork(void)
{
	struct agent_ns_counters_pcp *c = cur_counters();
	if (c) this_cpu_inc(c->fork_count);
}
EXPORT_SYMBOL_GPL(agent_ns_count_fork);

/* ---- ns_common ops ---- */
static struct ns_common *agentns_get(struct task_struct *task)
{
	struct agent_namespace *ns = NULL;
	task_lock(task);
	if (task->nsproxy) {
		ns = task->nsproxy->agent_ns;
		get_agent_ns(ns);
	}
	task_unlock(task);
	return ns ? &ns->ns : NULL;
}

static void agentns_put(struct ns_common *ns)
{
	put_agent_ns(container_of(ns, struct agent_namespace, ns));
}

static int agentns_install(struct nsset *nsset, struct ns_common *ns)
{
	struct agent_namespace *new = container_of(ns, struct agent_namespace, ns);

	if (!ns_capable(nsset->cred->user_ns, CAP_SYS_ADMIN))
		return -EPERM;

	get_agent_ns(new);
	put_agent_ns(nsset->nsproxy->agent_ns);
	nsset->nsproxy->agent_ns = new;
	return 0;
}

static struct user_namespace *agentns_owner(struct ns_common *ns)
{
	return container_of(ns, struct agent_namespace, ns)->user_ns;
}

/* ---- /proc surface ---- */
ssize_t agent_ns_proc_id_show(struct seq_file *m, void *v)
{
	struct task_struct *task = m->private;
	struct agent_namespace *ns;
	char buf[AGENT_NS_ID_BYTES * 2 + 1];

	task_lock(task);
	ns = task->nsproxy ? task->nsproxy->agent_ns : NULL;
	if (!ns) {
		task_unlock(task);
		return -ESRCH;
	}
	agent_session_id_format(&ns->session_id, buf, sizeof(buf));
	task_unlock(task);
	seq_printf(m, "%s\n", buf);
	return 0;
}
EXPORT_SYMBOL_GPL(agent_ns_proc_id_show);

ssize_t agent_ns_proc_intent_show(struct seq_file *m, void *v)
{
	struct task_struct *task = m->private;
	struct agent_namespace *ns;
	char buf[AGENT_NS_INTENT_MAX + 1];

	task_lock(task);
	ns = task->nsproxy ? task->nsproxy->agent_ns : NULL;
	if (!ns) {
		task_unlock(task);
		return -ESRCH;
	}
	spin_lock(&ns->intent_lock);
	strscpy(buf, ns->intent_tag, sizeof(buf));
	spin_unlock(&ns->intent_lock);
	task_unlock(task);
	seq_printf(m, "%s\n", buf);
	return 0;
}
EXPORT_SYMBOL_GPL(agent_ns_proc_intent_show);

ssize_t agent_ns_proc_counters_show(struct seq_file *m, void *v)
{
	struct task_struct *task = m->private;
	struct agent_namespace *ns;
	struct agent_ns_counters_snapshot snap;

	task_lock(task);
	ns = task->nsproxy ? task->nsproxy->agent_ns : NULL;
	if (!ns) {
		task_unlock(task);
		return -ESRCH;
	}
	get_agent_ns(ns);
	task_unlock(task);

	agent_ns_snapshot(ns, &snap);
	seq_printf(m,
		"{\n"
		"  \"total_syscalls\": %llu,\n"
		"  \"openat_count\": %llu,\n"
		"  \"write_bytes\": %llu,\n"
		"  \"connect_count\": %llu,\n"
		"  \"unlink_count\": %llu,\n"
		"  \"fork_count\": %llu,\n"
		"  \"elapsed_ns\": %llu\n"
		"}\n",
		snap.total_syscalls, snap.openat_count, snap.write_bytes,
		snap.connect_count, snap.unlink_count, snap.fork_count,
		snap.elapsed_ns);
	put_agent_ns(ns);
	return 0;
}
EXPORT_SYMBOL_GPL(agent_ns_proc_counters_show);

/* ---- prctl handlers ---- */
int agent_ns_prctl(struct task_struct *me, int option,
		   unsigned long arg2, unsigned long arg3,
		   unsigned long arg4, unsigned long arg5)
{
	struct agent_namespace *ns;
	int ret = 0;

	if (!me->nsproxy)
		return -EINVAL;
	ns = me->nsproxy->agent_ns;
	if (!ns)
		return -EINVAL;

	switch (option) {
	case PR_GET_AGENT_SESSION_ID: {
		struct agent_session_id_uapi out;
		BUILD_BUG_ON(sizeof(out) != AGENT_NS_ID_BYTES);
		memcpy(out.bytes, ns->session_id.bytes, AGENT_NS_ID_BYTES);
		if (copy_to_user((void __user *)arg2, &out, sizeof(out)))
			return -EFAULT;
		return 0;
	}
	case PR_GET_AGENT_PARENT_ID: {
		struct agent_session_id_uapi out;
		memcpy(out.bytes, ns->parent_session_id.bytes, AGENT_NS_ID_BYTES);
		if (copy_to_user((void __user *)arg2, &out, sizeof(out)))
			return -EFAULT;
		return 0;
	}
	case PR_SET_AGENT_INTENT_TAG: {
		char tag[AGENT_NS_INTENT_MAX + 1];
		long len;
		unsigned long now = jiffies;

		spin_lock(&ns->intent_lock);
		if (time_before(now, ns->intent_last_set_jiffies + HZ)) {
			spin_unlock(&ns->intent_lock);
			return -EAGAIN; /* rate-limited */
		}
		spin_unlock(&ns->intent_lock);

		len = strncpy_from_user(tag, (const char __user *)arg2,
					AGENT_NS_INTENT_MAX);
		if (len < 0)
			return len;
		tag[len] = '\0';

		spin_lock(&ns->intent_lock);
		memcpy(ns->intent_tag, tag, len + 1);
		ns->intent_last_set_jiffies = now;
		spin_unlock(&ns->intent_lock);

		trace_agent_intent_set(&ns->session_id, ns->intent_tag);
		return 0;
	}
	case PR_GET_AGENT_INTENT_TAG: {
		char buf[AGENT_NS_INTENT_MAX + 1];
		spin_lock(&ns->intent_lock);
		strscpy(buf, ns->intent_tag, sizeof(buf));
		spin_unlock(&ns->intent_lock);
		if (copy_to_user((void __user *)arg2, buf, strlen(buf) + 1))
			return -EFAULT;
		return 0;
	}
	case PR_SET_AGENT_BUDGET_LIMITS: {
		struct agent_ns_budget_uapi u;
		if (copy_from_user(&u, (void __user *)arg2, sizeof(u)))
			return -EFAULT;
		if (u.action > 2)
			return -EINVAL;
		WRITE_ONCE(ns->budget.max_syscalls, u.max_syscalls);
		WRITE_ONCE(ns->budget.max_write_bytes, u.max_write_bytes);
		WRITE_ONCE(ns->budget.max_elapsed_ns, u.max_elapsed_ns);
		WRITE_ONCE(ns->budget.action, u.action);
		return 0;
	}
	case PR_GET_AGENT_COUNTERS: {
		struct agent_ns_counters_snapshot snap;
		struct agent_ns_counters_uapi u;
		agent_ns_snapshot(ns, &snap);
		u.total_syscalls = snap.total_syscalls;
		u.openat_count   = snap.openat_count;
		u.write_bytes    = snap.write_bytes;
		u.connect_count  = snap.connect_count;
		u.unlink_count   = snap.unlink_count;
		u.fork_count     = snap.fork_count;
		u.elapsed_ns     = snap.elapsed_ns;
		if (copy_to_user((void __user *)arg2, &u, sizeof(u)))
			return -EFAULT;
		return 0;
	}
	default:
		ret = -EINVAL;
	}
	return ret;
}
EXPORT_SYMBOL_GPL(agent_ns_prctl);

/* ---- max-lifetime reaper ---- */
static void reaper_work_fn(struct work_struct *w);
static DECLARE_DELAYED_WORK(reaper_work, reaper_work_fn);

static void kill_ns_tasks(struct agent_namespace *ns)
{
	struct task_struct *p;
	struct agent_ns_counters_snapshot snap;
	agent_ns_snapshot(ns, &snap);
	pr_info("agent_ns: reaping NS session %16phN (age %llu ns) — killing live tasks\n",
		ns->session_id.bytes, snap.elapsed_ns);

	rcu_read_lock();
	for_each_process(p) {
		if (p->nsproxy && p->nsproxy->agent_ns == ns) {
			send_sig(SIGKILL, p, 1);
		}
	}
	rcu_read_unlock();
}

static void reaper_work_fn(struct work_struct *w)
{
	struct agent_ns_link *l, *tmp;
	u64 now = ktime_get_boot_ns();
	u64 max_ns;

	if (!READ_ONCE(sysctl_agent_ns_enabled))
		goto reschedule;

	max_ns = (u64)READ_ONCE(sysctl_agent_ns_max_lifetime) * NSEC_PER_SEC;
	if (!max_ns)
		goto reschedule;

	spin_lock(&agent_ns_list_lock);
	list_for_each_entry_safe(l, tmp, &agent_ns_list, node) {
		struct agent_namespace *ns = l->ns;
		if ((now - ns->created_ns) > max_ns) {
			spin_unlock(&agent_ns_list_lock);
			kill_ns_tasks(ns);
			spin_lock(&agent_ns_list_lock);
		}
	}
	spin_unlock(&agent_ns_list_lock);

reschedule:
	schedule_delayed_work(&reaper_work, HZ * 60);
}

void agent_ns_reaper_kick(void)
{
	mod_delayed_work(system_wq, &reaper_work, 0);
}
EXPORT_SYMBOL_GPL(agent_ns_reaper_kick);

/* ---- sysctl table ---- */
static struct ctl_table agent_ns_sysctls[] = {
	{
		.procname	= "enabled",
		.data		= &sysctl_agent_ns_enabled,
		.maxlen		= sizeof(int),
		.mode		= 0644,
		.proc_handler	= proc_dointvec_minmax,
		.extra1		= SYSCTL_ZERO,
		.extra2		= SYSCTL_ONE,
	},
	{
		.procname	= "max_lifetime",
		.data		= &sysctl_agent_ns_max_lifetime,
		.maxlen		= sizeof(unsigned int),
		.mode		= 0644,
		.proc_handler	= proc_douintvec,
	},
	{ }
};

static int __init agent_ns_init(void)
{
	agent_ns_cache = KMEM_CACHE(agent_namespace, SLAB_PANIC | SLAB_ACCOUNT);
	agent_ns_link_cache = KMEM_CACHE(agent_ns_link, SLAB_PANIC | SLAB_ACCOUNT);

	/* set init_agent_ns inum so /proc/$PID/ns/agent for init tasks resolves */
	if (ns_alloc_inum(&init_agent_ns.ns))
		pr_warn("agent_ns: failed to allocate init NS inum\n");
	init_agent_ns.counters = alloc_percpu(struct agent_ns_counters_pcp);
	init_agent_ns.created_ns = ktime_get_boot_ns();

	register_sysctl_init("kernel/agent_ns", agent_ns_sysctls);
	schedule_delayed_work(&reaper_work, HZ * 60);

	pr_info("agent_ns: initialized (enabled=%d, max_lifetime=%us)\n",
		sysctl_agent_ns_enabled, sysctl_agent_ns_max_lifetime);
	return 0;
}
early_initcall(agent_ns_init);
