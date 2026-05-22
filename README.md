# wintermute

Eight local CLI tools that make a coding agent (Claude Code, in practice) less
timid: a sandbox, a supervised background runner, FS/proc/tmux observers, a
transactional editor, and two eBPF tools that audit and constrain what gets
done on the host.

All tools are single-file Python 3 scripts (no deps beyond the stdlib + the
platform binary each one shells out to: `bwrap`, `watchman`, `tmux`,
`bpftrace`, `bpftool`).

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

## Why this exists

Agent-friendly infrastructure. The premise: an agent is less likely to make
timid, low-leverage choices if it has cheap, structured ways to (a) run things
safely, (b) observe what happened, and (c) undo. `sbx`/`bpolicy` are (a),
`pevent`/`wchg`/`procstat`/`tcap`/`ctrace` are (b), `txn-edit` is (c).
