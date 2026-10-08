# ADR 0022: Native shell and Rust sidecar

Date: 2026-10-08

Status: Accepted

The desktop crate joins the Rust workspace and its lockfile. Release scripts bundle the Rust antiquota executable and the same built UI served by browsers. An explicit engine executable takes precedence; otherwise discovery checks bundled resources before development binaries. No Python interpreter or PyInstaller is required. Engine metadata accepts only a canonical loopback URL and a matching healthy antiquota PID. Requests do not follow redirects or proxies, and the GUI token never enters the URL or Debug output.

The sidecar has an independent process group, private append-only output and app ownership markers. The shell attaches through the existing engine.json and GUI-token files. Restart waits for shutdown before rediscovery and propagates failures. Healthy rediscovery retains the current engine-origin page, its selection and unsent drafts. Unknown work state cannot count as idle when quitting; dismissal keeps the engine running.

The platform adapter uses official Tauri APIs for zoom, clipboard, external HTTP links, file actions, autostart, attention, notifications and export. External links have explicit HTTP and HTTPS permission scopes. File opening stays within the user's home, including hidden components. Menu events contain typed string actions; tray IDs use their own namespace so global menu listeners cannot dispatch an action twice. Deferred UI completions retain their initiating session and view lifetime.

Native zoom applies once to the root webview, while the sidebar reserves physical traffic-light space. Window-state restoration includes size, position and maximization, excludes visibility, and checks the final restored title position against connected monitors. Hidden startup remains hidden. Debug background proof delays showing the test window until the application is ready and suppresses focus requests. It does not change release behavior. The splash registers its status listener before requesting startup.

Native Mac dictation prefers AAC MP4 and disables browser speech services. The narrow macOS transport exception permits only HTTP loopback at 127.0.0.1. This follows [Apple's local-network transport documentation](https://developer.apple.com/documentation/bundleresources/information-property-list/nsapptransportsecurity/nsallowslocalnetworking); transport security is not disabled globally.

Mac builds produce an app and compressed DMG. Without a configured signing identity the local bundle is ad-hoc signed and its signature is verified; this does not establish Developer ID signing, notarization or Gatekeeper acceptance. CI prepares native dependencies and runs both toolchains before platform bundling. Remote outcomes are recorded separately.

Focused and whole gates prove synthetic engine behavior, browser fallbacks, mocked native IPC and the bundled splash HTML. They do not prove OS menu dispatch, native zoom geometry, microphone permissions or an actual AAC recording. Background Mac startup remains blocked in a regular-file open for the test engine.json. A contemporaneous OS policy check is evidence to investigate, not a confirmed cause. Synchronous discovery reads can bypass the polling deadline while blocked. Native acceptance remains Pending until that condition is resolved with the user; no permission is accepted or protection bypassed automatically.

The pinned Linux GTK3 dependency requires the unmaintained proc-macro-error crate. Cargo-deny has one documented exception for [RUSTSEC-2024-0370](https://rustsec.org/advisories/RUSTSEC-2024-0370.html), which has no patched version. Review this exception when Tauri no longer requires GTK3. There is no paid live call or cutover in this slice.
