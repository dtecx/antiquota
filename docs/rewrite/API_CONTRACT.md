# Legacy HTTP contract

This contract was captured from the current Python engine before the rewrite. The capture uses a real `ThreadingHTTPServer`, two synthetic signed-in slots, the existing `tests/stub_agy.py`, a synthetic dictation response, local git repositories, and a real local PTY. It makes no paid model calls and never changes the user's account slots, settings, project repositories, or CLI configuration.

The authoritative route list and capture provenance are in [`tests/fixtures/api/manifest.json`](../../tests/fixtures/api/manifest.json). Every one of the 75 API routes has a successful HTTP fixture. Error and alternate-state fixtures are also retained. These are legacy observations, not proof that the Rust rewrite passes any test.

## Reproduce the capture

Before cutover, run:

```sh
python3 tests/fixtures/api/capture.py
```

The helper recreates only `~/Desktop/test/antiquota-contract`, starts an ephemeral loopback server, and shuts it down after the scenario. `capture.py` owns setup, HTTP transport, normalization, and the completeness manifest. `scenario.py` owns the ordered API interactions and local preparation steps. The existing stub spends no quota. The dictation wrapper emits either `Captured speech.` or `NO_SPEECH`; the captured bytes exercise the raw transport and are not a live speech-quality test.

The scenario signs in `one@example.com` and `two@example.com` with a synthetic single-line code, seeds quota readings with 90% remaining and fixed 2099 reset dates, creates an initial `main` git commit, and disables keep-awake. Its files, fake token bytes, and process paths are confined to the test directory. Original unnormalized captures and the capture log remain in ignored `.handoff/evidence/`.

## Fixture and replay rules

