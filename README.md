# agyloop

Long-running agent loops on the Antigravity CLI (`agy`), spread over several
Google accounts. agy-loop runs agy with its own copy of your agy setup, so your
agy CLI and the Antigravity IDE are never changed by it.

```sh
python3 -m agyloop gui          # web GUI on http://127.0.0.1:8765
python3 -m agyloop goal "all tests pass" --check "pytest -q" --cwd ~/code/app
```

## GUI

- Sessions per project folder, a streamed transcript with tool steps, file
  diffs, markdown, and a composer with model, effort and permission mode.
- **Goal mode**: the agent keeps working until it ends with `GOAL_COMPLETE`
  *and* an optional check command exits 0 *and* an independent verifier (by
  default Claude Sonnet, on the separate Claude/GPT quota) answers
  `VERDICT: DONE`. Otherwise the reason goes back to the agent as the next
  prompt. When every account is out of quota it sleeps until the first reset.
- **Accounts by email**: *Add account* opens Google sign-in; paste the code it
  shows (agy allows 60 seconds). Accounts are used in order; a quota error
  passes the turn to the next account in the same conversation.
- **Settings** (⌘,): edit agy-loop's own rules (GEMINI.md, rules/), skills,
  hooks, MCP servers, plugins, permissions and session defaults. Each item shows
  whether it matches your agy CLI or the IDE, with one-click copy from either.
- Slash commands: `/goal`, `/stop`, `/model`, `/mode`, `/login`, `/settings`, ...

The server binds to 127.0.0.1 only and every API call needs a per-launch token
and a localhost Host header.

## Accounts and the stack

```
~/.agyloop/
  stack/.gemini/          agy-loop's copy of ~/.gemini: rules, skills, hooks, MCP, plugins, settings
  slots/<email>/          one fake HOME per account; its sign-in token is a file in it
  sessions/<id>/          meta.json + events.jsonl per session
  state/                  quota walls, leases, probe snapshots, raw failures
```

- **Tokens are files, on every OS.** agy normally keeps its token in the OS
  keyring (Keychain, Secret Service, Credential Manager), which belongs to the
  OS user, so a second account would overwrite the first and your own agy CLI.
  When agy sees an SSH session it uses file storage instead (its log:
  `Using file-based token storage because SSH session detected`). agy-loop sets
  `SSH_CONNECTION` (and `HOME`/`USERPROFILE`) on every agy call, so each token
  lives in `slots/<email>/.gemini/antigravity-cli/jetski_state.pbtxt`. Same on
  macOS, Linux and Windows, headless or not; your keyring is never touched. A
  slot folder can be copied to another machine. `agyloop doctor` checks that
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
  agy-loop gives it a pseudo-terminal (macOS, Linux). On Windows agy finishes
  sign-in by itself once the browser step completes.

## CLI

```sh
agyloop login                  # add a Google account (link + paste code)
agyloop accounts               # quota, walls and leases per account
agyloop probe                  # refresh quota walls (cron every 10 min if no GUI runs)
agyloop doctor                 # every account loads the stack's skills and hooks?
agyloop stack [reseed]         # stack folder; reseed re-copies config from ~/.gemini
agyloop run -- -p "..."        # agy on the first account with quota left; exit 75 if none
agyloop goal "objective" ...   # headless goal run; the session also shows in the GUI
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
