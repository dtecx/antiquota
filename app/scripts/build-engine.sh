#!/bin/sh
# Build the engine as a self-contained folder (app/engine-dist/agyloopd) with PyInstaller,
# so the desktop app needs no Python on the user's machine.
set -eu
here=$(cd "$(dirname "$0")/.." && pwd)
root=$(cd "$here/.." && pwd)
venv="$here/.venv"
py=${AGYLOOP_BUILD_PYTHON:-python3}
[ -x "$venv/bin/python" ] || "$py" -m venv "$venv"
"$venv/bin/python" -m pip install --quiet --upgrade pip pyinstaller
cd "$root"
"$venv/bin/pyinstaller" --noconfirm --clean --onedir --name agyloopd \
  --distpath "$here/engine-dist" --workpath "$here/.build" --specpath "$here/.build" \
  --paths "$root" --add-data "$root/agyloop/ui:agyloop/ui" \
  --collect-submodules agyloop \
  "$here/engine/agyloopd.py"
"$here/engine-dist/agyloopd/agyloopd" --help >/dev/null 2>&1 || true
echo "engine: $here/engine-dist/agyloopd"
