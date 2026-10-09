# antiquota: agent guide for the Rust and TypeScript rewrite

Read this file first. It is the complete handoff for an agent without memory, skills or local tools. The branch `rust` holds the rewrite; `main` holds the legacy Python app, which stays untouched.

## What antiquota is

antiquota is a local workbench for Google's `agy` coding agent (the Antigravity CLI). It runs many agy sessions across several Google accounts and moves work to an account that still has quota. Main parts:

- Sessions: one agy conversation per session, with a durable transcript, a message queue, steering while a turn runs, stop, fork, rewind, clear and compact.
- Accounts: sign-in, quota probes, routing by headroom, walls when quota is spent, and handoff to another account.
- Safety: a permission policy and a fast `antiquota hook` that agy calls before tools; approvals in the UI.
- Goal mode: work toward an objective until a check command passes and an independent verifier agrees.
- Background jobs, loops and daily schedules; git worktrees and change review; a terminal per session; attachments and dictation.
- A browser UI, a CLI (`run`, `goal`, account commands, `serve`), and later a native Tauri shell with a tray.

The legacy implementation is in this branch for reference: `antiquota/*.py` (engine, server, accounts, policy) and `antiquota/ui/app.js` (UI). Read it when a feature row cites it.

## Layout

- `crates/core`: pure domain types, policy, prompts and the ports (traits) in `ports.rs`. No I/O.
- `crates/features`: pure feature decisions (goal, handoff and similar).
- `crates/app`: `Service`, the application layer. `Service::compose(Ports { .. })` wires the adapters. Tests with in-memory ports are in `crates/app/src/tests/`.
- `crates/adapters/*`: `agy` (runner, ask, preflight), `accounts`, `store` (files, attachments, workspace), `pty` (terminal), `shell` (commands, jobs, keep-awake), `git`.
- `crates/server`: axum HTTP and SSE. `http.rs` dispatches routes, `routes.rs` is the route catalog (implemented or pending), `stream.rs` holds the SSE streams.
- `crates/cli`: the `antiquota` binary, including `hook`.
- `tests/fake-agy`: the fake agent used by every test. It reads `FAKE_AGY_SCRIPT` (JSON, such as `{"scenario":"delay","delay_ms":300}`) and writes each argv to `fake-calls.jsonl`.
- `tests/rewrite/*.rs`: HTTP integration tests. Each is included by a `crates/server/tests/*_contract.rs` file through `#[path]`. `tests/rewrite/support` provides `Server` (an isolated engine under `~/Desktop/test`), `Replay` (replays captured cases from `tests/fixtures/api/cases/*.json`) and raw HTTP helpers.
- `ui/`: Svelte 5, strict TypeScript and Vite. `src/gen/` holds types generated from Rust by ts-rs. `src/lib/*.ts` holds the logic, with Vitest tests next to it. `src/components/*.svelte` holds the components. `e2e/*.spec.ts` holds the Playwright tests; `e2e/fixtures.ts` starts a sandbox engine per test.
- `docs/ARCHITECTURE.md`, `docs/adr/` (ADR 0001 to 0026), and `docs/rewrite/`:
  - `FEATURES.md` is the canonical checklist: 1150 rows, each with acceptance text and a `[ ]`/`[x]` box.
  - `TEST_REPORT.md` holds one row per feature (tests, status, evidence) and a section per slice.
  - `known-bugs.md` lists B- (legacy defects), R- (risks) and W- (defects found during the rewrite) entries.
  - `API_CONTRACT.md` and `DATA_FORMATS.md` describe the wire and file formats.

## Status

