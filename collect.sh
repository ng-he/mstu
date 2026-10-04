#!/usr/bin/env bash
# Collects what the plugins have built into plugins/, where the engine looks.
#
# A plugin is built by its own repository, in whatever language it is written
# in. This looks for the result in two places: dist/, which any plugin can put
# its library and ui folder in, and target/release, where cargo leaves it.
#
#   ./collect.sh            collect what is already built
#   ./collect.sh --build    run each plugin's build.sh first
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
SRC="$ROOT/plugins-src"
DEST="${MSTU_PLUGIN_DIR:-$ROOT/plugins}"

build=false
[ "${1:-}" = "--build" ] && build=true

mkdir -p "$DEST"

found=0

for dir in "$SRC"/*/; do
    [ -d "$dir" ] || continue

    name="$(basename "$dir")"

    if $build && [ -x "$dir/build.sh" ]; then
        echo "==> building $name"
        "$dir/build.sh"
    fi

    # The library, named as the plugin is but with underscores.
    lib="lib${name//-/_}"
    taken=false

    for place in "$dir/dist" "$dir/target/release"; do
        for ext in so dll dylib; do
            if [ -f "$place/$lib.$ext" ]; then
                cp "$place/$lib.$ext" "$DEST/"
                echo "    $lib.$ext"
                taken=true
                break 2
            fi
        done
    done

    if ! $taken; then
        echo "    $name: nothing built yet" >&2
        continue
    fi

    # ui() returns a folder name resolved next to the library.
    for place in "$dir/dist/ui" "$dir/ui"; do
        if [ -d "$place" ]; then
            rm -rf "${DEST:?}/$name-ui"
            cp -r "$place" "$DEST/$name-ui"
            echo "    $name-ui"
            break
        fi
    done

    found=$((found + 1))
done

echo "collected $found plugin(s) into $DEST"
