# antiquota

Long-running agent loops on the Antigravity CLI (`agy`), spread over several
Google accounts. antiquota runs agy with its own copy of your agy setup, so your
agy CLI and the Antigravity IDE are never changed by it.

```sh
python3 -m antiquota gui          # web GUI on http://127.0.0.1:8765
python3 -m antiquota serve        # the engine alone (for the desktop app or a service)
python3 -m antiquota goal "all tests pass" --check "pytest -q" --cwd ~/code/app
```

## Desktop app

`app/` is a native shell (Tauri v2) around the same engine and UI, for macOS,
Windows and Linux. The window shows the engine's UI; the app adds:

- **Tray / menu bar**: what is working, each account's Gemini quota or why it
  rests, *Stop all running*, *New session*. Closing the window keeps antiquota
  in the tray; quitting asks first while sessions or jobs run, and can leave
  them running in the background (the next start attaches again).
- **Native notifications** (turn finished, goal met, job done, approval
  needed), dock badge with the number of working sessions, a bounce when an
  approval waits.
- Save dialogs for exports, clipboard access for the sign-in code, links in
  your default browser, file drag and drop, one instance, remembered window
  size, *Open at login* (starts in the tray), keep-awake while agents work.

The engine is a separate process (`antiquota serve`, one per antiquota folder,
found through `~/.antiquota/state/engine.json`). The app attaches to a running
engine or starts one: the bundled engine in a release, the source tree in
development.

```sh
cd app && npm install
npm run dev                     # development: engine from this repository
npm run build                   # release: bundled engine (PyInstaller) + .app/.dmg, .msi/.exe, .deb/.rpm/.AppImage
```

Requirements to build: Rust, Node, Python 3.10+. Windows builds use
`scripts/build-engine.ps1` (adds pywinpty, which gives agy the console it needs
to read the sign-in code). `.github/workflows/app.yml` builds all three
platforms. Unsigned builds: macOS Gatekeeper and Windows SmartScreen warn until
the app is signed.

## Permissions and approvals

Headless agy cannot ask anyone and ignores allow-rules, so while antiquota's
`antiquota` hook is on (Settings → Hooks), agy runs with
`--dangerously-skip-permissions` and antiquota decides every tool call
(`antiquota/policy.py`):

- reading and searching pass; *bypass* mode passes everything;
- shell commands and MCP tools follow antiquota's allow/deny rules
  (`command(npm test)`, `mcp(server/tool)`), anything else is asked;
- edits follow the session mode: *accept edits* allows them, *plan* refuses
  them, *default* asks;
- deploys, databases and package installs are asked.

An approval appears in the transcript (*Allow once*, *Always allow …*,
*Decline*); agy waits for it (up to 15 minutes, then hears "no"). With no app
or engine to ask (a CLI run), the hook declines what would need a yes.
Messages you send while agy works reach it at its next step, like Claude Code.
Switching the hook off gives back agy's own headless behaviour.

## GUI

- **Sessions** per project folder, running in parallel; each turn goes to the
  first account with quota left, and a quota error hands the turn to the next
  account in the same conversation.
- **Transcript like Claude Code / Codex**: streamed replies with markdown;
  **thinking** blocks (Gemini's reasoning, read from agy's own conversation db,
  since the CLI stream never carries it); tool rows with a short **preview**
  (last lines of a command, first changed lines of an edit) that expand to the
  full output or diff; a compact mode and an expand-all switch.
- **Token meter** under the composer: context sent with the last request against
  the model's window, session totals (sent, received, thinking), and the
  serving account's remaining quota.
- **Model picker**: family first (grouped by Gemini vs Claude/GPT quota), then
  effort; Gemini efforts are model variants, Claude/GPT-OSS have a fixed one.
- **Goal mode**: the agent keeps working until it ends with `GOAL_COMPLETE`
  *and* an optional check command exits 0 *and* an independent verifier answers
  `VERDICT: DONE`; reasons go back as the next prompt; when every account is out
  of quota it sleeps until the first reset. The verifier defaults to Gemini 3.1
  Pro, a different model on the same Google quota; if a chosen Claude/GPT
  verifier runs out of its (small) quota everywhere, the session's model judges.
- **Routing like a multi-account gateway**: a session keeps its account while
  that account has headroom (its conversation is warm there); below 3% it moves
  on before failing mid-turn. Which account it moves to follows *Rotation* in
  Settings: the allowance that resets soonest first (default, so no quota
  expires unused), one by one, or round robin. Errors are told apart: a spent
  quota walls the account until its reset; a rate limit while the quota panel
  still shows quota only rests it (1, 5, then 15 minutes); temporary errors
  retry twice on the same account, then move on; a rejected sign-in parks the
  account until it signs in again. Quota panels are re-read every 10 minutes
  (no tokens). Every decision lands in a routing log (Accounts drawer).
- **Handoff without a memory service**: the turn continues in the same agy
  conversation on the next account. When a conversation cannot be resumed, or
  a new session should pick up an earlier one, antiquota's own handoff state
  (`STATE.md` in the session folder: objective, requests, a per-turn work log,
  files changed, jobs, open problems, last reply; `/state` shows it) goes in
  front of the next message.
