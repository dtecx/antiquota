# Existing data and compatibility contract

The rewrite must open the existing `~/.antiquota` directly, preserve all five signed-in accounts, and retain every session and configuration item. The observations below combine the legacy readers/writers with a read-only inspection of the current installation. Synthetic examples were captured from the isolated HTTP scenario in [`tests/fixtures/data/`](../../tests/fixtures/data/); they contain no real credentials.

The data fixture manifest records the source path and capture phase for actual stack snapshots: permission JSON, hooks JSON, MCP JSON, rule text, skill Markdown, plugin enablement JSON/resources, and global instructions. These files preserve the isolated stack's bytes and whitespace except path/identity normalization. They were read after the specified real HTTP writes/copies and before deletion or reseeding, not reconstructed from an expected schema.

## Backup and continuity evidence

Before any new engine touched real data, the required `cp -a` backup was created locally at:

```text
/Users/dtecx/.antiquota.bak-2026-10-07-152006
```

Five signed-in token files were found. Each token was byte-identical in the backup and had mode `0600`. Token hashes, slot names, and the conversation-store link snapshot are retained only in ignored `.handoff/evidence/account-continuity-before.json`. The backup contains tokens and must remain local; neither it nor private continuity evidence belongs in git or an uploaded artifact. The backup preserves symbolic links as links.

After compatibility tests and before cutover, compare account identities, exact token-file hashes, ownership/modes, shared conversation targets, and session counts against this snapshot. An account refresh may legitimately change token bytes; account identity and readable sign-in state must still hold, with that refresh recorded privately. Until the user approves cutover, the Python checkout and its data remain the rollback path. Stop all engine writers before restoring files; do not run a backup as a second engine against links that still point at the live folder.

## Root resolution

| Purpose | Default | Override or behavior |
| --- | --- | --- |
| Real user home | OS account home on POSIX; normal home resolution on Windows | `ANTIQUOTA_HOME` isolates tests. Account subprocess `HOME` must not redefine the owning user's home. |
| Data root | `~/.antiquota` | `ANTIQUOTA_DIR` overrides the entire data root. |
| Shared agy stack | `<base>/stack` | Owned by antiquota. The user's CLI and IDE roots are read-only sources for explicit copies. |
| Account slots | `<base>/slots/<name>` | Visible signed-in slots are sorted, non-hidden directories containing the auth file. |
| Session data | `<base>/sessions/<id>` | Twelve hexadecimal characters are used by current newly-created sessions. Existing ids must be preserved. |
| Worktree directories | `<base>/worktrees/<repository-name>-<session-id>` | Existing paths and branch records are part of session metadata. |

The legacy reader can move an older predecessor data folder when the current root does not exist, then preserve its old location with a link. Existing links and transcript paths from that history must remain readable. No new destructive relocation is authorized by this rewrite.

## Sessions

`sessions/<id>/meta.json` is a UTF-8 JSON object replaced atomically. The reader overlays legacy missing keys with defaults and resets the execution status to `idle`; it preserves goal and job data. Preserve unknown fields rather than silently discarding a newer or older writer's data. Do not share mutable default arrays/objects between sessions.

| Field | Stored form and meaning |
| --- | --- |
| `id`, `title`, `cwd`, `model`, `effort`, `mode` | Strings; mode is `default`, `accept-edits`, `plan`, or `bypass`. `cwd` is an absolute project or worktree path. |
| `created`, `updated` | Numeric Unix timestamps in seconds, often fractional. |
| `status` | Execution/status string; saved states include idle, waiting, running, sleeping, approval, checking, and verifying. |
| `status_until` | Numeric deadline/reset instant or null. |
| `conversation_id`, `last_slot` | Strings or null. Conversation ids name files in the shared store. |
| `turns`, `context` | Integer counters; context is the largest observed prompt/cache footprint of the turn. |
| `usage` | Object with integer `input`, `output`, and `thinking` totals; older data may omit `thinking`. The rewrite adds an optional `cache`, the cached input across turns, written only when above zero. A session loaded without it rebuilds it from its turn ends. |
| `queue` | Array mixing plain mid-turn user strings and objects containing `text`, `attachments`, and `kind`. |
| `pending_note` | String inserted into a later turn for compaction, inheritance, shell output, or rewind. |
| `goal`, `loop`, `worktree` | Structured objects described below, or null. |
| `jobs` | Array of job records; current writer keeps the last 20. |
| `pinned`, `archived` | Booleans independent of execution state. |

