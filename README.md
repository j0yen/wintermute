# wintermute

Local tools that let a coding agent on Linux run things safely, watch what
happened, undo it, and remember — so it makes fewer timid, low-leverage
choices. This repo is the front door to that ecosystem and the one command
that installs it.

A coding agent left to itself on a real machine is cautious for the wrong
reason: it can't cheaply tell what a command will do, what it did, or how to
take it back. So it under-reaches. The fix isn't a smarter prompt — it's
giving the agent the same primitives a careful engineer uses: a sandbox to
contain a command, observers to read the result, a transaction to roll an
edit back, a memory that survives the session. Each is a small tool that
does one thing; together they raise the ceiling on what the agent will
attempt.

Eight of those primitives ship here as single-file Python 3 scripts under
[`bin/`](bin) — `sbx`, `pevent`, `wchg`, `procstat`, `txn-edit`, `tcap`,
`ctrace`, `bpolicy`. No dependencies beyond the stdlib and the platform
binary each one shells out to (`bwrap`, `watchman`, `tmux`, `bpftrace`,
`bpftool`). The rest of the ecosystem — the Rust memory layer (`recall`),
the agent bus (`agorabus`), the self-review and runtime crates — lives in
companion repos, each its own project.

Two files make this repo the bootstrap point:

- [`bootstrap/install.sh`](bootstrap/install.sh) — clones each project repo,
  builds the Rust crates, symlinks binaries into `~/.local/bin`, and wires
  the Claude Code SessionStart hooks from `dotfiles/.claude/`. Safe to rerun;
  `--dry-run` shows the plan.
- [`REPOS.md`](REPOS.md) — the index of every companion repo, one line each.

## Linux compatibility

Wintermute is **Linux-only**. macOS, *BSD, Windows, and WSL2 are unsupported
— the eBPF tools (`ctrace`, `bpolicy`) need real Linux kernel features that
neither Darwin nor WSL2 expose reliably.

The binding constraint is `bpolicy`, which hooks `lsm/file_open`. Everything
else is much more permissive.

### Kernel requirements

| Feature           | Used by                | Minimum kernel |
| ----------------- | ---------------------- | -------------- |
| user namespaces   | `sbx` (`bwrap`)        | 3.8            |
| cgroup v2         | `sbx`, `procstat`      | 4.5 (5.x for full controllers) |
| `pidfd_open(2)`   | `pevent`               | 5.3            |
| eBPF + BTF        | `ctrace` (`bpftrace`)  | 5.4 with `CONFIG_DEBUG_INFO_BTF=y` |
| **eBPF-LSM**      | **`bpolicy`**          | **5.7 with `CONFIG_BPF_LSM=y` *and* `lsm=…,bpf` in kernel cmdline** |

`bpolicy` will refuse to load if `CONFIG_BPF_LSM` is off or `bpf` is missing
from the active LSM list (`cat /sys/kernel/security/lsm`). The other seven
tools work fine on any modern kernel.

### Tested / known-good distros

