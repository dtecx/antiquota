# Rewrite integration tests

This directory owns reusable test support for the Rust rewrite. It does not change the captured Python baseline. The server integration entry point is `crates/server/tests/rewrite_contract.rs`; `tests/fake-agy` supplies the test executable. All subprocesses, account tokens, scripts, and project edits live in disposable directories beneath the current user's `Desktop/test`. Tests never start real agy or read real account credentials.

Build both binaries before running the integration test:

```sh
cargo build -p antiquota-cli -p fake-agy
cargo test -p antiquota-server --test rewrite_contract
```

The harness sets `ANTIQUOTA_HOME`, `ANTIQUOTA_DIR`, `ANTIQUOTA_AGY`, `ANTIQUOTA_NO_USAGE_LINK=1`, and a fixture UI directory explicitly. The parent server retains the OS HOME only to resolve the allowed Desktop/test boundary; all application data uses the explicit isolated home and data roots. The seeded account uses synthetic token bytes with mode `0600` on Unix. Fake agy receives that account's isolated HOME and SSH environment; its invocation record excludes token bytes. Every test owns its server process and removes its disposable directory when finished. Evidence under `.handoff/evidence/` is ignored by Git.

## Captured contract coverage

`slice1-scope.json` lists all 75 captured routes individually. There are 22 distinct selected captured cases: an 18-case primary sequence, successful send/stop/delete cases in their controlled scenarios, and the `events-sse` case. That SSE case contributes eight selected first-turn samples. These assertions cover all ten Slice 1 endpoints. The 65 other endpoints remain individually pending; full chronological replay of all 172 cases is not claimed. The primary sequence covers health, state, authentication, Host validation, static containment, malformed and oversized bodies, session creation/get/update, invalid session inputs, empty sends, and shutdown. Separate whole-server tests cover queued sends, crash recovery, discovery, CLI execution, shutdown with an open SSE client, and exact restart replay. `coverage-proposal.json` maps precise passing tests and evidence to candidate feature IDs, with unproved acceptance behavior explicitly pending; it does not update the canonical report.

The first-turn SSE comparison uses original sample indices `7, 10, 12, 13, 14, 15, 16, 17` from `events-sse.json`. It checks every field in user, turn-start, both tool states, both deltas, assistant, and turn-end samples, including the file diff and usage. Each sample keeps its original JSON-pointer rules. Global account, approval, goal, job, thinking, and terminal events belong to later slices. The control scenario binds `SESSION_1` from its actual created session; the independent deletion scenario binds `SESSION_2` from another fresh created session. It verifies the captured deletion route response without claiming the original fork preparation, which is pending.

Comparison preserves exact object keys, array lengths, nulls, stable numbers, and strings. Only each fixture's explicit numeric JSON pointers are dynamic. Unknown comparison rules fail. Identities bind from the manifest's response pointers before later requests; paths stay exact after replacing the isolated test root. Recorded raw bodies are decoded from base64, preserving malformed JSON. Response `Content-Length` must match actual encoded bytes, and `Date` must be a valid HTTP date. The implementation's `Server` header may differ as recorded by the captured contract.

JSON numbers compare by exact decimal value: `0` and `0.0` are equal. The comparator removes insignificant zeros and combines decimal scale with the integer exponent; it never converts equality comparisons to `f64`. Distinct large integers such as `9007199254740993` and `9007199254740992`, different close decimals, and different exponent values fail. Exponents beyond the supported integer range remain equal only if their literal representations are identical. No numeric field is removed, and neither API nor data semantics change. Focused tests prove equality, inequality, signed zero, arbitrary-precision digits, and exponent handling. Dynamic timestamp/process rules retain their explicit numeric constraints.

SSE parsing decodes HTTP transport chunks first. It preserves JSON lines and UTF-8 bytes when an HTTP chunk splits them. Tests compare event content and durable replay, rather than transport chunk boundaries. Polls require both the expected turn count and its `turn_end` before treating a session as idle.

`state-initial` recreates its recorded two-account setup from the captured quota-file bytes. The executable is copied to the isolated root, preserving the captured agy path. The test holds the first account's actual file lease while checking `busy=true`, then releases it before session creation. No account or quota field is excluded. Full quota probing and routing behavior remain pending for the account slice.

## Fake agy scenarios

Set `FAKE_AGY_SCENARIO` to `success`, `delay`, `quota`, `rate`, `auth`, `approval`, `hook`, `background`, `goal_complete`, `huge`, `crash`, `stale_result`, or `duplicate_done`. The default success turn emits actual agy `event=init`, `event=step_update`, and `event=result` envelopes. Current-step usage is deliberately smaller than cumulative result usage. Resume ids stay stable. A successful synthetic file edit happens after the delay, immediately before the tool's completed frame is emitted. Cancelling during the delay prevents the edit.

`FAKE_AGY_SCRIPT` points to an isolated JSON file matching the Rust `Script` type. It can select scenarios by slot or turn, supply explicit event/delay actions, change usage or response text, and set exit codes. Normal tool progress waits 200 ms; `delay` waits 10 seconds unless `delay_ms` overrides it. `huge` defaults to 2 MB and is capped at 8 MB. The fake metadata commands, quota, sign-in, verifier, and dictation responses have no provider cost. Hook scenarios invoke only an explicitly supplied `antiquota` or `fake-hook` executable and never execute the proposed tool command themselves.

Scenario emission tests prove the fake protocol. They do not establish completed application support for goals, approvals, handoff, jobs, dictation, or other pending slices.
