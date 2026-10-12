#!/bin/sh
# The Linux side of play.ps1: build, then run. A failed build launches nothing.
# Usage: tools/run/play.sh [--release] [--no-build] <game dir> [l2-game flags]
# Target dir: $CARGO_TARGET_DIR, else ~/.cache/lords2-target, a symlink to F: so nothing
# builds in /home and the Windows target/ is left alone (docs/environment.md, "On Linux").
set -e
root=$(cd "$(dirname "$0")/../.." && pwd)
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/lords2-target}"
profile=debug; flag=; build=1
while :; do
  case "$1" in
    --release) profile=release; flag=--release; shift ;;
    --no-build) build=; shift ;;
    *) break ;;
  esac
done
if [ -n "$build" ]; then
  (cd "$root" && cargo build -p l2-game $flag) || { echo "build failed - not launching." >&2; exit 1; }
fi
exec "$CARGO_TARGET_DIR/$profile/l2-game" "$@"
