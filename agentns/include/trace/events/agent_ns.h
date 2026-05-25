/* SPDX-License-Identifier: GPL-2.0 */
#undef TRACE_SYSTEM
#define TRACE_SYSTEM agent_ns

#if !defined(_TRACE_AGENT_NS_H) || defined(TRACE_HEADER_MULTI_READ)
#define _TRACE_AGENT_NS_H

#include <linux/tracepoint.h>
#include <linux/agent_namespaces.h>

TRACE_EVENT(agent_session_start,

	TP_PROTO(const struct agent_session_id *id,
		 const struct agent_session_id *parent_id,
		 const char *intent_tag,
		 pid_t root_pid),

	TP_ARGS(id, parent_id, intent_tag, root_pid),

	TP_STRUCT__entry(
		__array(u8, sid, AGENT_NS_ID_BYTES)
		__array(u8, psid, AGENT_NS_ID_BYTES)
		__string(intent, intent_tag ? intent_tag : "")
		__field(pid_t, root_pid)
	),

	TP_fast_assign(
		memcpy(__entry->sid, id->bytes, AGENT_NS_ID_BYTES);
		memcpy(__entry->psid, parent_id->bytes, AGENT_NS_ID_BYTES);
		__assign_str(intent);
		__entry->root_pid = root_pid;
	),

	TP_printk("session_id=%16phN parent=%16phN intent=%s root_pid=%d",
		  __entry->sid, __entry->psid, __get_str(intent), __entry->root_pid)
);

TRACE_EVENT(agent_session_end,

	TP_PROTO(const struct agent_session_id *id,
		 const struct agent_ns_counters_snapshot *snap),

	TP_ARGS(id, snap),

	TP_STRUCT__entry(
		__array(u8, sid, AGENT_NS_ID_BYTES)
		__field(u64, total_syscalls)
		__field(u64, openat_count)
		__field(u64, write_bytes)
		__field(u64, connect_count)
		__field(u64, unlink_count)
		__field(u64, fork_count)
		__field(u64, elapsed_ns)
	),

	TP_fast_assign(
		memcpy(__entry->sid, id->bytes, AGENT_NS_ID_BYTES);
		__entry->total_syscalls = snap->total_syscalls;
		__entry->openat_count = snap->openat_count;
		__entry->write_bytes = snap->write_bytes;
		__entry->connect_count = snap->connect_count;
		__entry->unlink_count = snap->unlink_count;
		__entry->fork_count = snap->fork_count;
		__entry->elapsed_ns = snap->elapsed_ns;
	),

	TP_printk("session_id=%16phN syscalls=%llu openat=%llu write_bytes=%llu connect=%llu unlink=%llu fork=%llu elapsed_ns=%llu",
		  __entry->sid,
		  __entry->total_syscalls,
		  __entry->openat_count,
		  __entry->write_bytes,
		  __entry->connect_count,
		  __entry->unlink_count,
		  __entry->fork_count,
		  __entry->elapsed_ns)
);

TRACE_EVENT(agent_intent_set,

	TP_PROTO(const struct agent_session_id *id, const char *intent_tag),

	TP_ARGS(id, intent_tag),

	TP_STRUCT__entry(
		__array(u8, sid, AGENT_NS_ID_BYTES)
		__string(intent, intent_tag ? intent_tag : "")
	),

	TP_fast_assign(
		memcpy(__entry->sid, id->bytes, AGENT_NS_ID_BYTES);
		__assign_str(intent);
	),

	TP_printk("session_id=%16phN intent=%s",
		  __entry->sid, __get_str(intent))
);

#endif /* _TRACE_AGENT_NS_H */

#include <trace/define_trace.h>
