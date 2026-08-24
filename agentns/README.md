# agentns — agent namespace for the wintermute kernel

Vendor patch series adding an eighth Linux namespace type: the **agent
namespace**. Every task carries an opaque 128-bit `agent_session_id`, an
optional `intent_tag`, and a set of per-namespace counters. The kernel
exposes the session at `/proc/$PID/agent_session`,
`/proc/$PID/agent_counters`, and `/proc/$PID/ns/agent`.

**Creation mechanism: `prctl(PR_SET_AGENT_NS)`, not a clone flag.** The
32-bit clone/unshare flag space is exhausted (`CSIGNAL`..`CLONE_IO` are all
claimed; `CLONE_NEWTIME` 0x80 took the last low bit), so there is no free
bit reachable by `unshare(2)`. An early design hung the namespace off a
clone bit (`CLONE_NEWAGENT`); the value `0x00000100` it used **is**
`CLONE_VM`, so `unshare(CLONE_NEWAGENT)` always returned `EINVAL` and the
namespace could never be created — the diagnosis the `assay-agentns`
attestation reproduces on the booted kernel. The fix routes create-and-enter
through a dedicated prctl op that needs no clone bit at all. See
[Why prctl, not a clone flag](#why-prctl-not-a-clone-flag) below.

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
│   ├── 0010-add-new-files.patch          (placeholder; see scripts/build.sh)
│   └── 0011-prctl-PR_SET_AGENT_NS-create.patch   create-and-enter via prctl (the fix)
├── kernel/
│   └── agent_namespaces.c    new file; the heart of the implementation
├── include/
│   ├── linux/agent_namespaces.h          in-kernel API
│   ├── uapi/linux/agent_namespaces.h     userspace API (prctl options, structs)
│   └── trace/events/agent_ns.h           tracepoints
├── scripts/
│   ├── build.sh              fetch sources, apply patches, build kernel package
│   └── rebase-check.sh       /self-review-friendly drift detector
├── userspace/
│   ├── agentns-unshare.c     prctl(PR_SET_AGENT_NS) create-and-exec shim (USE THIS)
│   ├── agent-wrap.c          DEPRECATED legacy unshare(CLONE_NEWAGENT) wrapper
│   └── Makefile
├── tests/
│   ├── test_prctl_ns.c       round-trip: prctl(PR_SET_AGENT_NS) → /proc → prctl (primary)
│   ├── test_unshare.c        legacy probe: asserts unshare(0x100) fails, then prctl-creates
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

## Installation

Once the kernel package is built (see `scripts/build.sh` or the
`~/wintermute/wintermute-kernel/pkg/` PKGBUILD), install it with the
usual Arch flow:

```bash
# from the pkg/ directory
sudo pacman -U linux-wintermute-*.pkg.tar.zst linux-wintermute-headers-*.pkg.tar.zst
# or via makepkg:
cd ~/wintermute/wintermute-kernel/pkg && makepkg -e --skippgpcheck --noconfirm -i
```

After installation, reboot to activate the patched kernel. Once running
under `linux-wintermute`, apply the agentns patches with:

```bash
cd ~/wintermute/agentns && python3 scripts/apply-agentns.py
```

The agent namespace is enabled by default (`CONFIG_AGENT_NS=y`). You can
disable it at runtime without rebooting:

```bash
sysctl -w kernel.agent_ns.enabled=0   # makes all counter hooks no-ops
```

Verify the namespace is live:

```bash
agentns-unshare cat /proc/self/agent_session   # should print a non-zero 128-bit id
```

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
sudo ./test_prctl_ns      # primary: prctl(PR_SET_AGENT_NS) → /proc → prctl
sudo ./test_unshare       # legacy probe: unshare(0x100) must fail, then prctl-create
sudo ./test_inheritance.sh
sudo ./test_bpf_tracepoints.sh
```

`test_prctl_ns` is hermetic — no Claude or wintermute deps. It exits
77 (autotest "skip") if `/proc/self/agent_session` is missing or
`PR_SET_AGENT_NS` is unsupported, so running it on the stock kernel is
harmless. To launch a command inside a fresh agent namespace from the shell,
use the `userspace/agentns-unshare` shim (needs `CAP_SYS_ADMIN`):

```bash
sudo setcap cap_sys_admin+ep ./userspace/agentns-unshare
AGENT_INTENT=my-task ./userspace/agentns-unshare cat /proc/self/agent_session
```

## Sysctls

```
kernel.agent_ns.enabled         (0|1, default 1)
kernel.agent_ns.max_lifetime    (seconds, default 86400 = 24h)
```

The reaper runs once a minute and SIGKILLs every task in any NS older
than `max_lifetime`. This is the "leaked tracer" belt-and-suspenders
described in the PRD.

## Why prctl, not a clone flag

The agent namespace is created with `prctl(PR_SET_AGENT_NS)`, which
allocates a fresh `agent_ns` (new 128-bit session id, zeroed counters,
cleared intent tag) and installs it on the calling task's `nsproxy`. No
clone flag is involved.

**Why the legacy clone-flag approach was abandoned.** The original design
assumed a free legacy 32-bit clone bit existed for an eighth namespace
type. It does not: bits `0x80` (`CSIGNAL`-adjacent) through `0x80000000`
(`CLONE_IO`) are all claimed by upstream, and `CLONE_NEWTIME` (`0x80`)
consumed the last low bit years ago. An early implementation `#define`d
`CLONE_NEWAGENT` as `0x00000100` — which **is** `CLONE_VM`. The
consequences, both confirmed:

- `unshare(CLONE_NEWAGENT)` returns `EINVAL`, because
  `check_unshare_flags()` reads the bit as `CLONE_VM` and rejects it
  before the agentns code runs. The namespace can never be created. This
  is exactly what the **`assay-agentns`** attestation reports on the
  booted kernel (`/proc/self/agent_session` reads 32 zeros).
- Every kthread fork was misread as also requesting a new agent NS;
  `copy_agent_ns()` ran before `agent_ns_cache` was initialized and the
  kernel hung on the firmware splash with no console output (2026-05-25).

Picking a *different* legacy bit cannot work — every bit is occupied. So
creation moved off clone flags entirely, onto a dedicated prctl op (the
prctl dispatch already shipped for `PR_GET_AGENT_*`). `unshare(2)`'s flag
word only accepts the 32-bit set, so a 64-bit clone3-only flag wouldn't be
reachable by the existing `unshare`-based userspace either — prctl is the
one path that is both free of the exhausted flag space and callable from
plain userspace.

A `CLONE_NEWAGENT` flag still exists at `0x400000000ULL` — the next free
64-bit slot above `CLONE_INTO_CGROUP` (`0x200000000ULL`) — for the
`clone3(2)` inheritance path and as the `ns_common` type tag
(`AGENT_NS_TYPE`). It is unreachable from `unshare(2)` and is **not** the
creation mechanism. The agentns init carries a `BUILD_BUG_ON` against
every upstream `CLONE_*` bit, so a future re-aliasing of a low clone bit
fails to compile rather than bricking boot, and `apply-agentns.py --check`
fails if a stale `0x100` `CLONE_NEWAGENT` creation `#define` reappears.

## Risks & caveats

- **Kernel rebase cost**: every `linux` package bump may need a
  rebase. Run `scripts/rebase-check.sh` from `/self-review`.
- **Counter overflow**: per-cpu u64 counters wrap at 2^64 (write_bytes
  in particular). A long-running session writing 10 GB/s would still
  take ~58 years to wrap — fine.
- **Interaction with userns**: v0.1 requires `CAP_SYS_ADMIN` to
  `prctl(PR_SET_AGENT_NS)` from the init NS. Rootless creation is
  Open Question #3 in the PRD; deferred.
- **Not for upstream**: PRD explicitly accepts this is a vendor
  fork — Linux mainline will not take a new namespace type for an
  application-specific use case. The maintenance cost is the patch
  series rebasing.

## Identity

Repo identity per the standing convention (see `~/CLAUDE_SELF.md`):
`j0yen <jyen.tech@gmail.com>` via per-command
`git -c user.email=… -c user.name=…`.
