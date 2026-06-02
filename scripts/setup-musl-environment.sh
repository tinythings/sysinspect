#!/usr/bin/env sh
set -eu

PAM_VER="1.6.1"
PAM_URL="https://github.com/linux-pam/linux-pam/releases/download/v${PAM_VER}/Linux-PAM-${PAM_VER}.tar.xz"
DL_DIR="./target/tmp/pam-build"
INSTALL_LOG="./target/tmp/musl-pam-install.log"

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
	target="$1"
	cc="$2"
	sysroot="$3"

	libpam="${sysroot}/lib/libpam.a"
	libpam_misc="${sysroot}/lib/libpam_misc.a"

	if [ -f "$libpam" ] && [ -f "$libpam_misc" ]; then
		echo "PAM already installed for $target — skip."
		return
	fi

	require_cmd "$cc"
	require_cmd make

	echo "Building Linux-PAM ${PAM_VER} for $target (CC=$cc) ..."
	(
		cd "${DL_DIR}/Linux-PAM-${PAM_VER}"
		CC="$cc" ./configure \
			--host="$target" \
			--prefix="$sysroot" \
			--enable-static \
			--disable-shared \
			--disable-doc \
			--disable-nls \
			--disable-selinux \
			--disable-regenerate-docu \
			>"$INSTALL_LOG" 2>&1
		make -j"$(nproc)" -C libpam >> "$INSTALL_LOG" 2>&1
		make -j"$(nproc)" -C libpam_misc >> "$INSTALL_LOG" 2>&1
		make -j"$(nproc)" -C libpamc >> "$INSTALL_LOG" 2>&1

		MUST_SUDO=
		if [ ! -w "$sysroot" ]; then
			if ! sudo -n true 2>/dev/null; then
				echo "PAM install for $target skipped — requires sudo (run manually)."
				exit 1
			fi
			MUST_SUDO=sudo
		fi
		$MUST_SUDO mkdir -p "${sysroot}/lib" "${sysroot}/include/security"
		$MUST_SUDO cp libpam/.libs/libpam.a "${sysroot}/lib/"
		$MUST_SUDO cp libpam_misc/.libs/libpam_misc.a "${sysroot}/lib/"
		$MUST_SUDO cp libpamc/.libs/libpamc.a "${sysroot}/lib/" 2>/dev/null || true
		for d in libpam libpamc libpam_misc; do
			$MUST_SUDO cp "${d}/include/security/"*.h "${sysroot}/include/security/" 2>/dev/null || true
		done
		echo "PAM installed for $target OK."
	) || true
}

require_cmd wget
require_cmd nproc
require_cmd tar

download_pam

if command -v aarch64-linux-musl-gcc >/dev/null 2>&1; then
	build_target aarch64-linux-musl \
		aarch64-linux-musl-gcc \
		/opt/aarch64-linux-musl-cross/aarch64-linux-musl
fi

if command -v x86_64-linux-musl-gcc >/dev/null 2>&1; then
	build_target x86_64-linux-musl \
		x86_64-linux-musl-gcc \
		/usr/lib/x86_64-linux-musl
fi

echo "Done."
