#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
PLUGIN_SRC="$ROOT/mstu-plugins-src"
PLUGIN_DST="$ROOT/plugins"

mkdir -p "$PLUGIN_DST"

for dir in "$PLUGIN_SRC"/*; do
    [ -d "$dir" ] || continue

    echo "==> Building $(basename "$dir")"

    cargo build \
        --manifest-path "$dir/Cargo.toml" \
        --release

    find "target/release" \
        -maxdepth 1 \
        \( -name "*.so" -o -name "*.dll" -o -name "*.dylib" \) \
        -exec cp {} "$PLUGIN_DST" \;

    # ui() returns a folder name resolved next to the library.
    if [ -d "$dir/ui" ]; then
        rm -rf "$PLUGIN_DST/$(basename "$dir")-ui"
        cp -r "$dir/ui" "$PLUGIN_DST/$(basename "$dir")-ui"
    fi
done

echo "Done."