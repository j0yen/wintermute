# agentns — `CLONE_NEWAGENT` for the wintermute kernel

Vendor patch series adding an eighth Linux namespace type:
`CLONE_NEWAGENT`. Every task carries an opaque 128-bit
`agent_session_id`, an optional `intent_tag`, and a set of
per-namespace counters. The kernel exposes the session at
`/proc/$PID/agent_session`, `/proc/$PID/agent_counters`, and
`/proc/$PID/ns/agent`.

See [`PRD-agent-namespace.md`](../autobuilder/PRD-agent-namespace.md)
for the design rationale. This directory is the implementation.

## What ships here

```
agentns/
├── patches/                  unified diffs against linux mainline
│   ├── 0001-uapi-sched-add-CLONE_NEWAGENT.patch
│   ├── 0002-nsproxy-add-agent_ns.patch
│   ├── 0003-fork-count-and-init.patch
│   ├── 0004-exit-notify-agent_ns.patch
│   ├── 0005-prctl-dispatch-PR_AGENT.patch
│   ├── 0006-proc-expose-agent_ns.patch
│   ├── 0007-makefile-and-kconfig.patch
│   ├── 0008-syscall-counter-hooks.patch
│   ├── 0009-proc_ns_operations-register.patch
│   └── 0010-add-new-files.patch          (placeholder; see scripts/build.sh)
├── kernel/
│   └── agent_namespaces.c    new file; the heart of the implementation
├── include/
│   ├── linux/agent_namespaces.h          in-kernel API
│   ├── uapi/linux/agent_namespaces.h     userspace API (prctl options, structs)
│   └── trace/events/agent_ns.h           tracepoints
├── scripts/
│   ├── build.sh              fetch sources, apply patches, build kernel package
│   └── rebase-check.sh       /self-review-friendly drift detector
├── tests/
│   ├── test_unshare.c        round-trip: unshare → /proc → prctl → counters
│   ├── test_inheritance.sh   child inherits parent's session_id
│   ├── test_bpf_tracepoints.sh   tracepoints visible to bpftrace
│   └── Makefile
└── docs/                     additional design notes
```

## Building

The target is the kernel `pacman -Q linux` reports (currently
`linux 7.0.9.arch1-1`). The build script fetches matching sources via
`asp` (or `pkgctl` if `asp` is unavailable), applies the patch series,
copies the new files in, and runs `make`.

```bash
# fetch sources + apply patches + build kernel image
./scripts/build.sh

# patches only, no build
BUILD_KERNEL=0 ./scripts/build.sh

# target a specific kernel
KVER=7.1.2 ./scripts/build.sh
```

Install with the usual Arch flow (`makepkg -i` from the package
directory, or copy the resulting `vmlinuz`/modules into `/boot` and
regenerate the initramfs). Reboot to pick up the patched kernel. The
agent namespace is enabled by default (`CONFIG_AGENT_NS=y`); the user
can disable at runtime with `sysctl -w kernel.agent_ns.enabled=0`,
which makes every counter hook a no-op.

## Rebasing onto a new kernel

```bash
./scripts/rebase-check.sh
```

reports which patches no longer apply cleanly. The drift is usually
small (a context line shifts when adjacent code changes). When a
patch genuinely conflicts:

1. Apply by hand: `cd $KSRC && patch -p1 --merge < patches/000N-foo.patch`
2. Resolve the `.rej` hunks.
3. Regenerate the patch: `cd $KSRC && git diff > .../patches/000N-foo.patch`

## What's wired

| Phase | Status | What it does |
|------|--------|---|
| 0    | done   | `task_struct->nsproxy->agent_ns`, `/proc/$PID/agent_session`, default init NS for unmodified tasks |
| 1    | done   | per-cpu counters, prctl interface (`PR_GET_AGENT_SESSION_ID`, `PR_SET_AGENT_INTENT_TAG`, `PR_GET_AGENT_COUNTERS`, `PR_SET_AGENT_BUDGET_LIMITS`), `agent_session_end` tracepoint with counters |
| 2    | done   | tracepoints (`agent_ns:agent_session_start/_end/_intent_set`) exposed at `/sys/kernel/tracing/events/agent_ns/` |
| 3    | done   | LSM stamping xattrs — lives in [`provfs/lsm/`](../provfs/lsm/), reads `current->nsproxy->agent_ns->session_id` when `CONFIG_AGENT_NS=y`, falls back to `comm:pid:uid` otherwise |
| 4    | done   | budget-limit enforcement: reaper checks `ns->budget.{max_syscalls,max_write_bytes,max_elapsed_ns}` every 60s and performs `action` (0=log / 1=SIGTERM / 2=SIGKILL). See `tests/test_budget_enforce.sh` |

## Tests

From the running patched kernel:

```bash
cd tests
make
sudo ./test_unshare       # round-trip: unshare(CLONE_NEWAGENT) → /proc → prctl
sudo ./test_inheritance.sh
sudo ./test_bpf_tracepoints.sh
```

`test_unshare` is hermetic — no Claude or wintermute deps. It exits
77 (autotest "skip") if `/proc/self/agent_session` is missing, so
running it on the stock kernel is harmless.

## Sysctls

```
kernel.agent_ns.enabled         (0|1, default 1)
kernel.agent_ns.max_lifetime    (seconds, default 86400 = 24h)
```

The reaper runs once a minute and SIGKILLs every task in any NS older
than `max_lifetime`. This is the "leaked tracer" belt-and-suspenders
described in the PRD.

## CLONE_NEWAGENT bit choice

`0x400000000ULL` — the next free 64-bit slot above upstream's
`CLONE_INTO_CGROUP` (`0x200000000ULL`). The 32-bit space (`0x80` ..
`0x80000000`) is fully claimed by upstream CLONE_* flags and is *not*
available for vendor extensions.

The PRD originally speculated `0x40000000`; that's the long-standing
`CLONE_NEWNET` bit. An earlier implementation drifted to `0x00000100`
— which aliases `CLONE_VM`, so every kthread fork was inadvertently
requesting a new agent namespace. `copy_agent_ns()` ran before
`agent_ns_cache` was initialized and the kernel hung on the firmware
splash with no console output (2026-05-25 boot attempts). The agentns
init now carries a `BUILD_BUG_ON` against every upstream `CLONE_*` bit
so a future collision fails to compile rather than bricks boot.

Because `CLONE_NEWAGENT` is above bit 32, it is reachable only via
`clone3(2)` — the legacy `clone(2)` syscall passes a 32-bit flag word
and cannot express it. This is intentional; the `unshare(2)` flags
parameter is `unsigned long`, which is 64-bit on x86_64.

## Risks & caveats

- **Kernel rebase cost**: every `linux` package bump may need a
  rebase. Run `scripts/rebase-check.sh` from `/self-review`.
- **Counter overflow**: per-cpu u64 counters wrap at 2^64 (write_bytes
  in particular). A long-running session writing 10 GB/s would still
  take ~58 years to wrap — fine.
- **Interaction with userns**: v0.1 requires `CAP_SYS_ADMIN` to
  `unshare(CLONE_NEWAGENT)` from the init NS. Rootless creation is
  Open Question #3 in the PRD; deferred.
- **Not for upstream**: PRD explicitly accepts this is a vendor
  fork — Linux mainline will not take a new namespace type for an
  application-specific use case. The maintenance cost is the patch
  series rebasing.

## Identity

Repo identity per the standing convention (see `~/CLAUDE_SELF.md`):
`j0yen <jyen.tech@gmail.com>` via per-command
`git -c user.email=… -c user.name=…`.
