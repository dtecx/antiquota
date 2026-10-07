#!/bin/sh
# Release build: bundled engine + app, then installer images per platform.
set -eu
here=$(cd "$(dirname "$0")/.." && pwd)
cd "$here"
sh scripts/build-engine.sh
version=$(python3 -c "import json; print(json.load(open('src-tauri/tauri.conf.json'))['version'])")
case "$(uname -s)" in
  Darwin)
    npx tauri build --config src-tauri/tauri.bundle.json --bundles app
    app="src-tauri/target/release/bundle/macos/antiquota.app"
    mkdir -p dist
    out="dist/antiquota-$version-$(uname -m).dmg"
    stage=$(mktemp -d)
    cp -R "$app" "$stage/"
    ln -s /Applications "$stage/Applications"
    hdiutil create -quiet -volname "antiquota" -srcfolder "$stage" -ov -format UDZO "$out"
    rm -rf "$stage"
    echo "built $out"
    ;;
  Linux)
    npx tauri build --config src-tauri/tauri.bundle.json --bundles deb,rpm,appimage
    ;;
  *)
    npx tauri build --config src-tauri/tauri.bundle.json
    ;;
esac
