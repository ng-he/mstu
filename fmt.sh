#!/usr/bin/env bash
# Formats every part written in Rust.
#
#   ./fmt.sh            format in place
#   ./fmt.sh --check    say what is unformatted, change nothing
#
# A plugin may be written in a language rustfmt knows nothing about, so one
# with a fmt.sh of its own is left to it; the rest are formatted with cargo.
set -uo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"

args=()
[ "${1:-}" = "--check" ] && args=(-- --check)

unformatted=()

rust() {
    local name="$1"
    local manifest="$2"

    cargo fmt --manifest-path "$manifest" "${args[@]}" >/dev/null || unformatted+=("$name")
}

for part in mstu-sdk mstu-media mstu-engine integration; do
    [ -f "$part/Cargo.toml" ] && rust "$part" "$part/Cargo.toml"
done

for plugin in plugins-src/*/; do
    name="$(basename "$plugin")"

    if [ -x "$plugin/fmt.sh" ]; then
        "$plugin/fmt.sh" "$@" || unformatted+=("$name")
    elif [ -f "$plugin/Cargo.toml" ]; then
        rust "$name" "$plugin/Cargo.toml"
    fi
done

if [ ${#unformatted[@]} -eq 0 ]; then
    echo "formatting: clean"
    exit 0
fi

echo "unformatted: ${unformatted[*]}" >&2
echo "run ./fmt.sh to fix" >&2
exit 1
