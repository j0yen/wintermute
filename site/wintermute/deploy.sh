#!/usr/bin/env bash
# Deploy the mcphost.dev/wintermute static page from this repo to mcphost-1.
#   deploy.sh [--dry-run]   (default) show what would change, change nothing
#   deploy.sh --apply       back up the live dir, rsync, verify
# live.json is written by carbon's wintermute-live-export.timer and is never touched here.
set -euo pipefail

HOST=mcphost-1
REMOTE_DIR=/var/www/mcphost/wintermute
BACKUP_ROOT=/var/backups/mcphost-www
URL=https://mcphost.dev/wintermute

mode=dry-run
case "${1:-}" in
  ""|--dry-run) mode=dry-run ;;
  --apply) mode=apply ;;
  *) echo "usage: $0 [--dry-run|--apply]" >&2; exit 2 ;;
esac
[ $# -le 1 ] || { echo "usage: $0 [--dry-run|--apply]" >&2; exit 2; }

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo=$(git -C "$here" rev-parse --show-toplevel)
src="$here/"

# Guard: clean tree, HEAD on origin/master. Enforced for --apply; a dry run only warns, so the
# PR branch can be dry-run against prod before it merges.
guard_fail() {
  if [ "$mode" = apply ]; then echo "refusing: $1" >&2; exit 1; fi
  echo "warning (would refuse --apply): $1" >&2
}
[ -z "$(git -C "$repo" status --porcelain --untracked-files=all)" ] || guard_fail "working tree is not clean"
git -C "$repo" fetch -q origin master
git -C "$repo" merge-base --is-ancestor HEAD origin/master ||
  guard_fail "HEAD $(git -C "$repo" rev-parse --short HEAD) is not on origin/master (merge the PR first)"
echo "HEAD $(git -C "$repo" rev-parse --short HEAD); mode=$mode"

# Ownership/mode of the live files. Same ssh user as the export script's rsync (root), so new files
# land root-owned; -rlpt keeps modes and never chowns.
stat_live() { ssh -o BatchMode=yes "$HOST" "stat -c '%U:%G %a %n' $REMOTE_DIR $REMOTE_DIR/brand $REMOTE_DIR/index.html $REMOTE_DIR/brand/*.svg"; }
echo "== live owner/mode before"; stat_live

rsync_args=(-rlpt --checksum --itemize-changes '--chmod=F644,D755' --exclude live.json --exclude '.*'
            -e 'ssh -o BatchMode=yes')

if [ "$mode" = dry-run ]; then
  echo "== rsync dry run (no --delete; live.json excluded)"
  out=$(rsync -n "${rsync_args[@]}" "$src" "$HOST:$REMOTE_DIR/" --exclude deploy.sh --exclude README.md)
  [ -n "$out" ] && printf '%s\n' "$out"
  # itemize lines starting with "." mean no data transfer (mtime-only touch); these are not content changes
  c=$(printf '%s' "$out" | grep -c '^[^.]' || true)
  t=$(printf '%s' "$out" | grep -c '^\.' || true)
  echo "dry-run: $c file change(s), $t mtime-only; nothing written"
  exit 0
fi

ts=$(date -u +%Y%m%dT%H%M%SZ)
backup="$BACKUP_ROOT/wintermute-$ts"
echo "== backup -> $HOST:$backup"
ssh -o BatchMode=yes "$HOST" "mkdir -p '$backup' && cp -a $REMOTE_DIR/. '$backup/' && rm -f '$backup/live.json'"

echo "== rsync apply"
rsync "${rsync_args[@]}" "$src" "$HOST:$REMOTE_DIR/" --exclude deploy.sh --exclude README.md

echo "== live owner/mode after"; stat_live

code=$(curl -sS -o /dev/null -w '%{http_code}' "$URL")
[ "$code" = 200 ] || { echo "FAIL: $URL returned $code" >&2; echo "restore: ssh $HOST \"rsync -a --exclude live.json $backup/ $REMOTE_DIR/\"" >&2; exit 1; }
want=$(sha256sum "$here/index.html" | cut -d' ' -f1)
got=$(ssh -o BatchMode=yes "$HOST" "sha256sum $REMOTE_DIR/index.html" | cut -d' ' -f1)
[ "$want" = "$got" ] || { echo "FAIL: live index.html sha256 $got != repo $want" >&2; echo "restore: ssh $HOST \"rsync -a --exclude live.json $backup/ $REMOTE_DIR/\"" >&2; exit 1; }
echo "verified: HTTP 200, index.html sha256 matches repo"
echo "backup:  $HOST:$backup"
echo "restore: ssh $HOST \"rsync -a --exclude live.json $backup/ $REMOTE_DIR/\""
