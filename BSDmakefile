.PHONY: setup build devel all all-devel modules modules-dev modules-dist-devel modules-refresh-devel \
	modules-refresh clean check fix stats man test test-core test-modules test-sensors test-integration tar dev-tls \
	musl-aarch64-dev musl-aarch64 musl-x86_64-dev musl-x86_64

GNU_MAKE?=	gmake
FREEBSD_SETUP_PACKAGES=	gmake rust pkgconf llvm protobuf libffi libsodium openssl jq

setup:
	@command -v ${GNU_MAKE} >/dev/null 2>&1 || { \
		command -v pkg >/dev/null 2>&1 || { \
			echo "FreeBSD bootstrap requires pkg to install ${GNU_MAKE} and Rust."; \
			exit 1; \
		}; \
		echo "Installing FreeBSD bootstrap packages: ${FREEBSD_SETUP_PACKAGES}"; \
		$$(command -v sudo >/dev/null 2>&1 && printf 'sudo ' || true)pkg install -y ${FREEBSD_SETUP_PACKAGES}; \
	}; \
	cd ${.CURDIR} && exec ${GNU_MAKE} setup

build devel all all-devel modules modules-dev modules-dist-devel modules-refresh-devel modules-refresh clean check fix stats man test test-core test-modules test-sensors test-integration tar dev-tls musl-aarch64-dev musl-aarch64 musl-x86_64-dev musl-x86_64:
	@command -v ${GNU_MAKE} >/dev/null 2>&1 || { \
		echo "Use 'make setup' first. FreeBSD needs ${GNU_MAKE} for this project."; \
		exit 1; \
	}; \
	cd ${.CURDIR} && exec ${GNU_MAKE} ${.TARGET}
