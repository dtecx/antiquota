#!/bin/sh
# Release build: bundled engine + app, then installer images per platform.
set -eu
here=$(cd "$(dirname "$0")/.." && pwd)
cd "$here"
sh scripts/build-engine.sh
version=$(node -p "JSON.parse(require('fs').readFileSync('src-tauri/tauri.conf.json','utf8')).version")
case "$(uname -s)" in
  Darwin)
    npx --no-install tauri build --config src-tauri/tauri.bundle.json --bundles app
    app="../target/release/bundle/macos/Antiquota.app"
    if [ -z "${APPLE_SIGNING_IDENTITY:-}" ]; then
      codesign --force --deep --sign - "$app"
    fi
    codesign --verify --deep --strict "$app"
    mkdir -p dist
    out="dist/antiquota-$version-$(uname -m).dmg"
    stage_root=${TMPDIR:-"$HOME/Desktop/test"}
    mkdir -p "$stage_root"
    stage=$(mktemp -d "$stage_root/antiquota-bundle.XXXXXX")
    cp -R "$app" "$stage/"
    ln -s /Applications "$stage/Applications"
    hdiutil create -quiet -volname "Antiquota" -srcfolder "$stage" -ov -format UDZO "$out"
    rm -rf "$stage"
    echo "built $out"
    ;;
  Linux)
    npx --no-install tauri build --config src-tauri/tauri.bundle.json --bundles deb,rpm,appimage
    sh scripts/verify-linux-packages.sh
    ;;
  *)
    npx --no-install tauri build --config src-tauri/tauri.bundle.json
    ;;
esac
