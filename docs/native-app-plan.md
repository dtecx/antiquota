# antiquota: landscape, native app plan, missing features

Written 2026-10-07. The facts about agy below were checked against agy 1.3.0 on this machine unless marked "to verify".

## 1. Are we reinventing the wheel?

The answer differs by layer.

### Account layer: yes, partly

Many tools already switch agy between Google accounts. As of 2026-10-07 the serious ones are:

| Project | What it does | How it compares to antiquota |
|---|---|---|
| [hrmasss/antigravity-swap](https://github.com/hrmasss/antigravity-swap) (`aswap`, Python) | Switch, auto-switch, quota table. `aswap run` / `aswap exec` resume the same conversation on the next account when one runs out. Strategies: best, round-robin, next-available, soonest-reset. Hysteresis, cooldown, quarantine of dead logins, export/import, aliases, directory→account binding, `--json`. | The most mature account layer. It hands off the same way antiquota does (same conversation id). On macOS and Windows it is limited to **one account at a time**, because the login lives in the OS keyring. Worth borrowing ideas from (see section 4). |
| [wafardev/multigravity-cli](https://github.com/wafardev/multigravity-cli) (`mgy`) | One HOME per profile; history, MCP and settings are shared through links. | Same idea as antiquota's slots. No routing, no harness. |
| [Dicklesworthstone/coding_agent_account_manager](https://github.com/Dicklesworthstone/coding_agent_account_manager) (CAAM, Go, 208★) | Swaps the auth files of Claude Code, Codex and agy. `shallow-spawn` runs parallel sessions with an isolated HOME. | Generic account vault. No goal mode, routing or GUI for agy. |
| [cruel-synthesis/agy-auth](https://github.com/cruel-synthesis/agy-auth) | Switch plus "auto" ranking by quota that would go unused. | Switch only. |
| [daniilkireev/agy-multi-account-sync](https://github.com/daniilkireev/agy-multi-account-sync) | One `agy remote-control` daemon per account, with a shared conversation database (Linux, systemd). | Targets Google's web UI, not automation. |
| [lbjlaq/Antigravity-Manager](https://github.com/lbjlaq/Antigravity-Manager) (32k★, Tauri) | Account manager, plus an API proxy that turns Antigravity OAuth into OpenAI/Anthropic endpoints. | This is the API-proxy approach, which you said you do not want. Antigravity's terms §6 name "third party software … with Antigravity OAuth" as a breach. It is still a useful reference for a Tauri desktop app in this space. |

There are also dozens of zero-star "switcher / quota HUD" repositories created in the last few weeks. Many of them ship prebuilt executables. Treat them as untrusted, and do not run their binaries.

### Harness layer: no

Nothing found combines these on top of agy:
- several Google accounts at once
- goal mode with a check command and an independent verifier
- background jobs that wake the agent
- one cloned stack per tool set
- a GUI

The closest multi-provider harnesses do not support agy:
- [NeotaskInc/neomax-orchestrator](https://github.com/NeotaskInc/neomax-orchestrator) does account rotation and worker failover across Claude, Codex, OpenCode, Kimi and Grok.
- [metratek-telematics/relay](https://github.com/metratek-telematics/relay) runs a supervisor, a worker and a reviewer across Codex, Claude Code and Gemini CLI, with a browser UI and git worktrees.

### What only antiquota does

**File-token mode** (`SSH_CONNECTION` plus a per-account HOME). It makes agy keep each login in that account's own folder on every OS. So antiquota runs several accounts **at the same time** on macOS and Windows too, where aswap, agy-auth and Antigravity-Manager's CLI mode can run only one.

### Verdict

Keep antiquota's own account layer: it is small, and concurrency is the point. Copy aswap's best ideas (section 4) instead of depending on it. The harness is not a reinvention.

### Risk

Pooling many free student AI Pro accounts is a pattern Google can see. The terms say tool misuse "may be grounds for suspension or termination". antiquota drives the official agy binary and never uses the OAuth tokens itself, which is the safer side of §6. It is still not risk-free. Do not add direct calls to Google's quota or chat endpoints with stored tokens.

## 2. Native app: decision

**Tauri v2 shell plus the existing Python engine as a sidecar daemon.** The HTTP+SSE API stays the contract.

These were considered and rejected for now:

| Option | Why not (now) |
|---|---|
| Electron | Bundles of 150 MB or more and a Node runtime. Its one advantage is the same Chromium everywhere, and our UI is plain web code that WebKit renders fine. |
| Wails (Go) / Rust rewrite of the engine | Weeks of porting a working, tested engine for no user-visible gain. This can come later (phase 5), once the engine is stable. |
| Flutter / Qt / SwiftUI | Throws away the web UI and needs a new UI per platform. |
| PWA only | No tray, no reliable notifications, no keep-awake, no sidecar. |

Why Tauri:
- 3–10 MB shell, using the system WebView: WKWebView, WebView2, WebKitGTK.
- First-class sidecar binaries.
- Plugins for the tray, notifications, the updater (signed), single instance, deep links, autostart, global shortcuts and window state.
- Antigravity-Manager already ships on it for macOS, Windows and Linux.

### Architecture

```
┌──────────── antiquota.app (Tauri v2) ────────────┐
│  WebView UI (TypeScript)        Rust shell       │
│  sessions · goal · jobs ·       tray · notify ·  │
│  accounts · settings            updater · deep   │
│            │  HTTP + SSE (token) links · autostart│
└────────────┼─────────────────────────────────────┘
             ▼
   antiquotad  (Python engine, sidecar or remote)
   accounts · routing · sessions · goal · jobs · stack
             │ spawns
             ▼
   agy (official binary, one HOME per account)
```

Design rules:

- **The engine is a daemon, the window is a client.** Closing the window keeps `antiquotad` running in the tray, so goals, training jobs and `/loop` continue. Quitting from the tray asks first while work is running.
- **Local or remote engine.** The app connects to `127.0.0.1` by default, or to a remote `antiquotad`, for example a headless Linux GPU box where the training runs. The connection goes over an SSH tunnel the app opens, or a token over TLS. This is the key piece for experiment loops: the engine must run where the code and the GPU are.
- **One API for every client.** The desktop app, a browser on any device, and the `antiquota` CLI all use the same API.
  - Version it as `/api/v1`.
  - Describe it in an OpenAPI file.
  - Put a schema version on events.

## 3. Plan in phases

### Phase 0: engine ready to be a daemon (about 1 week)

- `antiquotad`:
  - one engine per user (lock file) and a health endpoint
  - graceful shutdown
  - jobs keep running across restarts (already true)
- API v1 and OpenAPI. Event schema with `v`. Breaking changes need a version bump.
- **Persistent agy sessions:** one `agy --input-format stream-json --output-format stream-json -p ""` process per conversation. Messages are NDJSON in the form `{"event":"user","message":{"role":"user","content":…}}`.
  - Verified: two turns in one process, same conversation; the second turn took 2.6 s.
  - Benefits: faster turns, and true queueing.
  - To verify: whether the "Mid-turn SendUserMessage" path in the binary allows steering mid-turn.
  - Restart the process on an account switch, which is the only time the token changes.
- **Live approvals:** agy has a `PreToolUse` hook with `decision: allow|deny|ask|…`. The stack gets a small hook script. The script asks the engine, the engine asks the GUI, and the user answers "allow once", "always" or "deny". Headless turns then no longer die on a denied command.
- **Keep-awake while work runs, in the engine** (so it works headless too):
  - macOS: `caffeinate -i -w <pid>`
  - Windows: `SetThreadExecutionState`
  - Linux: `systemd-inhibit`

### Phase 1: Tauri shell around today's UI (about 1 week)

- `src-tauri/`:
  - start `antiquotad` as a sidecar, or attach to an engine that is already running
  - pass the token through Tauri state (no query string)
  - single instance
  - window state
- **Tray:**
  - per-account quota rings
  - running sessions and jobs
  - "Open", "Pause all", "Quit"
- **Native features:**
  - notifications (goal met, job finished, account walled, approval needed)
  - an `antiquota://session/<id>` deep link
  - optional autostart
  - a global shortcut that opens a new session
- Native file drop and paste, through the existing attachment API.

### Phase 2: UI rewrite in TypeScript (2–3 weeks, can overlap)

The current vanilla JS (about 2k lines) has reached the size where components and types pay off.

- Stack: Vite + TypeScript + **React**, chosen for its ecosystem.
- Libraries:
  - Radix primitives for dialogs, menus and comboboxes
  - TanStack Virtual for long transcripts
  - Shiki for code
  - CodeMirror 6 for the file viewer and editor
  - xterm.js for a terminal pane
  - Zustand for state
- Keep the design tokens and the look. Port screen by screen behind the same API.
- Accessibility: keyboard paths everywhere, focus management, reduced motion.

### Phase 3: packaging, signing, updates (about 1 week)

- CI matrix:
  - macOS arm64/x64
  - Windows x64
  - Linux x64/arm64
- Steps:
  1. Build `antiquotad` with PyInstaller (onedir, for fast start).
  2. Build the Tauri bundles: `.dmg`, NSIS `.exe` / `.msi`, `.AppImage` / `.deb` / `.rpm`.
- Signing:
  - macOS needs Developer ID and notarization, otherwise Gatekeeper blocks it.
  - Windows needs a code-signing certificate, otherwise SmartScreen warns. A PyInstaller sidecar raises false antivirus alarms more often without one.
  - The updater uses Tauri's signed manifest.
- agy itself is not bundled. The app finds it, checks the version, and links to the install steps.
- Linux headless: `antiquotad` plus the browser UI only, which is today's mode.

### Phase 4: platforms

| | macOS | Windows | Linux desktop | Linux headless |
|---|---|---|---|---|
| Account isolation (file tokens) | verified | to verify | expected | expected |
| Sign-in code entry | pty, verified | **needs ConPTY** (`pywinpty`) | pty | pty |
| Stack links | symlinks | junctions / hard links (written, untested) | symlinks | symlinks |
| Keep-awake | caffeinate | SetThreadExecutionState | systemd-inhibit | systemd-inhibit |

### Phase 5 (optional, later)

- Port `antiquotad` to Rust or Go for a single binary with no Python. Only do this once the API has been stable for a while.
- If agy ships ACP ([issue #31](https://github.com/google-antigravity/antigravity-cli/issues/31), 196 comments), use it, and expose antiquota itself as an ACP agent for Zed and JetBrains.

## 4. Missing features, by value

### High

1. **Live approvals** through a `PreToolUse` hook (phase 0). This is the biggest gap from Claude Code.
2. **Persistent agy sessions** with stream-json input (phase 0): faster turns, a real queue, and possibly mid-turn steering.
3. **Remote engine and phone access:**
   - the app connects to an engine on a GPU box
   - a mobile-friendly UI
   - push notifications through ntfy or Telegram for long runs
4. **Experiment runs panel** for loops like training:
   - each job gets metrics parsed from its log (loss, accuracy, by regex)
   - a chart, and a comparison of runs
   - the config diff between runs
   - keep-awake while jobs run
5. **Account layer parity with aswap:**
   - hysteresis
   - a dead-login check and quarantine through `agy models` (lists models, no quota spent)
   - disable an account, or hold it out for N minutes
   - aliases
   - export and import of accounts
   - binding a project folder to an account
   - `antiquota accounts --json`
6. **Git worktree per session or task**:
   - diff review with accept/reject per hunk
   - commit, branch, draft PR
   - this makes parallel sessions on one repository safe
7. **Windows sign-in** through ConPTY, and real test runs on Windows and Linux.

### Medium

8. Plan-mode flow: plan, approve, execute. Render agy's task list. An agent picker for `--agent` / `agy agents`.
9. Sessions:
   - full-text search
   - fork a session from a turn
   - archive and pin
   - group by project
10. Usage history: per-account and per-day charts, and how much of the weekly allowance went unused at each reset.
11. Scheduled runs (cron-like "every night at 03:00", not only `/loop`).
12. A terminal pane (xterm.js) and a file tree for the session folder.

### Low / only if needed

13. Model fallback across quota pools (opt-in; Claude/GPT quota is small and may leave AI Pro).
14. More than one concurrent call per account. Measure Google's per-account limits first.
15. Automatic compaction: likely not needed. The binary carries a built-in checkpointer configuration (`CASCADE_USE_EXPERIMENT_CHECKPOINTER`, token threshold 50k), so agy appears to summarize long conversations itself. To verify on a long session.
