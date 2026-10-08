#!/bin/sh
# Bundle the Rust engine and the same production UI used by browsers.
set -eu
here=$(cd "$(dirname "$0")/.." && pwd)
root=$(cd "$here/.." && pwd)
cd "$root"
cargo build --release --locked --jobs 2 -p antiquota-cli
pnpm --dir ui install --frozen-lockfile
pnpm --dir ui build
mkdir -p "$here/engine-dist/bin"
cp "$root/target/release/antiquota" "$here/engine-dist/bin/antiquota"
rm -rf "$here/engine-dist/ui"
cp -R "$root/ui/dist" "$here/engine-dist/ui"
"$here/engine-dist/bin/antiquota" --help >/dev/null