Each case records the actual request method, path, effective wire headers (including the HTTP library's Host, Accept-Encoding, and Content-Length), JSON or base64 bytes; the actual response status, headers, and JSON, text, or base64 bytes; and a comparison policy. When replay substitutes paths or serializes JSON, recompute ordinary request Content-Length from the encoded body; preserve the deliberate oversized `declaredContentLength` fixtures. Capture order is `manifest.replaySequence`. Replay that sequence against one fresh isolated server, rather than replaying filenames alphabetically. The API is stateful: a fork inherits the preceding transcript, a rewind removes checkpoints, and later git operations depend on earlier commits.

`manifest.setup` records initial accounts, directory roots, fake-agy environment transitions, non-HTTP preparation, and waits. The exact preparation is executable in the two capture helpers. Wait for completion after turns and goal rounds; release the probe lease before the idle account check. Write the worktree fixture file before its git status/commit cases. The thinking scenario creates a synthetic agy SQLite row, then obtains the thinking event over real HTTP. The quota scenario makes the selected fake account return an actual quota error and captures the resulting handoff.

String placeholders preserve cross-request identities. Bind `{{TOKEN}}`, `{{PORT}}`, `{{TEST_ROOT}}`, and `{{REPO_ROOT}}` from the test environment. Bind session, job, approval, login, conversation, and commit placeholders from actual preceding responses using `manifest.bindings`. Discover any additional placeholders in response identity fields before comparing later fields. Both approval ids and conversation ids have explicit binding sources. Short commit ids and full base commits remain distinct bindings. `{{WORKTREE_PATH}}` comes from `worktree-create`'s cwd. Bind the worktree branch from that same response and verify its relationship to the title and abbreviated session id.

Only the exact JSON-pointer paths listed in each case's `comparison.dynamicValues` are dynamic. Check their specified type and meaning instead of comparing the captured instant or process id. All field names, nulls, stable numeric values, limits, quota fractions, rounds, statuses, booleans, and strings remain part of the contract. In particular, daily schedule `at` is an exact `HH:MM` string, not a globally ignored key. Durations must remain nonnegative; process ids must be positive integers.

HTTP `Date` is a valid HTTP date. `Server` identifies the implementation and will change with the new server. `Content-Length`, when present, must equal the actual encoded body length, since JSON serialization whitespace can change without changing the JSON value. Content type, cache policy, authentication decisions, and `X-Content-Type-Options` remain semantic assertions. SSE samples retain actual `data:` framing; compare parsed `eventSamples` through the same pointer rules. Stream ordering within a session is preserved, while unrelated global account updates can interleave.

`manifest.calendar` records the capture's local date, IANA timezone, and observed UTC offset. Replay with a fake clock at that local date/timezone, or compare each usage day relative to the replay's local date while preserving row order, account splits, counts, and totals. A capture's date strings must not freeze the product to that day. Terminal SSE chunk boundaries depend on scheduling: decode and concatenate output in order, check initial metadata and the final exit marker, and assert the typed command/result. Shell prompts and their escape sequences vary by platform.

Static source bytes are retained as capture evidence. Replacing the JS/CSS/HTML build is explicitly required by the rewrite; static parity checks assert serving behavior, content type, token bootstrap, and security boundaries rather than identical legacy source bytes. The generated hook command also changes from a Python script path to the Rust binary's `hook` entry point; its role and isolation remain part of the contract.

## Transport and security

The server listens on `127.0.0.1`. An exact `Host` of `127.0.0.1:<port>` or `localhost:<port>` is required even for health and static requests. A bad host receives HTTP 403 with `{"error":"bad host"}`. The health route needs no token and reveals only `ok`, application name, version, and pid.

Other API routes require the per-install `X-Antiquota-Token` header. SSE, terminal SSE, and attachment downloads instead require the `token` query parameter; the legacy server ignores the header for those routes. Missing or wrong credentials receive 403 with `{"error":"forbidden"}`. The root page substitutes its token bootstrap; static serving needs no token after the host check. Only index, app JS/CSS, and single-level allowed vendor JS/CSS files are served. Traversal and unknown paths are refused.

Ordinary request bodies are JSON, with an empty body treated as `{}`. The size ceiling is 5,000,000 bytes. Raw attachment and transcription bodies have a 25,000,000-byte ceiling. Raw audio type is `X-Audio-Type`, default `wav`; attachment names use URL-decoded `X-Filename`. Fixtures include rejected oversized lengths without allocating or transmitting oversized data.

Successful JSON routes return 200. `KeyError` returns 404, `ValueError` returns 400, and missing files return 404. Unknown routes return 404. Other Python exceptions are not uniformly converted into JSON errors; the rewrite must define recovery for such failures without treating an uncaught legacy failure as acceptable behavior.

## Streaming

The global stream is `/api/events?token=...`. It emits JSON in `data: <json>\n\n` frames, with `: ping\n\n` after an idle interval. There is no SSE event id or persisted replay cursor in the legacy protocol. Live events include `sid` and timestamp. Persisted session events omit the enclosing `sid`; transient deltas and metadata updates are not persisted in the old transcript log.

The capture covers account updates, metadata, user messages, turn start/end, tools, deltas, assistant replies, thinking, notices, approvals, jobs, shell output, and deletion. Notice variants include checks, verifier verdicts, goal completion, rewind, handoff, and an all-paused quota wall. Some background updates can arrive while another request is executing. SSE sample counts and event kinds can be inspected directly in `events-sse.json`.

The terminal stream first sends base64 backlog plus `shell`, `cwd`, and `alive`. Later frames carry base64 output `d`; termination sends `{"exit":true}`. It uses the same token-query authentication and idle ping framing. The fixture includes real shell output and termination, with input, resize, restart, and kill driven through HTTP.

## Deliberate correction ledger

| Legacy observation | Required rewrite behavior | Evidence |
| --- | --- | --- |
| An attachment containing a space uploads as `contract note.txt`, but downloading its URL-encoded name returns 404 because the path segment is never decoded. | Decode the name once, then apply canonical containment and file validation. This is the requested bug repair B-021; encoded-name downloads should succeed. | `attachment-upload.json`, `attachment-download.json`, `attachment-download-plain.json` |

No feature removal, route removal, data migration, or unrelated response-shape change is approved by this document. Additional intentional contract changes must be recorded and reviewed under the user's task rules.

## Route coverage

The table below is generated from the legacy match block and special route handlers. Each feature id links to the exhaustive inventory; fixture names link to real captures. Representative errors for a route are listed in its manifest entry.

| Feature | Method | Route | Successful captured fixture |
| --- | --- | --- | --- |
| [F-323](FEATURES.md) | `GET` | `/api/health` | [health](../../tests/fixtures/api/cases/health.json) |
| [F-324](FEATURES.md) | `GET` | `/api/events` | [events-sse](../../tests/fixtures/api/cases/events-sse.json) |
| [F-325](FEATURES.md) | `GET` | `/api/sessions/{sid}/term/stream` | [terminal-sse](../../tests/fixtures/api/cases/terminal-sse.json) |
| [F-326](FEATURES.md) | `POST` | `/api/transcribe` | [transcribe](../../tests/fixtures/api/cases/transcribe.json) |
| [F-327](FEATURES.md) | `POST` | `/api/sessions/{sid}/attach` | [attachment-upload](../../tests/fixtures/api/cases/attachment-upload.json) |
| [F-328](FEATURES.md) | `GET` | `/api/sessions/{sid}/attachments/{name}` | [attachment-download-plain](../../tests/fixtures/api/cases/attachment-download-plain.json) |
| [F-329](FEATURES.md) | `GET` | `/api/state` | [state-initial](../../tests/fixtures/api/cases/state-initial.json) |
| [F-330](FEATURES.md) | `POST` | `/api/approvals` | [approval-request](../../tests/fixtures/api/cases/approval-request.json) |
| [F-331](FEATURES.md) | `GET` | `/api/approvals/{aid}` | [approval-wait](../../tests/fixtures/api/cases/approval-wait.json) |
| [F-332](FEATURES.md) | `POST` | `/api/approvals/{aid}/answer` | [approval-answer](../../tests/fixtures/api/cases/approval-answer.json) |
| [F-333](FEATURES.md) | `POST` | `/api/sessions/{sid}/fork` | [session-fork](../../tests/fixtures/api/cases/session-fork.json) |
| [F-334](FEATURES.md) | `GET` | `/api/search` | [search](../../tests/fixtures/api/cases/search.json) |
| [F-335](FEATURES.md) | `POST` | `/api/accounts/{name}/pause` | [account-pause](../../tests/fixtures/api/cases/account-pause.json) |
| [F-336](FEATURES.md) | `POST` | `/api/sessions/{sid}/steer` | [session-steer](../../tests/fixtures/api/cases/session-steer.json) |
| [F-337](FEATURES.md) | `GET` | `/api/engine` | [engine](../../tests/fixtures/api/cases/engine.json) |
| [F-338](FEATURES.md) | `GET` | `/api/engine/log` | [engine-log](../../tests/fixtures/api/cases/engine-log.json) |
| [F-339](FEATURES.md) | `POST` | `/api/shutdown` | [shutdown](../../tests/fixtures/api/cases/shutdown.json) |
| [F-340](FEATURES.md) | `POST` | `/api/sessions` | [session-create](../../tests/fixtures/api/cases/session-create.json) |
| [F-341](FEATURES.md) | `POST` | `/api/sessions/{sid}/term` | [terminal-create](../../tests/fixtures/api/cases/terminal-create.json) |
| [F-342](FEATURES.md) | `POST` | `/api/sessions/{sid}/term/input` | [terminal-input](../../tests/fixtures/api/cases/terminal-input.json) |
| [F-343](FEATURES.md) | `POST` | `/api/sessions/{sid}/term/resize` | [terminal-resize](../../tests/fixtures/api/cases/terminal-resize.json) |
| [F-344](FEATURES.md) | `POST` | `/api/sessions/{sid}/term/kill` | [terminal-kill](../../tests/fixtures/api/cases/terminal-kill.json) |
| [F-345](FEATURES.md) | `GET` | `/api/sessions/{sid}/tree` | [session-tree](../../tests/fixtures/api/cases/session-tree.json) |
| [F-346](FEATURES.md) | `GET` | `/api/sessions/{sid}/git` | [session-git](../../tests/fixtures/api/cases/session-git.json) |
| [F-347](FEATURES.md) | `GET` | `/api/sessions/{sid}/git/diff` | [session-git-diff](../../tests/fixtures/api/cases/session-git-diff.json) |
| [F-348](FEATURES.md) | `POST` | `/api/sessions/{sid}/git/commit` | [session-git-commit](../../tests/fixtures/api/cases/session-git-commit.json) |
| [F-349](FEATURES.md) | `POST` | `/api/sessions/{sid}/git/discard` | [session-git-discard](../../tests/fixtures/api/cases/session-git-discard.json) |
| [F-350](FEATURES.md) | `POST` | `/api/sessions/{sid}/git/merge` | [worktree-merge](../../tests/fixtures/api/cases/worktree-merge.json) |
| [F-351](FEATURES.md) | `POST` | `/api/sessions/{sid}/git/remove` | [worktree-remove](../../tests/fixtures/api/cases/worktree-remove.json) |
| [F-352](FEATURES.md) | `GET` | `/api/sessions/{sid}` | [session-get-empty](../../tests/fixtures/api/cases/session-get-empty.json) |
| [F-353](FEATURES.md) | `PATCH` | `/api/sessions/{sid}` | [session-patch](../../tests/fixtures/api/cases/session-patch.json) |
| [F-354](FEATURES.md) | `DELETE` | `/api/sessions/{sid}` | [session-delete](../../tests/fixtures/api/cases/session-delete.json) |
| [F-355](FEATURES.md) | `POST` | `/api/sessions/{sid}/send` | [send](../../tests/fixtures/api/cases/send.json) |
| [F-356](FEATURES.md) | `POST` | `/api/sessions/{sid}/rewind` | [session-rewind](../../tests/fixtures/api/cases/session-rewind.json) |
| [F-357](FEATURES.md) | `POST` | `/api/sessions/{sid}/clear` | [session-clear](../../tests/fixtures/api/cases/session-clear.json) |
| [F-358](FEATURES.md) | `POST` | `/api/sessions/{sid}/jobs` | [job-create](../../tests/fixtures/api/cases/job-create.json) |
| [F-359](FEATURES.md) | `POST` | `/api/sessions/{sid}/jobs/{jid}/stop` | [job-stop](../../tests/fixtures/api/cases/job-stop.json) |
| [F-360](FEATURES.md) | `GET` | `/api/sessions/{sid}/jobs/{jid}/log` | [job-log](../../tests/fixtures/api/cases/job-log.json) |
| [F-361](FEATURES.md) | `POST` | `/api/sessions/{sid}/loop` | [loop-create](../../tests/fixtures/api/cases/loop-create.json) |
| [F-362](FEATURES.md) | `DELETE` | `/api/sessions/{sid}/loop` | [loop-delete](../../tests/fixtures/api/cases/loop-delete.json) |
| [F-363](FEATURES.md) | `GET` | `/api/sessions/{sid}/state` | [session-state](../../tests/fixtures/api/cases/session-state.json) |
| [F-364](FEATURES.md) | `POST` | `/api/sessions/{sid}/inherit` | [session-inherit](../../tests/fixtures/api/cases/session-inherit.json) |
| [F-365](FEATURES.md) | `POST` | `/api/sessions/{sid}/shell` | [session-shell](../../tests/fixtures/api/cases/session-shell.json) |
| [F-366](FEATURES.md) | `POST` | `/api/sessions/{sid}/note` | [session-note](../../tests/fixtures/api/cases/session-note.json) |
| [F-367](FEATURES.md) | `GET` | `/api/sessions/{sid}/file` | [session-file](../../tests/fixtures/api/cases/session-file.json) |
| [F-368](FEATURES.md) | `POST` | `/api/sessions/{sid}/compact` | [session-compact](../../tests/fixtures/api/cases/session-compact.json) |
| [F-369](FEATURES.md) | `GET` | `/api/sessions/{sid}/files` | [session-files](../../tests/fixtures/api/cases/session-files.json) |
| [F-370](FEATURES.md) | `POST` | `/api/sessions/{sid}/stop` | [session-stop](../../tests/fixtures/api/cases/session-stop.json) |
| [F-371](FEATURES.md) | `POST` | `/api/sessions/{sid}/goal` | [goal-create](../../tests/fixtures/api/cases/goal-create.json) |
| [F-372](FEATURES.md) | `POST` | `/api/sessions/{sid}/goal/resume` | [goal-resume](../../tests/fixtures/api/cases/goal-resume.json) |
| [F-373](FEATURES.md) | `DELETE` | `/api/sessions/{sid}/goal` | [goal-delete](../../tests/fixtures/api/cases/goal-delete.json) |
| [F-374](FEATURES.md) | `GET` | `/api/usage` | [usage-empty](../../tests/fixtures/api/cases/usage-empty.json) |
| [F-375](FEATURES.md) | `GET` | `/api/routing` | [routing](../../tests/fixtures/api/cases/routing.json) |
| [F-376](FEATURES.md) | `GET` | `/api/accounts` | [accounts-initial](../../tests/fixtures/api/cases/accounts-initial.json) |
| [F-377](FEATURES.md) | `POST` | `/api/accounts/probe` | [accounts-probe](../../tests/fixtures/api/cases/accounts-probe.json) |
| [F-378](FEATURES.md) | `POST` | `/api/accounts/login` | [login-start](../../tests/fixtures/api/cases/login-start.json) |
| [F-379](FEATURES.md) | `GET` | `/api/accounts/login/{lid}` | [login-poll](../../tests/fixtures/api/cases/login-poll.json) |
| [F-380](FEATURES.md) | `POST` | `/api/accounts/login/{lid}` | [login-submit](../../tests/fixtures/api/cases/login-submit.json) |
| [F-381](FEATURES.md) | `DELETE` | `/api/accounts/login/{lid}` | [login-delete](../../tests/fixtures/api/cases/login-delete.json) |
| [F-382](FEATURES.md) | `POST` | `/api/accounts/{name}/check` | [account-check-idle](../../tests/fixtures/api/cases/account-check-idle.json) |
| [F-383](FEATURES.md) | `DELETE` | `/api/accounts/{name}` | [account-delete](../../tests/fixtures/api/cases/account-delete.json) |
| [F-384](FEATURES.md) | `GET` | `/api/settings` | [settings](../../tests/fixtures/api/cases/settings.json) |
| [F-385](FEATURES.md) | `PUT` | `/api/settings/defaults` | [defaults](../../tests/fixtures/api/cases/defaults.json) |
| [F-386](FEATURES.md) | `GET` | `/api/settings/file` | [settings-file-missing](../../tests/fixtures/api/cases/settings-file-missing.json) |
| [F-387](FEATURES.md) | `PUT` | `/api/settings/file` | [settings-file-put](../../tests/fixtures/api/cases/settings-file-put.json) |
| [F-388](FEATURES.md) | `GET` | `/api/settings/{kind}/{name}` | [settings-hook-get](../../tests/fixtures/api/cases/settings-hook-get.json) |
| [F-389](FEATURES.md) | `PUT` | `/api/settings/{kind}/{name}` | [settings-hook-put](../../tests/fixtures/api/cases/settings-hook-put.json) |
| [F-390](FEATURES.md) | `DELETE` | `/api/settings/{kind}/{name}` | [settings-hook-delete](../../tests/fixtures/api/cases/settings-hook-delete.json) |
| [F-391](FEATURES.md) | `POST` | `/api/settings/{kind}/{name}/enabled` | [settings-hook-disable](../../tests/fixtures/api/cases/settings-hook-disable.json) |
| [F-392](FEATURES.md) | `POST` | `/api/settings/copy` | [settings-copy-rule](../../tests/fixtures/api/cases/settings-copy-rule.json) |
| [F-393](FEATURES.md) | `GET` | `/api/stack` | [stack](../../tests/fixtures/api/cases/stack.json) |
| [F-394](FEATURES.md) | `POST` | `/api/stack/reseed` | [stack-reseed](../../tests/fixtures/api/cases/stack-reseed.json) |
| [F-395](FEATURES.md) | `GET` | `/api/permissions` | [permissions](../../tests/fixtures/api/cases/permissions.json) |
| [F-396](FEATURES.md) | `PUT` | `/api/permissions` | [permissions-put](../../tests/fixtures/api/cases/permissions-put.json) |
| [F-397](FEATURES.md) | `GET` | `/api/fs` | [filesystem](../../tests/fixtures/api/cases/filesystem.json) |