Goal records contain `objective`, `check`, `verifier`, `verifier_model`, `max_rounds`, `round`, and `state`; retain any additional unknown fields. States include active, paused, met, and failed. Legacy `_verify` falls back to the session model when the stored verifier model is empty. The rewrite must use the required `gemini-3.8-flash-high` default for new unspecified verifier choices while preserving explicit stored choices. An active saved goal is not proof that a worker is currently executing.

Loop records contain `every` seconds, `prompt`, and `next` Unix timestamp. A daily schedule also stores `at` as local `HH:MM`, with `every` 86400. Both local-time scheduling and daylight-saving changes require tests; `at` is not a timestamp field. Jobs carry `id`, shell `cmd`, `pid`, absolute `log` path, `started`, nullable `ended`/`code`, integer `every`, boolean `notify`, `by` (`user` or `agent`), and boolean `stopped`.

Worktree records contain `repo`, `path`, `branch`, `base`, and `base_sha`. Removing a worktree clears that record, resets the session cwd/conversation, archives the session, and records an explanatory pending note. Deleting a session does not authorize deletion of unrelated project files.

`events.jsonl` is one UTF-8 JSON object per line. Each durable event carries `t` and `ts`, with fields determined by its kind. Tool events can include parameters, file paths, diff, before/after text, errors, and timings; final replies and turn usage are durable. The old file omits the enclosing SSE `sid`, metadata broadcasts, and transient text deltas. The Rust reader must preserve the old transcript and support mixed old/new entries through explicit decoding, without rewriting it just to normalize representation.

`checkpoints/<turn>.json` maps absolute file paths to `{before, after}` text. Empty text represents either an empty or previously absent file in the old writer; the rewrite must not silently assume this is a lossless distinction. Rewind restores the earliest before-value for the requested turn range and removes consumed checkpoints. `STATE.md` is a generated UTF-8 handoff summary. `attachments/` stores sanitized uploaded filenames, suffixing collisions (`name-2.ext`, etc.). `jobs/<id>.log` is process output. There is no JSON manifest for attachments; names, sizes, and image classification are derived from files.

## Accounts and configuration links

Each slot is a per-account HOME. Its auth path is:

```text
slots/<account>/.gemini/antigravity-cli/antigravity-oauth-token
```

The current real token is JSON with top-level `token`, `auth_method`, and `id_token`. The nested token has `access_token`, `token_type`, `refresh_token`, and `expiry`. Treat the entire file as opaque credential bytes: preserve it exactly, allow agy to own refreshes, and never print it. All five current files are regular, mode `0600`. The optional neighboring `jetski_state.pbtxt` is private account state. `installation_id`, token backups, process logs/cache/crashes, and per-process state are also private entries, never links to another account.

The slot rebuilds `.gemini` and selected children as directories, linking each non-private entry to antiquota's stack. Other HOME entries link to the owning user's home. File-token isolation requires `HOME` and `USERPROFILE` set to that slot and an `SSH_CONNECTION` value; it applies equally to turns, quota/model probes, login, verifier, and dictation. Never read/write the user's agy keyring or edit its CLI/IDE configuration to make the rewrite work.

The stack owns shared `conversations`, `brain`, `annotations`, and `implicit` directories. A conversation is a SQLite file `conversations/<conversation-id>.db`. Its `steps` table includes `idx`, `step_type`, and protobuf `step_payload`. Reply step type 15 contains planner field 20; reply text is field 20.1 and thinking is field 20.3. Rust must retain these stores, open them read-only for thinking, and let agy own writes. A missing, locked, malformed, or not-yet-written row is recoverable.

The conversation store link is `~/.config/antigravity` pointing to `<base>/stack/.gemini/antigravity-cli`. Keep an existing correct link. Do not replace an unrelated existing directory or valid foreign link. Repair only a dangling link owned by antiquota when its destination has moved. Windows uses symbolic links where permitted, otherwise directory junctions or file hard links/copies; test these path/permission semantics rather than assuming POSIX behavior.

`slots/.login-*` directories are temporary sign-in HOMEs. A successful sign-in adopts only account credential/state files, then retains leftover sign-in evidence in the slot's `.antiquota-login` directory. Cancellation/failure removes the temporary directory and can leave a redacted failure log. Slot names are validated, and busy account removal must fail while its lease is held.

## Settings, rules, skills, and hooks

The optional root `settings.json` stores GUI/engine defaults as strings, including numeric settings. Its absence means use defaults; it was absent in the current real installation. Keep the known settings and unknown data on read/write. The defaults are model/verifier `gemini-3.8-flash-high`, mode `default`, rotation `perishable`, keep-awake `on`, reserve `3`, quota polling `10`, approval timeout `15`, maximum rounds `20`, auto-continue `2`, cwd empty, and worktree default `off`. The rewrite adds `placement` (`fill` or `spread`, default `fill`) on the owner's request; the default is not written, so a file without the key reads as `fill` and the legacy app ignores the key.

