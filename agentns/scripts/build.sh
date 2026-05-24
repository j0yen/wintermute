#!/usr/bin/env bash
# Vendor-patch builder for the wintermute Agent Namespace.
#
# Steps:
#   1. Fetch matching kernel sources for the running kernel (or KVER override).
#   2. Apply our patch series (numbered 0001..0009) on top.
#   3. Copy new files (kernel/, include/) into the tree (the 0010 placeholder).
#   4. Build the kernel package.
#
# Defaults assume Arch Linux + pkgctl.  Override KSRC_FETCH for other distros.
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
KVER="${KVER:-$(uname -r | cut -d- -f1)}"
WORKDIR="${WORKDIR:-$HERE/build-$KVER}"
KSRC_FETCH="${KSRC_FETCH:-asp}"  # asp | pkgctl | tarball

log() { printf '[agentns/build] %s\n' "$*"; }
die() { printf '[agentns/build] ERROR: %s\n' "$*" >&2; exit 1; }

ensure_sources() {
	if [[ -d "$WORKDIR/linux-$KVER" ]]; then
		log "sources already extracted at $WORKDIR/linux-$KVER"
		return
	fi
	mkdir -p "$WORKDIR"
	pushd "$WORKDIR" >/dev/null

	case "$KSRC_FETCH" in
	asp)
		command -v asp >/dev/null || die "asp not installed; pacman -S asp"
		asp export linux
		cd linux && makepkg --nobuild --noextract --skippgpcheck
		;;
	pkgctl)
		command -v pkgctl >/dev/null || die "pkgctl not installed"
		pkgctl repo clone --protocol=https linux
		cd linux && makepkg --nobuild --noextract --skippgpcheck
		;;
	tarball)
		: "${KORG_TARBALL:?set KORG_TARBALL=https://cdn.kernel.org/.../linux-$KVER.tar.xz}"
		curl -fLO "$KORG_TARBALL"
		tar xf "linux-$KVER.tar.xz"
		;;
	*)
		die "unknown KSRC_FETCH=$KSRC_FETCH"
		;;
	esac
	popd >/dev/null
}

src_dir() {
	# Arch's PKGBUILD lays the tree at src/linux-$KVER; tarball form puts it at the top level.
	if [[ -d "$WORKDIR/linux/src/linux-$KVER" ]]; then
		echo "$WORKDIR/linux/src/linux-$KVER"
	elif [[ -d "$WORKDIR/linux-$KVER" ]]; then
		echo "$WORKDIR/linux-$KVER"
	else
		die "cannot locate kernel source tree under $WORKDIR"
	fi
}

copy_new_files() {
	local src="$1"
	log "copying new files into $src"
	install -Dm644 "$HERE/kernel/agent_namespaces.c"             "$src/kernel/agent_namespaces.c"
	install -Dm644 "$HERE/include/linux/agent_namespaces.h"      "$src/include/linux/agent_namespaces.h"
	install -Dm644 "$HERE/include/uapi/linux/agent_namespaces.h" "$src/include/uapi/linux/agent_namespaces.h"
	install -Dm644 "$HERE/include/trace/events/agent_ns.h"       "$src/include/trace/events/agent_ns.h"
}

apply_patches() {
	local src="$1"
	pushd "$src" >/dev/null
	for p in "$HERE"/patches/0001-*.patch \
		 "$HERE"/patches/0002-*.patch \
		 "$HERE"/patches/0003-*.patch \
		 "$HERE"/patches/0004-*.patch \
		 "$HERE"/patches/0005-*.patch \
		 "$HERE"/patches/0006-*.patch \
		 "$HERE"/patches/0007-*.patch \
		 "$HERE"/patches/0008-*.patch \
		 "$HERE"/patches/0009-*.patch
	do
		log "applying $(basename "$p")"
		if ! patch -p1 --dry-run < "$p" >/dev/null 2>&1; then
			log "patch does not apply cleanly; rebase manually: $p"
			patch -p1 --dry-run < "$p" || true
			die "rebase needed against linux $KVER"
		fi
		patch -p1 < "$p"
	done
	popd >/dev/null
}

configure_and_build() {
	local src="$1"
	pushd "$src" >/dev/null
	scripts/config --enable AGENT_NS || die "scripts/config failed"
	# touch .config so olddefconfig picks up our new bool
	yes "" | make ARCH=x86_64 olddefconfig
	make ARCH=x86_64 -j"$(nproc)"
	popd >/dev/null
}

main() {
	log "target kernel = $KVER"
	log "workdir       = $WORKDIR"
	ensure_sources
	local src; src="$(src_dir)"
	copy_new_files "$src"
	apply_patches  "$src"
	if [[ "${BUILD_KERNEL:-1}" = "1" ]]; then
		configure_and_build "$src"
		log "kernel built; vmlinux at $src/vmlinux"
	else
		log "BUILD_KERNEL=0 — patches applied, build skipped"
	fi
}

main "$@"
