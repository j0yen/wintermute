# dotfiles

Machine config for this laptop. Tracks `~/.claude/` settings + hook scripts
and personal scratch CLIs under `~/.local/bin/`; intentionally narrow scope
(the things that, if lost, would meaningfully break my agent setup).

## Layout

```
.
├── .claude/
│   ├── settings.json            # Claude Code global settings (hooks, permissions, theme)
│   └── scripts/
│       ├── ctrace-session-start.sh    # eBPF session tracer (start)
│       ├── ctrace-session-end.sh      # eBPF session tracer (stop)
│       ├── summarize-ctrace-session.sh
│       ├── recall-session-start.sh    # SessionStart hook — emit relevant memories
│       ├── scratch-tools-start.sh     # SessionStart hook — surface stack/letter at session top
│       └── ousia-guard-pretool.sh     # PreToolUse hook — ousia-guard ethical gate (allow/ask/deny)
├── .local/
│   ├── bin/                     # personal scratch CLIs (symlinked into ~/.local/bin/)
│   │   ├── napkin               # within-session scratchpad (append-only)
│   │   ├── stack                # LIFO of pending intentions across sessions
│   │   ├── bookmark             # file:line markers; resume where you stopped
│   │   └── letter               # letters from past-Claude to future-Claude
│   └── share/ousia/
│       └── participants.json    # ousia-guard operator registry (regex → participant/edge rules)
├── install.sh                   # symlink installer with backups
└── README.md
```

`~/.claude/settings.local.json` is intentionally **not** tracked
(machine-local permission allowlist); it's listed in `.gitignore` and
skipped by `install.sh`.

The `claude-self` cluster (`CLAUDE_SELF.md`, `CLAUDE_SELF.default.md`,
`claude-self-start.sh` hook, and the `claude-self` CLI) lives elsewhere
until the autobuilder pipeline wires it. `install.sh`'s `skip_pattern`
defensively excludes those paths if they ever appear here.

## Install on a fresh machine

```sh
git clone git@github.com:j0yen/wintermute.git ~/wintermute
cd ~/wintermute/dotfiles
./install.sh --dry-run     # preview
./install.sh               # symlink into ~/
```

`install.sh` is idempotent. If the target file already exists and is *not*
a symlink to the right place, it is renamed to `<name>.bak.<UTC timestamp>`
before the symlink is created.

## Why not chezmoi / yadm / stow?

A handful of tracked files. The complexity budget for a personal dotfiles
subtree should be near zero — one shell script is fewer moving parts than
any of the alternatives.

## Related

- `~/wintermute/recall/hooks/session-start.sh` is the *canonical* source for
  the SessionStart hook script. The copy here in `.claude/scripts/` is the
  installed runtime artifact, kept in sync by hand.
