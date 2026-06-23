#!/usr/bin/env sh
set -eu

PROFILE="release"
MUSL_TRIPLE=""

while [ $# -gt 0 ]; do
	case "$1" in
		--musl)
			MUSL_TRIPLE="$2"
			shift 2
			;;
		*)
			echo "assemble-dist.sh: unknown argument: $1" >&2
			exit 1
			;;
	esac
done

PLATFORM=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)
case "$ARCH" in
	aarch64|arm64) ARCH="arm64" ;;
esac

if [ -n "$MUSL_TRIPLE" ]; then
	PREFIX="musl"
	CARGO_TARGET="target/${MUSL_TRIPLE}/${PROFILE}"
else
	PREFIX="dyn"
	CARGO_TARGET="target/${PROFILE}"
fi

LABEL="${PREFIX}-${ARCH}-dist"
DEST="build/${PLATFORM}/${LABEL}"

echo "=== sysinspect distribution assembly ==="
echo "  platform: $PLATFORM"
echo "  arch:     $ARCH"
echo "  profile:  $PROFILE"
echo "  prefix:   $PREFIX"
echo "  dest:     $DEST"
echo ""

rm -rf "$DEST"
mkdir -p "${DEST}/bin" "${DEST}/master" "${DEST}/models" "${DEST}/modules"

echo "--- core binaries ---"
for bin in sysinspect sysmaster sysminion; do
	src="${CARGO_TARGET}/${bin}"
	if [ ! -f "$src" ]; then
		echo "FATAL: missing core binary: $src" >&2
		exit 1
	fi
	cp "$src" "${DEST}/bin/"
	chmod +x "${DEST}/bin/${bin}"
	echo "  bin/${bin}"
done

echo ""
echo "--- modules ---"
find modules -maxdepth 3 -name Cargo.toml | sort | while read toml; do
	pkg=$(awk '/^[[:space:]]*name[[:space:]]*=/ {
		gsub(/"/, "", $3); name=$3; exit
	}
	END { if (name=="") name=""; print name }' "$toml")
	[ -n "$pkg" ] || continue

	src_dir=$(dirname "$toml")
	spec="${src_dir}/src/mod_doc.yaml"
	bin="${CARGO_TARGET}/${pkg}"

	if [ ! -f "$bin" ]; then
		echo "  skip (no binary): $pkg" >&2
		continue
	fi
	if [ ! -f "$spec" ]; then
		echo "  skip (no spec):   $pkg" >&2
		continue
	fi

	mkdir -p "${DEST}/modules/${pkg}"
	cp "$bin"  "${DEST}/modules/${pkg}/${pkg}"
	cp "$spec" "${DEST}/modules/${pkg}/${pkg}.spec"
	chmod +x "${DEST}/modules/${pkg}/${pkg}"
	echo "  modules/${pkg}/"
done

echo ""
echo "--- models ---"
MODEL_SRC="examples/demos/infoping"
if [ -f "${MODEL_SRC}/model.cfg" ]; then
	mkdir -p "${DEST}/models/infoping"
	cp "${MODEL_SRC}/model.cfg" "${DEST}/models/infoping/"
	echo "  models/infoping/"
else
	echo "  skip (not found): infoping" >&2
fi

echo ""
echo "=== done: ${DEST} ==="
echo ""
find "$DEST" -type d -o -type f | sort | while read entry; do
	rel="${entry#${DEST}/}"
	if [ "$rel" = "$entry" ]; then
		continue
	fi
	if [ -d "$entry" ]; then
		echo "  ${rel}/"
	else
		echo "  ${rel}"
	fi
done
