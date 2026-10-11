#!/usr/bin/env bash
# Runs every test in the project: each part on its own, then the parts
# together.
#
#   ./test.sh           everything
#   ./test.sh --unit    only the parts, needing neither an engine nor plugins
#
# The parts written here are tested with cargo. A plugin is tested by its own
# test.sh, since it may be written in a language cargo knows nothing about;
# all this asks of it is an exit status.
set -uo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"

only_unit=false
[ "${1:-}" = "--unit" ] && only_unit=true

failed=()
skipped=()

run() {
    local name="$1"
    shift

    echo
    echo "==> $name"

    if "$@"; then
        return 0
    fi

    failed+=("$name")
}

for part in mstu-sdk mstu-media mstu-engine; do
    [ -f "$part/Cargo.toml" ] || continue

    run "$part" cargo test --manifest-path "$part/Cargo.toml"
done

for plugin in plugins-src/*/; do
    name="$(basename "$plugin")"

    if [ -x "$plugin/test.sh" ]; then
        run "$name" "$plugin/test.sh"
    elif [ -f "$plugin/Cargo.toml" ]; then
        # No script of its own, but cargo knows what to do with it.
        run "$name" cargo test --manifest-path "$plugin/Cargo.toml"
    else
        skipped+=("$name")
    fi
done

if ! $only_unit; then
    # The integration tests need the engine built and the plugins collected.
    run "building the engine" cargo build --release --manifest-path mstu-engine/Cargo.toml
    run "collecting plugins" ./collect.sh --build
    run "integration" cargo test --manifest-path integration/Cargo.toml
fi

echo

for name in "${skipped[@]:-}"; do
    [ -n "$name" ] && echo "no tests for $name: it has neither test.sh nor a cargo manifest"
done

if [ ${#failed[@]} -eq 0 ]; then
    echo "all green"
    exit 0
fi

echo "failed: ${failed[*]}"
exit 1
