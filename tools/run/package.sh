#!/bin/sh
# package.sh - the Linux half of package.ps1: build a release and tar it for players.
#
#   tools/run/package.sh [--no-build]        # -> dist/open-lords2-<build>-linux.tar.gz
#
# The archive holds exactly the files listed between `files:begin` and
# `files:end`, in package.ps1's own `'src' = 'name'` form so that
# crates/l2-testkit/tests/package_contents.rs checks both lists (CLAUDE.md
# rule 1: no game asset ever goes in it). Target dir: $CARGO_TARGET_DIR, else
# target/ (docs/environment.md "On Linux" says why a Linux build sets it).
set -e
root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root"
target=${CARGO_TARGET_DIR:-$root/target}
if [ "$1" != "--no-build" ]; then
  cargo build --release -p l2-game
fi
exe="$target/release/l2-game"

# Source path -> name in the archive.
files='
  # files:begin
  "$exe" = "l2-game"
  "LICENSE" = "LICENSE.txt"
  "NOTICE" = "NOTICE.txt"
  "tools/run/README-player.txt" = "README.txt"
  # files:end
'

build=$(git rev-parse --short HEAD)
stage="$root/dist/open-lords2-$build-linux"
rm -rf "$stage"; mkdir -p "$stage"
printf '%s\n' "$files" | sed -n 's/^ *"\([^"]*\)" = "\([^"]*\)".*/\1 \2/p' | while read -r src name; do
  [ "$src" = '$exe' ] && src=$exe
  [ -f "$src" ] || { echo "missing: $src" >&2; exit 1; }
  cp "$src" "$stage/$name"
done
chmod +x "$stage/l2-game"
tar -C "$root/dist" -czf "$stage.tar.gz" "$(basename "$stage")"
rm -rf "$stage"
echo "wrote $stage.tar.gz"
