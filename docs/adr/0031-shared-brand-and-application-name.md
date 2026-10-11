# ADR 0031: share one mark across the GUI and native application

The sidebar, splash and desktop icon previously used different shapes. Use one vector at `ui/src/assets/brand-mark.svg`: an ascending A with a tilted amber orbit and an account node. The sidebar takes ink and amber from the existing theme tokens. The splash uses the same geometry. Platform icons and the favicon add a graphite tile; the tray uses a monochrome template of the same mark.

`npm --prefix app run icons` derives the assets with the existing Tauri CLI, validates PNG sizes and platform signatures, and removes its temporary sandbox. Commit the results so normal builds need no icon tools. Repeated generation must produce identical bytes. Keep the favicon under the existing `/assets/` static route.

The owner chose **Antiquota** for the installed application and its native application identity. The GUI wordmark, splash, page and window titles, and CLI remain **antiquota**. Preserve the bundle identifier, executable, storage paths and wire formats. macOS packaging reads the resulting `Antiquota.app` and names the installer volume Antiquota.

The Linux package checker reads the product name for its resource directory and desktop entry, matching the [Tauri 2.12.1 bundler](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.1/crates/tauri-bundler/src/bundle/linux/freedesktop/mod.rs). Pin the WiX upgrade code derived from the previous lowercase product name so renaming does not create a new Windows upgrade identity. Fresh Linux and Windows installers still need their own platform runs.

Native proof uses a separate test bundle identifier and a background launch with an isolated fake engine. Standard macOS config and WebKit locations can ignore HOME overrides. Relocate only that test identity's directories into the Desktop test sandbox and retain links at the expected OS paths. Do not redirect the installed application's state, bypass permissions, move user windows, or infer production-identity permission proof from this test.
