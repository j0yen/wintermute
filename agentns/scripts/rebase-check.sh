#!/usr/bin/env bash
# Quick check: do our patches still apply against the current kernel?
# Used by /self-review and CI to catch drift before it bites.
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
KVER="${KVER:-$(uname -r | cut -d- -f1)}"
WORKDIR="${WORKDIR:-/tmp/agentns-rebase-check-$KVER}"

if [[ ! -d "$WORKDIR/linux-$KVER" ]]; then
	echo "[rebase-check] no extracted tree at $WORKDIR/linux-$KVER"
	echo "[rebase-check] run scripts/build.sh first (BUILD_KERNEL=0)"
	exit 2
fi

cd "$WORKDIR/linux-$KVER"
fail=0
for p in "$HERE"/patches/000[1-9]-*.patch; do
	if patch -p1 --dry-run --silent < "$p" >/dev/null 2>&1; then
		echo "ok      $(basename "$p")"
	else
		echo "FAIL    $(basename "$p")"
		fail=1
	fi
done
exit $fail
