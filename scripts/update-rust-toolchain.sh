#!/usr/bin/bash
set -e

LATEST=$(curl -s https://static.rust-lang.org/dist/channel-rust-stable.toml | grep "^version = \"1\." | sort | uniq | tail -1 | sed -nE 's/version = "([^ ]+).*/\1/p')

if [ -z "$LATEST" ]; then
    echo "Could not fetch the latest Rust version"
    exit 1
fi
echo "Latest Rust stable version: $LATEST"

# Write the new version to rust-toolchain.toml.
cat <<EOF > rust-toolchain.toml
[toolchain]
channel = "$LATEST"
EOF

# Check if the file has changed (this assumes you are in a git repository).
if [ -n "$(git status --porcelain rust-toolchain.toml)" ]; then
    echo "rust-toolchain.toml updated. Committing changes..."
    git add rust-toolchain.toml
    git commit -m "Update rust-toolchain.toml to Rust $LATEST"
    git push
else
    echo "rust-toolchain.toml is already up-to-date."
fi