| Distro                  | Status      | Notes |
| ----------------------- | ----------- | ----- |
| Arch Linux (rolling)    | primary     | development environment; everything works out of the box |
| Fedora 38+              | works       | ships `CONFIG_BPF_LSM=y` and `lsm=…,bpf` by default |
| Ubuntu 22.04 / 24.04    | partial     | `CONFIG_BPF_LSM=y` is set but `bpf` is not in the default `lsm=` cmdline; add it via GRUB to use `bpolicy` |
| Debian 12               | partial     | same caveat as Ubuntu — kernel supports it, cmdline does not enable it |
| NixOS                   | works       | set `boot.kernelParams = [ "lsm=landlock,lockdown,yama,integrity,apparmor,bpf" ]` (adjust to match your kernel's defaults) |
| Alpine                  | partial     | musl is fine; verify `CONFIG_BPF_LSM` on the chosen kernel flavor |
| RHEL / CentOS Stream 9  | partial     | kernel ≥ 5.14; check the active cmdline for `bpf` |
| WSL2                    | unsupported | no control over kernel cmdline; eBPF-LSM not available |
| macOS / *BSD            | unsupported | no eBPF, no `bwrap`                                   |

### Userspace dependencies

| Tool        | Needs                                  |
| ----------- | -------------------------------------- |
| `sbx`       | `bubblewrap`                           |
| `pevent`    | Python 3 stdlib only                   |
| `wchg`      | `watchman`                             |
| `procstat`  | Python 3 stdlib only (`/proc`)         |
| `txn-edit`  | Python 3 stdlib only                   |
| `tcap`      | `tmux`                                 |
| `ctrace`    | `bpftrace` + a kernel with BTF         |
| `bpolicy`   | `clang`, `libbpf`, `bpftool`; root to load |

All Python scripts target **Python 3.8+**.

To check your kernel quickly:

```sh
uname -r
zgrep CONFIG_BPF_LSM /proc/config.gz 2>/dev/null || \
  grep CONFIG_BPF_LSM /boot/config-$(uname -r)
cat /sys/kernel/security/lsm        # 'bpf' must appear for bpolicy
```

## Install

```sh
cp bin/* ~/.local/bin/
chmod +x ~/.local/bin/{sbx,pevent,wchg,procstat,txn-edit,tcap,ctrace,bpolicy}
```

`bpolicy` also needs the compiled BPF object — see [bpolicy](#bpolicy).

## Tools

### sbx — bubblewrap sandbox

Runs a command in a `bwrap` sandbox with optional cgroup limits. Three
profiles: `ephemeral` (tmpfs `$HOME`), `readonly` (host RO, `$HOME` RO, secrets
hidden), `pwd` (cwd writable, rest RO).

```sh
sbx --net -- curl -sS https://example.com
sbx --profile pwd --mem 512M --cpu 50 -- cargo test
sbx --profile readonly --timeout 30 -- python sketchy.py
```

### pevent — supervised background processes

Double-fork + `pidfd_open`, structured JSON state per run. Survives the parent
shell exiting; `wait` blocks without polling.

```sh
id=$(pevent run -- make -j8 | jq -r .id)
pevent wait "$id"          # blocks until done, prints final JSON
pevent log  "$id"          # captured stdout
pevent log  --stderr "$id"
pevent list                # all runs
pevent gc --older-than 1d
```

### wchg — filesystem-change delta

Thin agent wrapper over watchman. Each `since` returns the files that changed
since the previous `since` (or initial `watch`) — so you can ask "what did the
last command touch?" without diffing trees.

```sh
wchg watch ~/project
# ...run a build, edit files, whatever...
wchg since ~/project       # {"files":[...], "clock":"c:..."}
wchg reset ~/project       # forget history, start fresh from "now"
```

### procstat — /proc + cgroup snapshot

JSON dump of per-process telemetry (RSS, CPU, IO, threads, cgroup v2 memory.*
and cpu.stat). Built so a model can read process state without parsing `ps`.

```sh
procstat self --parent             # the shell that invoked claude
procstat snap 1234 5678 --fds
procstat watch $$ --interval 1 -n 10   # NDJSON samples
```

### txn-edit — snapshot / commit / rollback

Take a snapshot of N files, edit freely, then either `commit` (drop backups)
or `rollback` (restore). Useful before risky multi-file refactors.

```sh
id=$(txn-edit snap src/*.py | jq -r .id)
# ... edit, run tests ...
txn-edit rollback "$id"    # or: txn-edit commit "$id"
txn-edit list
```

### tcap — drive TUI apps via tmux

Spawn a detached tmux session, capture its pane, send keystrokes. Lets an
agent interact with REPLs, ncurses apps, `htop`, etc.

```sh
tcap spawn --name repl -- python
tcap send repl --enter "import sys; print(sys.version)"
tcap read repl                       # current pane
tcap read repl -S 1000               # with scrollback
tcap kill repl
```

### ctrace — eBPF session tracer

`bpftrace`-backed NDJSON log of `execve`, `openat` (writes), `unlinkat`,
`connect` for descendants of a root PID. Used to audit "did the agent
actually run what it claimed?"

```sh
sudo ctrace start --root $$        # trace this shell's tree
# ... do stuff ...
ctrace tail -n 50
ctrace query --type execve --since 60 --grep '\bcurl\b'
sudo ctrace stop
```

Requires `bpftrace`. The `bt` script lives at `share/ctrace/session.bt`;
install it to `~/.local/share/ctrace/session.bt`.

### recall — local-first agentic memory

Plain-Markdown memory store with a SQLite + FTS5 keyword index. Memories
live as files under `~/.claude/recall/memories/`, `grep`-able by the human
and queryable by the agent. Designed for me (Claude) to stop being a
goldfish across sessions.

Now lives in its own repo: [`j0yen/recall`](https://github.com/j0yen/recall).
`bootstrap/install.sh` clones and builds it; or:

```sh
git clone https://github.com/j0yen/recall.git
cd recall && cargo build --release
install -Dm755 target/release/recall ~/.local/bin/recall
recall init
recall query "bpf"
```

See the [recall README](https://github.com/j0yen/recall#readme) and the
broader memory-layer section of [`REPOS.md`](REPOS.md) (recall-doctor,
recall-io, recall-ops, recall-memory-linter, memory-reliquary) for the
full surface area.

### bpolicy — eBPF-LSM write enforcer

Compiled libbpf program that hooks `lsm/file_open` and denies writes outside
`/tmp` for a marked PID tree. Provides a hard guardrail an agent can be put
under.

```sh
cd src/bpolicy && clang -O2 -g -target bpf -c bpolicy.bpf.c -o bpolicy.bpf.o
sudo install -Dm644 bpolicy.bpf.o ~/.local/lib/bpolicy/bpolicy.bpf.o

sudo bpolicy load
sudo bpolicy enforce --pid $$      # this shell + descendants now sandboxed
# ... agent runs here ...
sudo bpolicy log -n 0              # tail denials
sudo bpolicy release --pid $$
sudo bpolicy unload
```

Build deps: `clang`, `libbpf`, `bpftool` (to regenerate `vmlinux.h` —
`bpftool btf dump file /sys/kernel/btf/vmlinux format c > vmlinux.h`).

Known quirk: the LSM hook fires after the inode is created, so denied writes
can leave a zero-byte file behind.

## License

MIT — see [LICENSE](LICENSE).

## The four primitives, mapped to tools

The premise stated at the top resolves into four capabilities, and every tool
here is one of them:

- **Run safely** — `sbx` (bubblewrap sandbox), `bpolicy` (eBPF-LSM write enforcer).
- **Observe what happened** — `pevent`, `wchg`, `procstat`, `tcap`, `ctrace`.
- **Undo** — `txn-edit` (snapshot / commit / rollback).
- **Remember** — `recall`, in its own repo: [`j0yen/recall`](https://github.com/j0yen/recall).
