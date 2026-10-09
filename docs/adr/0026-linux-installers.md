# ADR 0026: Linux installers

Status: Accepted for Slice 15.

On Linux, `app/scripts/build-app.sh` asks Tauri for a deb, an rpm and an AppImage, then runs `app/scripts/verify-linux-packages.sh`. The check unpacks each package with `dpkg-deb`, `rpm2cpio` and the AppImage's own `--appimage-extract`, so it needs no root and installs nothing. It looks for the desktop app, exactly one Rust engine, the UI and the menu entry, refuses Python or PyInstaller files, and starts the packaged engine in a sandbox under `~/Desktop/test`: health, the token-filled UI, a second `serve` that attaches to the first, and `engine.json` removed on exit. CI runs the same check after its Linux installer build.

Every installer keeps the engine in `engine/` beside the UI in `ui/`. Without `ANTIQUOTA_UI_DIR`, the engine now looks for that `ui` folder first, so an engine started from a package serves its own UI. Development builds still use the source tree's `ui/dist`.
