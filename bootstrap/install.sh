#!/usr/bin/env bash
# wintermute bootstrap installer
#
# Clones each j0yen ecosystem project into $WINTERMUTE_ROOT (default
# $HOME/wintermute/<name>), builds the ones with a Cargo.toml, symlinks
# release binaries into $LOCAL_BIN_DIR (default $HOME/.local/bin), and
# (optionally) symlinks the Claude Code SessionStart hook scripts shipped
# in dotfiles/ into $HOME/.claude/scripts/.
#
# Safe to rerun: skips clones for repos already present, skips builds
# for crates whose binary mtime is newer than its Cargo.toml. Never
# overwrites an existing symlink or file without --force.
#
# Usage:
#   bootstrap/install.sh                  # full install
#   bootstrap/install.sh --no-hooks       # skip ~/.claude wiring
#   bootstrap/install.sh --no-build       # clone only
#   bootstrap/install.sh --dry-run        # show plan, do nothing
#   bootstrap/install.sh --force          # overwrite existing symlinks

set -euo pipefail

WINTERMUTE_ROOT="${WINTERMUTE_ROOT:-$HOME/wintermute}"
LOCAL_BIN_DIR="${LOCAL_BIN_DIR:-$HOME/.local/bin}"
CLAUDE_DIR="${CLAUDE_DIR:-$HOME/.claude}"
GH_USER="${GH_USER:-j0yen}"

no_hooks=0
no_build=0
dry_run=0
force=0
for arg in "$@"; do
    case "$arg" in
        --no-hooks) no_hooks=1 ;;
        --no-build) no_build=1 ;;
        --dry-run)  dry_run=1 ;;
        --force)    force=1 ;;
        -h|--help)
            sed -n '3,18p' "$0" | sed 's/^# \?//'
            exit 0
            ;;
        *)
            echo "unknown flag: $arg" >&2
            exit 2
            ;;
    esac
done

# Each entry: "<repo-name> <binary-name-or-->"
# `-` means: no binary to symlink (library-only crate or non-cargo).
repos=(
    "agent-pipe apipe"
    "agentsh -"
    "agorabus agorabus"
    "ambient ambient"
    "autobuilder autobuilder"
    "autobuilder-metric-harness -"
    "baton baton"
    "claude-self claude-self"
    "confidant confidant"
    "conversations-zine zine"
    "daily-receipt -"
    "episodic-observer episode"
    "fsstory -"
    "learning-db -"
    "letters-we-never-sent letter-curate"
    "mcp-autotuner -"
    "memory-reliquary reliquary"
    "mirror mirror"
    "morsel-bake morsel-bake"
    "provfs provfs"
    "recall recall"
    "recall-doctor recall-doctor"
    "recall-io recall-io"
    "recall-memory-linter recall-lint"
    "recall-ops recall-ops"
    "repo-as-landscape repo-as-landscape"
    "self-portrait self-portrait"
    "session-index transcript"
    "session-trace-receipt -"
    "skill-manifest skill"
    "skill-telemetry spool"
    "tide-chart -"
)

# Per-repo extra wiring after clone (idempotent). Each entry is "<repo>:<script>"
# where <script> is a path inside the cloned repo to run from the repo root.
# Use for projects that ship their own install.sh (shell plugins, etc.) that the
# bootstrap can't infer from a Cargo binary alone.
extra_install=(
    "agentsh:install.sh"
)

step() { printf '\n== %s ==\n' "$*"; }
note() { printf '  %s\n' "$*"; }
do_or_say() {
    if [ "$dry_run" = "1" ]; then
        printf '  [dry-run] %s\n' "$*"
    else
        eval "$*"
    fi
}

step "Preparing directories"
do_or_say "mkdir -p '$WINTERMUTE_ROOT' '$LOCAL_BIN_DIR'"

# Confirm required tools.
for tool in git; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "missing required tool: $tool" >&2
        exit 1
    fi
