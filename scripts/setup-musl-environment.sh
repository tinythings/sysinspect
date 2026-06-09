#!/usr/bin/env sh
set -eu

PAM_VER="1.6.1"
PAM_URL="https://github.com/linux-pam/linux-pam/releases/download/v${PAM_VER}/Linux-PAM-${PAM_VER}.tar.xz"
ROOT_DIR=$(pwd)
TMP_DIR="${ROOT_DIR}/target/tmp"
DL_DIR="${TMP_DIR}/pam-build"
INSTALL_LOG="${TMP_DIR}/musl-pam-install.log"

require_cmd() {
	command -v "$1" >/dev/null 2>&1 || { echo "Missing $1. Install it first." >&2; exit 1; }
}

download_pam() {
	if [ -f "$DL_DIR/pam-${PAM_VER}.done" ]; then
		echo "Linux-PAM ${PAM_VER} source already present."
		return
	fi
	rm -rf "${DL_DIR}/Linux-PAM-${PAM_VER}"
	mkdir -p "$DL_DIR"
	echo "Downloading Linux-PAM ${PAM_VER} ..."
	wget -q "$PAM_URL" -O "$DL_DIR/pam.tar.xz"
	tar xf "$DL_DIR/pam.tar.xz" -C "$DL_DIR"
	rm "$DL_DIR/pam.tar.xz"
	touch "$DL_DIR/pam-${PAM_VER}.done"
}

build_target() {
	pam_target="$1"
	cc="$2"
	rust_target="$3"
	prefix="${ROOT_DIR}/target/musl/${rust_target}"
	libdir="${prefix}/lib"
	includedir="${prefix}/include"
	build_marker="${prefix}/.sysinspect-pam-pic"

	libpam="${libdir}/libpam.a"
	libpam_misc="${libdir}/libpam_misc.a"

	if [ -f "$libpam" ] && [ -f "$libpam_misc" ] && [ -f "$build_marker" ]; then
		echo "PAM already installed for $rust_target — skip."
		return
	fi

	require_cmd "$cc"
	require_cmd make

			echo "Building Linux-PAM ${PAM_VER} for $rust_target (CC=$cc) ..."
			echo "Installing static PAM into $libdir"
		(
			cd "${DL_DIR}/Linux-PAM-${PAM_VER}"
			make distclean >/dev/null 2>&1 || true
			CC="$cc" CFLAGS="-fPIC" ./configure \
				--host="$pam_target" \
				--prefix="$prefix" \
				--enable-static \
			--disable-shared \
			--disable-doc \
			--disable-nls \
			--disable-selinux \
			--disable-regenerate-docu \
			>"$INSTALL_LOG" 2>&1
			make -j"$(nproc)" -C libpam_internal >> "$INSTALL_LOG" 2>&1
			make -j"$(nproc)" -C libpam >> "$INSTALL_LOG" 2>&1
			make -j"$(nproc)" -C libpamc >> "$INSTALL_LOG" 2>&1
			make -j"$(nproc)" -C libpam_misc >> "$INSTALL_LOG" 2>&1

			mkdir -p "$libdir" "$includedir/security"
			cp libpam/.libs/libpam.a "$libdir/"
			cp libpam_misc/.libs/libpam_misc.a "$libdir/"
			cp libpamc/.libs/libpamc.a "$libdir/" 2>/dev/null || true
		for d in libpam libpamc libpam_misc; do
			cp "${d}/include/security/"*.h "$includedir/security/" 2>/dev/null || true
		done
			: > "$build_marker"
			echo "PAM installed for $rust_target OK."
		)
}

require_cmd wget
require_cmd nproc
require_cmd tar
mkdir -p "$TMP_DIR"

download_pam

if command -v aarch64-linux-musl-gcc >/dev/null 2>&1; then
	build_target aarch64-linux-musl \
		aarch64-linux-musl-gcc \
		aarch64-unknown-linux-musl
fi

if command -v x86_64-linux-musl-gcc >/dev/null 2>&1; then
	build_target x86_64-linux-musl \
		x86_64-linux-musl-gcc \
		x86_64-unknown-linux-musl
fi

echo "Done."