- **Background jobs and /loop**: the agent ends a reply with
  `RUN_IN_BACKGROUND: <cmd>` (or you type `/bg <cmd>`); the job runs detached
  with its own log, and agy gets the end of the log when it exits. A goal waits
  for its jobs instead of nudging. `/loop 15m <prompt>` repeats a prompt while
  the session is idle; `/schedule 03:00 <prompt>` sends one every day at that
  time.
- **Plan, then execute**: in *plan* mode agy only proposes a numbered plan
  (edits are refused); a bar then offers *Execute plan*, *Execute with
  approvals* or *Revise*.
- **Git worktrees**: *New session → Separate git worktree* gives the session
  its own branch (named after the first request) and folder, so parallel
  sessions never touch the same files or your checkout. The Changes tab shows
  uncommitted files with diffs, discard per file, *Commit all*, what the branch
  adds, *Merge into <base>* (refused if your checkout has uncommitted changes)
  and *Remove worktree*. Sessions in any repository get the same review and
  commit.
- **Claude Code habits**: `!cmd` runs a shell command (agy sees the output with
  your next message), `# note` appends to the project's GEMINI.md, `/init`,
  `/memory`, `@file` mentions, images and files by +, drag and drop or paste,
  rewind, `/compact`, `/export`; agy's file links open in a viewer.
- **Accounts by email**: *Add account* opens Google sign-in; click "Copy to
  Clipboard" on Google's page and come back: the code goes in by itself (agy
  allows 60 seconds). Accounts Google wants verified first are kept, parked with
  a *Verify with Google* link and *Check again*; they join the rotation once
  Google accepts them. Each card shows quota windows and today's tokens.
- **Changes** panel: git status and commit (above), plus every file agy
  edited this session with diffs. **Files** panel: browse the session folder,
  view a file, add it as an `@` mention, open it or show it in Finder/Explorer
  (desktop app).
- **Usage**: tokens per day for 14 days (hover for the split by account) and a
  per-account table, in the Accounts panel.
- **Sessions**: right-click or ⋯ to rename, pin, fork, archive, delete; ⌘K
  searches titles and message text; ⌘/ lists every shortcut and command.
- **Settings** (⌘,): antiquota's own rules (GEMINI.md, rules/), skills, hooks,
  MCP servers, plugins, permissions and session defaults, each compared with
  your agy CLI and the IDE, with one-click copy from either.
- Slash commands: `/goal`, `/stop`, `/model`, `/effort`, `/mode`, `/login`,
  `/settings`, ...

The server binds to 127.0.0.1 only; every API call needs the install's token and
a localhost Host header.

## Accounts and the stack

```
~/.antiquota/
  stack/.gemini/          antiquota's copy of ~/.gemini: rules, skills, hooks, MCP, plugins, settings
  slots/<email>/          one fake HOME per account; its sign-in token is a file in it
  sessions/<id>/          meta.json + events.jsonl per session
  state/                  quota walls, leases, probe snapshots, raw failures
```

- **Tokens are files, on every OS.** agy normally keeps its token in the OS
  keyring (Keychain, Secret Service, Credential Manager), which belongs to the
  OS user, so a second account would overwrite the first and your own agy CLI.
  When agy sees an SSH session it uses file storage instead (its log:
  `Using file-based token storage because SSH session detected`). antiquota sets
  `SSH_CONNECTION` (and `HOME`/`USERPROFILE`) on every agy call, so each token
  lives in `slots/<email>/.gemini/antigravity-cli/antigravity-oauth-token`. Same on
  macOS, Linux and Windows, headless or not; your keyring is never touched. A
  slot folder can be copied to another machine. `antiquota doctor` checks that
  every account really signs in as its own email.
- On first use the whole `~/.gemini` is cloned into the stack, except identity,
  per-process state and history. Absolute `~/.gemini` paths in `hooks.json` and
  `mcp_config.json` are pointed at the copy. The IDE folder
  (`~/.gemini/antigravity`) is not cloned; copy items from it in Settings.
- A slot links `.gemini` from the stack and every other HOME entry (git, ssh,
  gitnexus and claude-mem data) from the real HOME. On Windows without symlink
  rights it uses junctions and hard links. All slots share one conversation
  store, so a turn can continue on another account.
- One agy process per account at a time (file lock). Gemini and Claude/GPT
  quotas are walled separately from `/quota` (no model tokens).
- Sign-in: agy prints a Google URL and reads the code from a terminal, so
  antiquota gives it a pseudo-terminal (macOS, Linux). On Windows agy finishes
  sign-in by itself once the browser step completes.

## CLI

```sh
antiquota login                  # add a Google account (link + paste code)
antiquota accounts               # quota, walls and leases per account
antiquota probe                  # refresh quota walls (cron every 10 min if no GUI runs)
antiquota doctor                 # every account loads the stack's skills and hooks?
antiquota stack [reseed]         # stack folder; reseed re-copies config from ~/.gemini
antiquota run -- -p "..."        # agy on the first account with quota left; exit 75 if none
antiquota goal "objective" ...   # headless goal run; the session also shows in the GUI
```

## Shared stack fixes

[`stack/gitnexus-agy-hook.cjs`](stack/gitnexus-agy-hook.cjs) bridges agy hooks
to the GitNexus Gemini-CLI hook: agy's `PostToolUse` may only answer `{}`, so
the bridge parks GitNexus context after a search and injects it through
`PreInvocation` before the next model call.

## Development

```sh
python3 -m pytest tests -q      # stub agy, no account, no quota
```