done
if [ "$no_build" = "0" ]; then
    if ! command -v cargo >/dev/null 2>&1; then
        note "cargo not on PATH (rustup/cargo); skipping builds (use --no-build to silence)"
        no_build=1
    fi
fi

step "Cloning repos"
for entry in "${repos[@]}"; do
    name=$(echo "$entry" | awk '{print $1}')
    target="$WINTERMUTE_ROOT/$name"
    if [ -d "$target/.git" ]; then
        note "skip $name (already cloned)"
        continue
    fi
    note "clone $GH_USER/$name -> $target"
    do_or_say "git clone --depth=200 'https://github.com/$GH_USER/$name.git' '$target'"
done

if [ "$no_build" = "0" ]; then
    step "Building Rust crates"
    for entry in "${repos[@]}"; do
        name=$(echo "$entry" | awk '{print $1}')
        bin=$(echo "$entry" | awk '{print $2}')
        target="$WINTERMUTE_ROOT/$name"
        [ -f "$target/Cargo.toml" ] || continue
        if [ "$bin" != "-" ] && [ -x "$target/target/release/$bin" ] && [ "$target/target/release/$bin" -nt "$target/Cargo.toml" ]; then
            note "skip $name (binary up to date)"
            continue
        fi
        note "cargo build --release in $name"
        do_or_say "(cd '$target' && cargo build --release --quiet)"
    done

    step "Symlinking release binaries into $LOCAL_BIN_DIR"
    for entry in "${repos[@]}"; do
        name=$(echo "$entry" | awk '{print $1}')
        bin=$(echo "$entry" | awk '{print $2}')
        [ "$bin" = "-" ] && continue
        src="$WINTERMUTE_ROOT/$name/target/release/$bin"
        dst="$LOCAL_BIN_DIR/$bin"
        if [ ! -f "$src" ]; then
            note "skip $bin (binary not built)"
            continue
        fi
        if [ -e "$dst" ] && [ "$force" = "0" ] && [ ! -L "$dst" ]; then
            note "skip $bin (existing non-symlink at $dst)"
            continue
        fi
        do_or_say "ln -sf '$src' '$dst'"
        note "$bin -> $src"
    done
fi

step "Running per-repo install.sh scripts"
for entry in "${extra_install[@]}"; do
    repo="${entry%%:*}"
    script="${entry#*:}"
    target="$WINTERMUTE_ROOT/$repo"
    if [ ! -d "$target" ]; then
        note "skip $repo (not cloned)"
        continue
    fi
    if [ ! -x "$target/$script" ]; then
        note "skip $repo (no executable $script)"
        continue
    fi
    note "run $repo/$script"
    do_or_say "(cd '$target' && './$script')"
done

if [ "$no_hooks" = "0" ]; then
    step "Wiring Claude Code dotfiles into $CLAUDE_DIR"
    if [ ! -d "$CLAUDE_DIR" ]; then
        note "skip (no $CLAUDE_DIR — install Claude Code first)"
    else
        dotroot="$(cd "$(dirname "$0")/.." && pwd)/dotfiles/.claude"
        if [ ! -d "$dotroot" ]; then
            note "skip (no dotfiles/.claude in this checkout)"
        else
            # symlink scripts/
            do_or_say "mkdir -p '$CLAUDE_DIR/scripts'"
            for f in "$dotroot/scripts"/*.sh; do
                [ -f "$f" ] || continue
                name=$(basename "$f")
                dst="$CLAUDE_DIR/scripts/$name"
                if [ -e "$dst" ] && [ "$force" = "0" ] && [ ! -L "$dst" ]; then
                    note "skip $name (existing non-symlink at $dst)"
                    continue
                fi
                do_or_say "ln -sf '$f' '$dst'"
                note "scripts/$name -> $f"
            done
            note "settings.json wiring is not automatic — review CLAUDE.md"
        fi
    fi
fi

step "Done"
note "installed $(printf '%s\n' "${repos[@]}" | wc -l) repo entries"
note "binaries: $LOCAL_BIN_DIR"
note "ensure \$LOCAL_BIN_DIR is on \$PATH"
