#!/usr/bin/env bash
# Builds everything that runs into build/: the engine, and the plugins beside
# it.
#
#   ./build.sh               build into build/
#   ./build.sh --clean       throw the last one away first
#   ./build.sh --with-app    package the desktop app in as well
#
# The engine is built here; every plugin is built by its own build.sh, in
# whatever language it is written in, and collected the same way collect.sh
# collects into plugins/ for development. The app is packaged by electron
# builder, which takes a minute and makes a hundred megabytes, so it is asked
# for rather than assumed.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"

OUT="$ROOT/build"
app=false

for arg in "$@"; do
    case "$arg" in
        --clean) rm -rf "$OUT" ;;
        --with-app) app=true ;;
        *)
            echo "no such option: $arg" >&2
            exit 1
            ;;
    esac
done

mkdir -p "$OUT"

echo "==> engine"
cargo build --release --manifest-path mstu-engine/Cargo.toml

# .exe where there is one, and nothing where there is not.
for name in mstu-engine mstu-engine.exe; do
    built="mstu-engine/target/release/$name"

    [ -f "$built" ] && cp "$built" "$OUT/$name"
done

echo "==> plugins"
MSTU_PLUGIN_DIR="$OUT/plugins" ./collect.sh --build

# The engine looks for plugins from the working directory upwards, which is
# not where a build is run from: this says where they are.
cat > "$OUT/serve.sh" <<'LAUNCH'
#!/usr/bin/env bash
# Serves the desktop app from this build.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"

export MSTU_PLUGIN_DIR="${MSTU_PLUGIN_DIR:-$HERE/plugins}"

exec "$HERE/mstu-engine" --serve "$@"
LAUNCH

chmod +x "$OUT/serve.sh"

if $app; then
    echo "==> desktop app"
    npm --prefix mstu-desktop run package

    # Whatever this platform makes: an AppImage here, a dmg or an installer
    # elsewhere.
    find mstu-desktop/dist -maxdepth 1 -type f \
        \( -name '*.AppImage' -o -name '*.deb' -o -name '*.dmg' -o -name '*.exe' \) \
        -exec cp {} "$OUT/" \;
fi

# Which commit of which part this is, since nine repositories make a build.
{
    echo "built $(date -u '+%Y-%m-%d %H:%M:%SZ')"
    echo

    for part in . mstu-sdk mstu-media mstu-engine mstu-desktop plugins-src/*/; do
        [ -d "$part/.git" ] || [ -f "$part/.git" ] || continue

        name="$part"
        [ "$part" = "." ] && name="mstu"

        printf '%-36s %s\n' "${name%/}" "$(git -C "$part" rev-parse --short HEAD 2>/dev/null)"
    done
} > "$OUT/build.txt"

echo
libraries=$(ls "$OUT/plugins"/*.so "$OUT/plugins"/*.dll "$OUT/plugins"/*.dylib 2>/dev/null | wc -l)

echo "build/ holds the engine, $libraries plugin librarie(s) and serve.sh"
$app && echo "and the app: $(cd "$OUT" && ls ./*.AppImage ./*.deb ./*.dmg ./*.exe 2>/dev/null | tr '\n' ' ')"
