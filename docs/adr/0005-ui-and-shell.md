# ADR 0005: One Svelte build and a thin native shell

Status: Accepted; browser/native parity proof pending.

Use a Svelte 5 SPA, strict TypeScript, Vite, and Svelte stores. The same production build runs in the browser and Tauri. No SvelteKit server or second webview app is needed. Stores own cross-view state and pure reducers handle events; focused components own rendering. [Svelte stores](https://svelte.dev/docs/svelte/stores), [Vite guide](https://vite.dev/guide/).

Preserve the current dense warm visual language. A single CSS token file defines light/dark themes and geometry. Split view renders two session components using one event connection, with per-session input/scroll/terminal state. Use xterm.js with fit, Chart.js, and MediaRecorder directly and dispose their instances when widgets unmount.

Native scaling uses Tauri webview `setZoom`, never CSS root zoom. Browser zoom remains browser-controlled. Window-state saves size, position, and maximized only. Validate final restored bounds after applying size and center an off-screen window while preserving hidden startup.

The shell owns sidecar discovery/supervision, tray, menus, notifications, and autostart. The sidecar is the Rust `antiquota` binary; no Python bundler remains after approved cutover. CLI and GUI share sessions through existing `engine.json` discovery.

Proof: split layouts at 80/100/125/150%, menu/tooltip/terminal geometry, both themes/narrow windows, native quit/relaunch and off-screen fallback, hidden autostart, engine respawn, and DMG installation. Source inspection alone is insufficient native evidence.
