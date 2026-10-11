#!/bin/sh
# Checks each Linux installer: the desktop app, one Rust engine and the UI are inside, and the
# packaged engine starts by itself and serves that UI. Test files stay under ~/Desktop/test.
set -eu
root=$(cd "$(dirname "$0")/../.." && pwd)
bundle="$root/target/release/bundle"
product=$(node -p "JSON.parse(require('fs').readFileSync(process.argv[1],'utf8')).productName" "$root/app/src-tauri/tauri.conf.json")
mkdir -p "$HOME/Desktop/test"
work=$(mktemp -d "$HOME/Desktop/test/antiquota-packages.XXXXXX")
trap 'rm -rf "$work"' EXIT

check() {
  label=$1
  tree=$2
  for path in usr/bin/antiquota-desktop "usr/lib/$product/engine/antiquota" \
    "usr/lib/$product/ui/index.html" "usr/lib/$product/ui/app.js" \
    "usr/share/applications/$product.desktop"; do
    test -s "$tree/$path" || { echo "$label: missing $path" >&2; exit 1; }
  done
  engines=$(find "$tree" -type f -name antiquota | wc -l)
  test "$engines" -eq 1 || { echo "$label: $engines engine executables" >&2; exit 1; }
  if find "$tree" -name '*.py' -o -name 'antiquotad*' | grep -q .; then
    echo "$label: carries Python or PyInstaller files" >&2
    exit 1
  fi
  engine="$tree/usr/lib/$product/engine/antiquota"
  "$engine" --help >/dev/null
  "$engine" serve --help >/dev/null
  data="$work/$label-data"
  mkdir -p "$data" "$work/$label-home"
  ANTIQUOTA_DIR="$data" ANTIQUOTA_HOME="$work/$label-home" ANTIQUOTA_NO_USAGE_LINK=1 \
    "$engine" serve --port 0 >"$work/$label.log" 2>&1 &
  pid=$!
  tries=0
  until [ -s "$data/state/engine.json" ]; do
    tries=$((tries + 1))
    if [ "$tries" -gt 200 ]; then
      cat "$work/$label.log" >&2
      kill "$pid" 2>/dev/null || true
      echo "$label: the packaged engine did not start" >&2
      exit 1
    fi
    sleep 0.1
  done
  port=$(sed -n 's/.*"port":\([0-9]*\).*/\1/p' "$data/state/engine.json")
  curl -fsS "http://127.0.0.1:$port/api/health" | grep -q '"app":"antiquota"'
  # The engine serves the UI from the package, with its token injected.
  curl -fsS "http://127.0.0.1:$port/" | grep -q 'name="antiquota-token" content="[^_]'
  # A second engine finds the running one through engine.json and leaves it alone.
  ANTIQUOTA_DIR="$data" ANTIQUOTA_HOME="$work/$label-home" ANTIQUOTA_NO_USAGE_LINK=1 \
    "$engine" serve --port 0 >"$work/$label-second.log" 2>&1
  grep -q "127.0.0.1:$port" "$work/$label-second.log"
  kill "$pid"
  wait "$pid" || true
  test ! -e "$data/state/engine.json" || { echo "$label: engine.json left behind" >&2; exit 1; }
  echo "$label: ok ($(basename "$3"))"
}

deb=$(ls "$bundle"/deb/*.deb)
rpm=$(ls "$bundle"/rpm/*.rpm)
appimage=$(ls "$bundle"/appimage/*.AppImage)
dpkg-deb -I "$deb" | grep -q 'Depends: .*libwebkit2gtk-4.1'
dpkg-deb -x "$deb" "$work/deb"
check deb "$work/deb" "$deb"
mkdir -p "$work/rpm"
(cd "$work/rpm" && rpm2cpio "$rpm" | cpio -idm --quiet)
check rpm "$work/rpm" "$rpm"
(cd "$work" && "$appimage" --appimage-extract >/dev/null)
check appimage "$work/squashfs-root" "$appimage"