The shared stack is `stack/.gemini`. Permission lists live in `antigravity-cli/settings.json` under `permissions.allow` and `permissions.deny`. Values are rule strings, with duplicates removed by the API. Preserve unrelated settings keys. Arbitrary runner commands must never be newly added to allow lists; risky flags and shell composition remain part of the permission policy.

| Kind | Stack format |
| --- | --- |
| Skill | `config/skills/<name>/SKILL.md` plus its sibling resources. |
| Rule | `config/rules/<name>` text file. |
| Plugin | `config/plugins/<name>` directory; related private plugin data in `antigravity-cli/plugin_data/<name>`, with enablement in `config/config.json`. |
| Hook | Named JSON object in `config/hooks.json`; preserves `enabled` and event arrays. |
| MCP | Named object under `mcpServers` in `config/mcp_config.json`; command/args/environment or URL configuration. |
| Global instructions | `GEMINI.md` text; absolute copied config paths are rewritten only into the owned stack. |

The `antiquota` hook entry contains PreToolUse and PreInvocation configuration and preserves an explicit disabled state. The rewrite's `hook` binary entry point replaces the old script command without changing unrelated hooks. Never remove or copy back changes into the user's CLI/IDE roots.

## Runtime state

| Path under `state/` | Format and compatibility requirement |
| --- | --- |
| `gui-token` | Persisted token text, at least 32 characters; mode `0600` when created. Reuse it so open browser tabs survive restart. |
| `engine.json` | Discovery object: `pid`, `port`, `url`, `version`, `started`, and launcher `by`. Replace atomically only after health is ready; stale files are not sufficient evidence of a running engine. |
| `engine.lock` | Exclusive OS file lock, mode `0600` when created. One engine per data folder. |
| `engine.log` | UTF-8 log, with replacement decoding on read; API exposes its last 400 lines. |
| `models.json` | Cached array of `{id, label}` model rows; survive offline model discovery. |
| `leases/<slot>.lock` | Per-account exclusive lock, mode `0600` when created. No simultaneous token refresh/process use for the same HOME. |
| `quota/<slot>.json` | `{at, quota}`; pools `gemini`/`cg` map window names to `[remaining_fraction, reset_epoch_or_null]`. Disabled windows can have zero fraction and null reset. |
| `walls/<slot>.<pool>` | Text: `<integer-expiry> <reason>\n`. Expired/malformed walls are inactive; preserve the readable reason. |
| `paused/<slot>` | Empty marker file means the slot is excluded from automatic rotation. |
| `verify/<slot>.json` | `{url, since}` for a signed-in account needing Google verification; retain token and parked state. |
| `usage.jsonl` | One turn row: `t`, `slot`, `model`, `session`, `input`, `output`, and optional `thinking`. The rewrite adds an optional `cache`, the input tokens agy read from its cache (`cache_read_tokens`), written only when above zero. Local-day aggregation tolerates bad lines, and an unreadable `cache` counts as zero. Day, account and model totals include `cache`. |
| `routing.jsonl` | Append-only decisions with `ts`, `event`, and variable contextual fields (slot, bucket, session, previous, reason). Current writer trims at about 2 MB to the latest 5,000 lines. |
| `calls.jsonl` | CLI call ledger with `t`, `slot`, `bucket`, `rc`, `secs`, and boolean `walled`; optional when only GUI turns were run. |
| `failures/` | Captured sign-in/call diagnostics; may include private identifiers, so retain locally and redact before sharing. |
| `dictation/` | Temporary audio files deleted after transcription success/failure; cleanup must survive interruption. |

Generic JSON files currently use atomic temp-and-replace writes, with pid/thread-specific temp names for concurrent writers. Existing file modes outside credentials vary and must not be assumed to be `0600`. Preserve permissions and symlink targets on compatibility reads; use private modes for new credential/lock files. Test interrupted writes, malformed JSON/JSONL, unknown fields, missing directories, concurrent readers/writers, and platform locking.

Short rate-limit strike counts are derived from recent `cooldown` events in `routing.jsonl`; there is no separate strike-state file. Preserve that history so restart does not erase escalation.

## Acceptance boundary

The captured examples and private continuity snapshot establish the input contract. They do not authorize format replacement or prove migration parity. Required rewrite evidence includes loading these examples unchanged, replaying old transcript data, preserving settings/rules and account isolation, verifying Windows paths and locks, and comparing all five real sign-ins before and after authorized live tests. Any incompatible format change requires the user's decision and a reversible plan.