Slices 1, 3 to 8, 9a to 9f, 11 (storage and defaults), 12 (session lifecycle and turn edges) 13 (quota routing, walls and error channels) 14 (the owner's requests of 2026-10-09: shared accounts, placement, usage detail and copy buttons) and 15 (Linux installers) are done and committed: sessions, permissions and approvals, goals, jobs, worktrees, terminal, attachments and dictation. Slice 2 (accounts and handoff) has code and tests; 33 fully proved backend rows are now folded into the report through `tests/rewrite/slice9f-account-coverage.json`; 32 partial proposal rows retain explicit remaining acceptance. Its account UI evidence is folded in Slice 9b, and Slice 13 proved 19 more quota and routing rows (`tests/rewrite/slice13-coverage.json`). 818 of 1150 rows are Passed. Slice 10 native-shell code and the macOS app/DMG build are implemented and tested; actual native engine navigation, zoom, lifecycle, menus, tray, OS permissions and AAC recording remain Pending because the background test app blocks on a sandbox engine.json filesystem open (R-009).

Next work, in order:

1. Slice 9, the remaining UI and engine parity. It is large, so deliver it as separately committed parts:
   - 9a (done): the remaining routes in the route catalog. These are fork, rewind, clear, compact, inherit, `state`, `shell` (`!cmd`), `note` (`# note`), `files`, search, `engine` and `engine/log`, `usage`, `fs`, the settings file, settings `{kind}/{name}` read, write, delete and enable, settings copy, and stack read and reseed.
   - 9b (done): Settings sections (general, appearance, agent, permissions, stack), account cards, rotation and the usage chart (use Chart.js directly), and the sign-in states.
   - 9c (done): the sessions and projects sidebar, context menus with keyboard support, search (⌘K) and split view. Split view uses two session components on one event connection, with no iframe and no second root zoom.
   - 9d (done): the composer. This covers every slash command entry point, autocomplete (`@` mentions and `/` commands), prompt history with ↑/↓, shortcuts, models and the header.
   - 9e (done): transcript rendering and safe permission retries (49 rows).
   - 9f (done): layout, themes, accessibility, dialog defaults and bounded interaction edges; 77 UI rows and 33 fully proved Slice 2 account rows. Remaining partial browser/account acceptance stays individually Pending.
2. Slice 10, the native Tauri shell: engine sidecar, native webview zoom instead of CSS zoom (B-001), window size and position saved with tauri-plugin-window-state (B-003), tray, menus and splash. Write the code and the Linux build where possible. Rows that need macOS or Windows stay Pending with a clear note.

`grep -n "Pending" docs/rewrite/TEST_REPORT.md` lists every open row. Section headings in `FEATURES.md` group them.

## Environment setup (Linux cloud machine)

```sh
mkdir -p ~/Desktop/test                      # every test sandbox lives here
rustup show                                  # stable toolchain, rustfmt and clippy
cargo install just cargo-deny --locked       # if missing
corepack enable && corepack prepare pnpm@11.19.0 --activate   # or: npx -y pnpm@11.19.0
pnpm --dir ui install --frozen-lockfile
pnpm --dir ui exec playwright install --with-deps chromium
```

On a Linux machine, the workspace needs Rust 1.99 or later (`rustup update stable`) and the webview libraries for the desktop crate: `apt-get install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf`. If Playwright cannot download its Chromium, point `PLAYWRIGHT_BROWSERS_PATH` at a folder whose `chromium-<revision>` and `chromium_headless_shell-<revision>` entries link to an installed Chromium; the revision is in `playwright-core/browsers.json`. Keep one Cargo target folder per checkout in mind: each takes about 13 GB.

`ui/package.json` asks for Node 26.10.0. If pnpm refuses an older Node, install that version (for example with nvm). If `cargo deny` cannot reach its advisory database, run `cargo deny --config .cargo/deny.toml check bans licenses sources` and say so in the report.

## Gates

- `just check`: generated-type freshness, rustfmt, clippy with `-D warnings`, cargo-deny, svelte-check plus tsc, ESLint and Prettier.
- `just test`: Rust tests (build `antiquota-cli` and `fake-agy` first), Vitest, then a UI build and Playwright.
- `just gen-types` regenerates `ui/src/gen` after Rust API types change.
- While you work, run only the affected tests: `cargo test -p <crate> <name>`, `pnpm --dir ui exec vitest run <file>`, `pnpm --dir ui exec vite build && pnpm --dir ui exec playwright test e2e/<file>`. Run the full gates once per committed part. The cloud budget is limited.

## How to deliver a part

1. Read the feature rows for the part in `FEATURES.md` and the legacy source lines they cite.
2. Implement it in the right layer: pure rules in core or features, orchestration in app, I/O in adapters, transport in server, and logic in `ui/src/lib` with thin components.
3. Prove it:
   - Rust unit and app tests.
   - An HTTP integration test that replays every captured fixture for the routes. A legacy defect gets a test of the documented correction.
   - Vitest tests for the UI logic.
   - Playwright for the user-visible behavior, with screenshots saved to `.handoff/evidence/ui-slice<N>/`.
4. Run `just check` and `just test`, and save their output to `.handoff/evidence/slice<N>-check.log` and `slice<N>-test.log`. `.handoff/` is ignored by git, so also write the counts and any failures into the report text.
5. Update `TEST_REPORT.md`. Replace the row `| F-xxx | <a id="f-xxx"></a>Pending — ... | Pending | Pending |` with linked test names, one or two sentences on what they prove, `Passed`, and the evidence path. In `FEATURES.md`, change `[ ]` to `[x]` and `[Pending](TEST_REPORT.md#f-xxx)` to `[Passed](...)` for the same row. Update the totals in the `TEST_REPORT.md` header and in the `FEATURES.md` intro, and add a `## Slice N` section before the line "Mark a row Passed only when its full acceptance". For a partly proven row, keep it Pending and update its **Remaining:** note.
6. Add an ADR (`docs/adr/00NN-*.md`, short plain prose) and one paragraph at the end of `docs/ARCHITECTURE.md`.
7. Commit with a message like `feat: add <part> slice` and push your working branch.

## Hard rules

- Mark a row Passed only when its full acceptance has passing tests. Report failures, flaky runs and skipped checks plainly, with the log.
- There are no real agy binaries or Google accounts in the cloud. Use only the fake agent and synthetic data. Never put tokens, real account data or personal paths in fixtures or docs.
- The model defaults are Gemini 3.8 Flash High (`gemini-3.8-flash-high`). Never make a Claude or GPT model a default.
- Allow rules must never cover arbitrary program runners (`xargs`, `awk`, `sh -c`, `bash -c`, `env`, `curl`, `python -c`, `node -e`, `find -exec` and similar). Risky flags always need a decision.
- Never write to agy's own CLI or IDE configuration (`~/.gemini`). Account homes are the app's own slot folders.
- Agent calls that do not need tools (such as dictation and the verifier) must not skip permissions (W-010).
- Multi-agent workflows are ways to use the product, not product code. Do not build orchestration features for them.
- Do not add references to third-party usage trackers in code, UI, docs or commits. Leave existing links as they are.
- A proposed additive `chat` CLI command is not approved. Do not add it.
- Tests and experiments write only under `~/Desktop/test`.
- Git: work on a branch from `rust` and push only that branch. Never push to `main`, never rewrite pushed history, never merge into `main`, and never delete the legacy app. Cutover needs the owner's explicit OK after every row is Passed.
- Write code, comments, docs, UI text and commit messages in normal English. Match the surrounding comment density and naming. Write docs as short, plain sentences.
- Svelte 5: use `$state.raw` for objects changed outside Svelte (xterm instances, recorders, connections). Deep `$state` proxies break them.
- The UI client (`ui/src/lib/api.ts`) refuses routes that the generated catalog does not mark implemented. When you add a route with a new path parameter, extend the catalog-path normalization there.
- Raw-body routes (uploads) are handled before the JSON dispatch in `crates/server/src/http.rs`. Read request headers into owned values before an `.await`, or the handler is not `Send`.
- Run `cargo fmt --all` after scripted edits; scripted string replacements often miss code that rustfmt has reformatted.
- Playwright `launchOptions` are per worker, so give each microphone or browser-flag setup its own spec file (see `e2e/media-support.ts`).
