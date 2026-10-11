# Feature inventory and rewrite acceptance

This is the canonical checklist for parity and cutover. It contains 1009 baseline acceptance items: 397 engine/API/CLI items (F-001–F-397) and 612 UI/shell items (F-398–F-1009), including the requested zoom and window-persistence repairs. The 143 additional items (F-1010–F-1152) record explicit requirements from the user's rewrite task and the owner's later requests. They are requirements, not claims about legacy behavior. No feature has been dropped.

Every row is independently reviewable and links to its [test report](TEST_REPORT.md). Slices 1, 3–8, 9a–9f, 11–15, 16, 16b, 17a, 17b and 18 and the proved Slice 10 subset have **884 Passed** acceptance items; the other **268 remain Pending**. Legacy test names and captured Python responses explain the baseline; they are not passing Rust or TypeScript proof. Mark a row complete only after its passing tests and evidence have been recorded. Store logs, screenshots and live-cost records in ignored `.handoff/evidence/`.

Legacy sources were read in full at commit `17ec296`; [engine](source-read-engine.md) and [UI/shell](source-read-ui.md) manifests record coverage and hashes. The sourced subsidiary inventories are [FEATURES-engine.md](FEATURES-engine.md) and [FEATURES-ui.md](FEATURES-ui.md). The canonical rows below contain their full acceptance text. UI identifiers were renumbered once from F-400–F-1011 to F-398–F-1009 to remove the unused gap; engine identifiers were unchanged.

Read [known-bugs.md](known-bugs.md), [API_CONTRACT.md](API_CONTRACT.md) and [DATA_FORMATS.md](DATA_FORMATS.md) alongside this checklist. API fixtures preserve observed wire responses, including defects; tests must assert the documented correction where the rewrite is required to repair a bug. Source-only concerns remain explicitly unverified until reproduced.

The authorized changes replace legacy Python/PyInstaller packaging with the Rust sidecar and PCM-only recording with direct MediaRecorder. They preserve equivalent user behavior, shared-engine discovery, useful audio input and silence protection. Native zoom and window persistence must meet the requested acceptance rather than reproduce the existing defects. On the owner's request of 2026-10-09, several sessions may run on one account at once, and a placement setting fills one account first (the default) or spreads new sessions; the usage view also shows cached input, and the manual Probe now button is gone because quotas refresh by themselves. No merge, deletion of the old stack, or push is authorized by this checklist; cutover still requires the user's OK after every item is proven.

## Engine, API and CLI baseline

### Storage and compatibility

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-001 | [ ] <a id="f-001"></a>Locate real HOME from ANTIQUOTA_HOME, otherwise OS account home on POSIX and user home on Windows, independent of a slot HOME. | `antiquota/accounts.py:65` | `tests/test_accounts.py::test_every_agy_call_keeps_the_token_in_the_account_folder` | [Pending](TEST_REPORT.md#f-001) |
| F-002 | [x] <a id="f-002"></a>Use ANTIQUOTA_DIR when supplied; otherwise read the existing ~/.antiquota layout. | `antiquota/accounts.py:75` | `tests/conftest.py:29` | [Passed](TEST_REPORT.md#f-002) |
| F-003 | [ ] <a id="f-003"></a>Preserve existing session metadata, transcripts, checkpoints, attachments, jobs, worktrees and account files without migration loss. | `antiquota/engine.py:248`; `antiquota/accounts.py:164` | `tests/test_engine.py::test_turn_streams_persists_and_survives_restart` | [Pending](TEST_REPORT.md#f-003) |
| F-004 | [x] <a id="f-004"></a>Support the old state-folder rename only when the destination does not exist, the source is a real directory and no migration has run already. | `antiquota/accounts.py:86` | `tests/test_accounts.py::test_old_agyloop_folder_moves_to_antiquota` | [Passed](TEST_REPORT.md#f-004) |
| F-005 | [x] <a id="f-005"></a>Rewrite links and path-bearing own-stack configuration during the old-folder rename; retain a compatibility link and make retries idempotent. | `antiquota/accounts.py:93` | `tests/test_accounts.py::test_old_agyloop_folder_moves_to_antiquota` | [Passed](TEST_REPORT.md#f-005) |
| F-006 | [ ] <a id="f-006"></a>Keep one private file token and jetski_state.pbtxt per account; preserve all five existing signed-in slots. | `antiquota/accounts.py:40`; `antiquota/accounts.py:564` | `tests/test_accounts.py::test_accounts_are_named_by_email_and_use_their_own_stack` | [Pending](TEST_REPORT.md#f-006) |
| F-007 | [ ] <a id="f-007"></a>Share conversations, brain, annotations and implicit folders among slots for conversation continuity. | `antiquota/accounts.py:190` | `tests/test_accounts.py::test_accounts_are_named_by_email_and_use_their_own_stack` | [Pending](TEST_REPORT.md#f-007) |
| F-008 | [ ] <a id="f-008"></a>Maintain the conversation store link at ~/.config/antigravity when absent or already pointing to the own store; leave another owner's live path unchanged. | `antiquota/accounts.py:259` | `tests/test_accounts.py::test_conversation_store_link` | [Pending](TEST_REPORT.md#f-008) |
| F-009 | [ ] <a id="f-009"></a>Allow ANTIQUOTA_NO_USAGE_LINK to suppress creation of the conversation store link. | `antiquota/accounts.py:267` | `tests/test_accounts.py::test_conversation_store_link` | [Pending](TEST_REPORT.md#f-009) |
| F-010 | [ ] <a id="f-010"></a>Replace a dangling conversation store link with the current own-store target. | `antiquota/accounts.py:269` | Not covered | [Pending](TEST_REPORT.md#f-010) |
| F-011 | [x] <a id="f-011"></a>Use per-process and per-thread temporary names for atomic state replacement, then rename to the final file. | `antiquota/accounts.py:738` | Not covered | [Passed](TEST_REPORT.md#f-011) |
| F-012 | [x] <a id="f-012"></a>Create per-account and daemon lock files with mode `0600`; create the slots directory with mode `0700` during sign-in. | `antiquota/accounts.py:445`; `antiquota/accounts.py:758`; `antiquota/server.py:635` | Not covered | [Passed](TEST_REPORT.md#f-012) |
| F-013 | [ ] <a id="f-013"></a>Use POSIX symlinks; on Windows fall back to directory junctions, file hard links, then file copies when rights require it. | `antiquota/accounts.py:147` | Not covered | [Pending](TEST_REPORT.md#f-013) |
| F-014 | [ ] <a id="f-014"></a>Read malformed or missing quota, wall, verification, routing and optional settings caches using their documented empty/fallback behavior. | `antiquota/accounts.py:664`; `antiquota/accounts.py:864`; `antiquota/stack.py:44` | Not covered | [Pending](TEST_REPORT.md#f-014) |
### Accounts and sign-in

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-015 | [ ] <a id="f-015"></a>List only non-hidden slot names containing a file token, sorted by account name. | `antiquota/accounts.py:172` | `tests/test_accounts.py::test_accounts_are_named_by_email_and_use_their_own_stack` | [Pending](TEST_REPORT.md#f-015) |
| F-016 | [ ] <a id="f-016"></a>Resolve agy through ANTIQUOTA_AGY or the expanded binary search path, including ~/.local/bin, /opt/homebrew/bin, /usr/local/bin, the inherited PATH and the Windows install location. | `antiquota/accounts.py:126` | Not covered | [Pending](TEST_REPORT.md#f-016) |
| F-017 | [ ] <a id="f-017"></a>Report a missing agy binary with an actionable ANTIQUOTA_AGY message. | `antiquota/accounts.py:134` | Not covered | [Pending](TEST_REPORT.md#f-017) |
| F-018 | [ ] <a id="f-018"></a>Every agy call supplies per-account HOME and USERPROFILE plus SSH_CONNECTION to select file-token storage. | `antiquota/accounts.py:141` | `tests/test_accounts.py::test_every_agy_call_keeps_the_token_in_the_account_folder` | [Pending](TEST_REPORT.md#f-018) |
| F-019 | [ ] <a id="f-019"></a>Clone own-stack configuration from the real CLI config without copying account identity, private process state, conversations or IDE configuration. | `antiquota/accounts.py:181`; `antiquota/accounts.py:228` | `tests/test_accounts.py::test_stack_is_separate_from_your_agy_cli` | [Pending](TEST_REPORT.md#f-019) |
| F-020 | [ ] <a id="f-020"></a>Rewrite absolute config, CLI, GEMINI.md and settings paths into the own stack while leaving IDE paths unchanged. | `antiquota/accounts.py:196` | `tests/test_stack.py::test_copy_items_in_and_rewrite_paths_into_the_stack` | [Pending](TEST_REPORT.md#f-020) |
| F-021 | [x] <a id="f-021"></a>Reseeding replaces own configuration while preserving shared conversations and other excluded history. | `antiquota/accounts.py:228` | `tests/test_accounts.py::test_stack_is_separate_from_your_agy_cli` | [Passed](TEST_REPORT.md#f-021) |
| F-022 | [ ] <a id="f-022"></a>Synchronize each slot's own .gemini farm as real directories, keeping token/state/log/cache/presence/updater entries private. | `antiquota/accounts.py:305` | `tests/test_accounts.py::test_accounts_are_named_by_email_and_use_their_own_stack` | [Pending](TEST_REPORT.md#f-022) |
| F-023 | [ ] <a id="f-023"></a>Link other HOME entries from real HOME, exclude a recursive link to the state root and remove dangling slot links. | `antiquota/accounts.py:300` | `tests/test_accounts.py::test_stack_is_separate_from_your_agy_cli` | [Pending](TEST_REPORT.md#f-023) |
| F-024 | [ ] <a id="f-024"></a>Never write through a symlink while creating a slot farm directory. | `antiquota/accounts.py:309` | Not covered | [Pending](TEST_REPORT.md#f-024) |
| F-025 | [ ] <a id="f-025"></a>Report unshared entries and identity mismatch in doctor without changing the real CLI token. | `antiquota/accounts.py:325`; `antiquota/__main__.py:74` | `tests/test_accounts.py::test_accounts_are_named_by_email_and_use_their_own_stack` | [Pending](TEST_REPORT.md#f-025) |
| F-026 | [x] <a id="f-026"></a>Inspect loaded skills/hooks through a signed-in slot; with no accounts, inspect the own-stack folders instead of waiting for sign-in. | `antiquota/accounts.py:337` | Not covered | [Passed](TEST_REPORT.md#f-026) |
| F-027 | [ ] <a id="f-027"></a>Validate account identifiers against the 1–100 character account-name allowlist, rejecting hidden names and traversal. | `antiquota/accounts.py:375` | `tests/test_server.py::test_add_account_from_the_gui` | [Pending](TEST_REPORT.md#f-027) |
| F-028 | [x] <a id="f-028"></a>Start sign-in in a temporary empty HOME under slots and create no final account on failure. | `antiquota/accounts.py:431` | `tests/test_accounts.py::test_sign_in_needs_the_right_code_and_never_half_creates_an_account` | [Passed](TEST_REPORT.md#f-028) |
| F-029 | [ ] <a id="f-029"></a>Use a POSIX PTY or Windows ConPTY for agy's authorization-code input; Windows without the optional console implementation supports browser-only completion. | `antiquota/accounts.py:380`; `antiquota/accounts.py:454` | Not covered | [Pending](TEST_REPORT.md#f-029) |
| F-030 | [x] <a id="f-030"></a>Capture only agy's Google sign-in URL and allow wait_url to return when the URL appears or the process ends. | `antiquota/accounts.py:369`; `antiquota/accounts.py:489` | `tests/test_server.py::test_add_account_from_the_gui` | [Passed](TEST_REPORT.md#f-030) |
| F-031 | [ ] <a id="f-031"></a>Accept only a trimmed, nonempty, single-line authorization code of at most 2000 characters with no CR/LF/NUL; reject submission after completion. | `antiquota/accounts.py:496` | `tests/test_server.py::test_add_account_from_the_gui` | [Pending](TEST_REPORT.md#f-031) |
| F-032 | [x] <a id="f-032"></a>Support sign-in states waiting, done, failed and verify; report error, adopted account name and verification URL. | `antiquota/accounts.py:517` | `tests/test_accounts.py::test_an_account_google_wants_verified_is_kept_and_parked` | [Passed](TEST_REPORT.md#f-032) |
| F-033 | [ ] <a id="f-033"></a>Adopt a successful account using the email in agy's auth log, move only account files, retain other sign-in artifacts in .antiquota-login and clear old walls/verification. | `antiquota/accounts.py:523`; `antiquota/accounts.py:564` | `tests/test_accounts.py::test_accounts_are_named_by_email_and_use_their_own_stack` | [Pending](TEST_REPORT.md#f-033) |
| F-034 | [x] <a id="f-034"></a>Preserve a valid signed-in token when Google requires verification, store the continue URL and park both quota buckets for 30 days until a successful probe. | `antiquota/accounts.py:544`; `antiquota/accounts.py:593` | `tests/test_accounts.py::test_an_account_google_wants_verified_is_kept_and_parked` | [Passed](TEST_REPORT.md#f-034) |
| F-035 | [ ] <a id="f-035"></a>Cancel or replace a pending sign-in, kill its process, clean temporary data and retain redacted failure evidence. | `antiquota/accounts.py:511`; `antiquota/accounts.py:551`; `antiquota/server.py:135` | `tests/test_server.py::test_a_new_sign_in_link_replaces_the_old_one` | [Pending](TEST_REPORT.md#f-035) |
| F-036 | [x] <a id="f-036"></a>Permit only one active web sign-in link; a replaced link reports failed on GET and is rejected on POST. | `antiquota/server.py:135`; `antiquota/server.py:487` | `tests/test_server.py::test_a_new_sign_in_link_replaces_the_old_one` | [Passed](TEST_REPORT.md#f-036) |
| F-037 | [x] <a id="f-037"></a>Remove a valid existing account only after acquiring its lease; reject removal while busy and clear both bucket walls. | `antiquota/accounts.py:646` | `tests/test_accounts.py::test_sign_in_needs_the_right_code_and_never_half_creates_an_account` | [Passed](TEST_REPORT.md#f-037) |
| F-038 | [ ] <a id="f-038"></a>Pause/unpause an account out of automatic rotation, retain named/pinned access, persist the marker and record routing history. | `antiquota/accounts.py:673` | `tests/test_engine.py::test_paused_accounts_sit_out_and_sessions_pin_fork_search` | [Pending](TEST_REPORT.md#f-038) |
| F-039 | [x] <a id="f-039"></a>Expose account busy/probed/quota/walls/reasons/paused/verify/today fields without exposing credentials. | `antiquota/accounts.py:719` | `tests/test_server.py::test_page_carries_token_and_api_demands_it` | [Passed](TEST_REPORT.md#f-039) |
### Quota, routing and usage

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-040 | [x] <a id="f-040"></a>Classify models beginning with claude or gpt into cg; all other/default models use gemini; accept both --model M and --model=M. | `antiquota/accounts.py:952` | `tests/test_accounts.py::test_run_is_sticky_and_fails_over` | [Passed](TEST_REPORT.md#f-040) |
| F-041 | [x] <a id="f-041"></a>Parse real quota groups into gemini/cg windows of remaining fraction and reset epoch. | `antiquota/accounts.py:962` | `tests/test_accounts.py::test_parse_real_quota_output` | [Passed](TEST_REPORT.md#f-041) |
| F-042 | [x] <a id="f-042"></a>Treat a disabled quota window with no fraction/reset as empty, using another window's reset for its wall. | `antiquota/accounts.py:965` | `tests/test_accounts.py::test_parse_quota_when_weekly_is_spent_and_5h_disabled` | [Passed](TEST_REPORT.md#f-042) |
| F-043 | [ ] <a id="f-043"></a>Probe /quota without model tokens, with a 90 second timeout; cache timestamp and parsed windows. | `antiquota/accounts.py:976` | `tests/test_accounts.py::test_probe_walls_spent_basket_until_panel_reset` | [Pending](TEST_REPORT.md#f-043) |
| F-044 | [x] <a id="f-044"></a>Wall a bucket when any window has at most 2% left; choose latest applicable reset plus 120 seconds, or fallback reset when absent. | `antiquota/accounts.py:995` | `tests/test_accounts.py::test_probe_walls_spent_basket_until_panel_reset` | [Passed](TEST_REPORT.md#f-044) |
| F-045 | [x] <a id="f-045"></a>Use the 5 hour 10 minute fallback wall when quota cannot be read after a quota error. | `antiquota/accounts.py:51`; `antiquota/accounts.py:805` | Not covered | [Passed](TEST_REPORT.md#f-045) |
| F-046 | [x] <a id="f-046"></a>Clear a bucket wall and verification marker after a healthy quota reading. | `antiquota/accounts.py:992` | `tests/test_accounts.py::test_an_account_google_wants_verified_is_kept_and_parked` | [Passed](TEST_REPORT.md#f-046) |
| F-047 | [x] <a id="f-047"></a>Park a signed-out account for 30 days in both buckets until sign-in/probe repairs it. | `antiquota/accounts.py:855` | `tests/test_engine.py::test_temporary_errors_retry_then_move_on_and_bad_sign_in_parks` | [Passed](TEST_REPORT.md#f-047) |
| F-048 | [x] <a id="f-048"></a>Keep gemini and cg walls independent, ignore expired walls and allow explicit unwall of both buckets. | `antiquota/accounts.py:664`; `antiquota/__main__.py:103` | `tests/test_accounts.py::test_run_is_sticky_and_fails_over` | [Passed](TEST_REPORT.md#f-048) |
| F-049 | [ ] <a id="f-049"></a>Acquire an account lease across processes and release every lease after the call. Legacy leases were exclusive; on the owner's request (2026-10-09, F-1148) model calls share the lease, while probes, checks and removal need it alone and skip a busy account. | `antiquota/accounts.py:758` | `tests/test_accounts.py::test_busy_account_is_skipped` | [Pending](TEST_REPORT.md#f-049) |
| F-050 | [x] <a id="f-050"></a>Wait up to 10 seconds for a session's preferred busy account before considering another live account; continue polling leases until stopped. | `antiquota/accounts.py:772` | Not covered | [Passed](TEST_REPORT.md#f-050) |
| F-051 | [x] <a id="f-051"></a>Stop acquisition promptly when the supplied cancellation event is set; return no account when all candidates are walled. | `antiquota/accounts.py:775` | `tests/test_engine.py::test_stop_ends_the_turn_and_pauses_the_goal` | [Passed](TEST_REPORT.md#f-051) |
| F-052 | [x] <a id="f-052"></a>Refresh only idle accounts with a quota snapshot older than max_age, returning whether a valid update occurred. | `antiquota/accounts.py:704` | Not covered | [Passed](TEST_REPORT.md#f-052) |
| F-053 | [x] <a id="f-053"></a>Use quota_poll minutes for freshness, clamped to at least 120 seconds, with 600 seconds fallback for invalid input. | `antiquota/accounts.py:873` | Not covered | [Passed](TEST_REPORT.md#f-053) |
| F-054 | [x] <a id="f-054"></a>Calculate account headroom as the minimum window fraction and expiry as the latest known window reset; missing quota remains unknown. | `antiquota/accounts.py:881` | `tests/test_accounts.py::test_rank_spends_the_soonest_reset_first_but_keeps_a_warm_account` | [Passed](TEST_REPORT.md#f-054) |
| F-055 | [x] <a id="f-055"></a>Default perishable routing spends the allowance that resets soonest first, then prefers more headroom, with unknown expiry last. | `antiquota/accounts.py:893` | `tests/test_accounts.py::test_rank_spends_the_soonest_reset_first_but_keeps_a_warm_account` | [Passed](TEST_REPORT.md#f-055) |
| F-056 | [x] <a id="f-056"></a>Sticky routing follows sorted account order; round-robin follows least-recent pick in routing history. | `antiquota/accounts.py:912` | `tests/test_accounts.py::test_rank_spends_the_soonest_reset_first_but_keeps_a_warm_account` | [Passed](TEST_REPORT.md#f-056) |
| F-057 | [x] <a id="f-057"></a>Across every routing strategy, put accounts strictly below the reserve last and retain the preferred account first while above it. | `antiquota/accounts.py:914` | `tests/test_accounts.py::test_rank_spends_the_soonest_reset_first_but_keeps_a_warm_account` | [Passed](TEST_REPORT.md#f-057) |
| F-058 | [x] <a id="f-058"></a>Exclude paused or currently walled accounts from automatic ranking. | `antiquota/accounts.py:905` | `tests/test_engine.py::test_paused_accounts_sit_out_and_sessions_pin_fork_search` | [Passed](TEST_REPORT.md#f-058) |
| F-059 | [x] <a id="f-059"></a>Find the earliest reset across currently walled signed-in accounts for the requested bucket. | `antiquota/accounts.py:792` | Not covered | [Passed](TEST_REPORT.md#f-059) |
| F-060 | [x] <a id="f-060"></a>Classify only genuine agy error channels, excluding successful model response text and successful stderr chatter. | `antiquota/accounts.py:825` | `tests/test_accounts.py::test_only_agy_errors_wall_never_model_text; tests/test_accounts.py::test_classify_reads_only_agys_error_channels` | [Passed](TEST_REPORT.md#f-060) |
| F-061 | [x] <a id="f-061"></a>Apply classification precedence auth, then quota/rate/429, then transient network/capacity/5 xx/deadline errors. | `antiquota/accounts.py:831` | `tests/test_accounts.py::test_classify_reads_only_agys_error_channels` | [Passed](TEST_REPORT.md#f-061) |
| F-062 | [x] <a id="f-062"></a>Probe after a quota-looking error; distinguish exhausted allowance from a rate limit with remaining quota. | `antiquota/accounts.py:797` | `tests/test_engine.py::test_rate_limit_with_quota_left_only_rests_the_account` | [Passed](TEST_REPORT.md#f-062) |
| F-063 | [x] <a id="f-063"></a>Escalate repeated rate limits to 60, 300 and 900 second cooldowns, counting the previous hour's matching cooldown entries. | `antiquota/accounts.py:59`; `antiquota/accounts.py:809` | `tests/test_engine.py::test_rate_limit_with_quota_left_only_rests_the_account` | [Passed](TEST_REPORT.md#f-063) |
| F-064 | [x] <a id="f-064"></a>Record timestamped pick/wall/cooldown/retry/paused/resumed/signed-out decisions in routing.jsonl; skip malformed lines on reading. | `antiquota/accounts.py:925` | `tests/test_engine.py::test_temporary_errors_retry_then_move_on_and_bad_sign_in_parks` | [Passed](TEST_REPORT.md#f-064) |
| F-065 | [x] <a id="f-065"></a>Trim routing history above 2 MB to its latest 5000 lines; requested reads default to 200 entries internally and 60 through HTTP. | `antiquota/accounts.py:931`; `antiquota/server.py:478` | Not covered | [Passed](TEST_REPORT.md#f-065) |
| F-066 | [x] <a id="f-066"></a>Aggregate today's input/output/thinking/turn count by account using local midnight and tolerate malformed ledger lines. | `antiquota/accounts.py:1019` | Not covered | [Passed](TEST_REPORT.md#f-066) |
| F-067 | [x] <a id="f-067"></a>Aggregate last 14 days by default into local-day totals, turns, per-account tokens, account input/output/today and model totals, filling empty days. | `antiquota/accounts.py:1041` | `tests/test_engine.py::test_plan_mode_plans_by_instruction_and_usage_history` | [Passed](TEST_REPORT.md#f-067) |
| F-068 | [x] <a id="f-068"></a>Clamp HTTP usage-history day count to 1–90 and accept explicit routing n. | `antiquota/server.py:476` | Not covered | [Passed](TEST_REPORT.md#f-068) |
### Persisted defaults and validation

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-069 | [x] <a id="f-069"></a>Default model is gemini-3.8-flash-high; persisted model identifiers are returned exactly. | `antiquota/stack.py:314` | `tests/test_stack.py::test_gui_defaults_round_trip` | [Passed](TEST_REPORT.md#f-069) |
| F-070 | [x] <a id="f-070"></a>Default permission mode is default; session mode validation accepts default, accept-edits, plan and bypass only. | `antiquota/stack.py:314`; `antiquota/engine.py:37` | `tests/test_server.py::test_session_round_trip_over_http` | [Passed](TEST_REPORT.md#f-070) |
| F-071 | [x] <a id="f-071"></a>Default verifier model is gemini-3.8-flash-high. | `antiquota/stack.py:314` | Not covered | [Passed](TEST_REPORT.md#f-071) |
| F-072 | [x] <a id="f-072"></a>Default rotation is perishable; validate perishable, sticky and round-robin only. | `antiquota/stack.py:315`; `antiquota/stack.py:334` | Not covered | [Passed](TEST_REPORT.md#f-072) |
| F-073 | [x] <a id="f-073"></a>Default keep_awake is on; validate on/off only. | `antiquota/stack.py:315`; `antiquota/stack.py:336` | Not covered | [Passed](TEST_REPORT.md#f-073) |
| F-074 | [x] <a id="f-074"></a>Default reserve is string 3 percent; accept numeric values 0–50 inclusive. | `antiquota/stack.py:316`; `antiquota/stack.py:324` | Not covered | [Passed](TEST_REPORT.md#f-074) |
| F-075 | [x] <a id="f-075"></a>Default quota_poll is string 10 minutes; accept 2–240 inclusive. | `antiquota/stack.py:317`; `antiquota/stack.py:324` | Not covered | [Passed](TEST_REPORT.md#f-075) |
| F-076 | [x] <a id="f-076"></a>Default approval_timeout is string 15 minutes; accept 1–240 inclusive. | `antiquota/stack.py:318`; `antiquota/stack.py:324` | Not covered | [Passed](TEST_REPORT.md#f-076) |
| F-077 | [x] <a id="f-077"></a>Default max_rounds is string 20; accept 1–500 inclusive in settings. | `antiquota/stack.py:319`; `antiquota/stack.py:324` | Not covered | [Passed](TEST_REPORT.md#f-077) |
| F-078 | [x] <a id="f-078"></a>Default auto_continue is string 2; accept 0–10 inclusive. | `antiquota/stack.py:320`; `antiquota/stack.py:325` | Not covered | [Passed](TEST_REPORT.md#f-078) |
| F-079 | [x] <a id="f-079"></a>Default default_cwd is empty; expand user paths, reject a non-directory when saving, and expose home as state cwd fallback. | `antiquota/stack.py:321`; `antiquota/stack.py:347`; `antiquota/server.py:224` | Not covered | [Passed](TEST_REPORT.md#f-079) |
| F-080 | [x] <a id="f-080"></a>Default worktree_default is off; validate on/off only. | `antiquota/stack.py:322`; `antiquota/stack.py:336` | Not covered | [Passed](TEST_REPORT.md#f-080) |
| F-081 | [x] <a id="f-081"></a>Merge only recognized setting keys, stringify updates, normalize numeric strings and ignore unknown keys. | `antiquota/stack.py:328` | `tests/test_stack.py::test_gui_defaults_round_trip` | [Passed](TEST_REPORT.md#f-081) |
| F-082 | [ ] <a id="f-082"></a>Recover numeric defaults from missing/invalid setting values, while rejecting invalid numeric updates with a field-specific error. | `antiquota/stack.py:339`; `antiquota/stack.py:353` | Not covered | [Pending](TEST_REPORT.md#f-082) |
### Stack management

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-083 | [ ] <a id="f-083"></a>Inspect own-stack and read-only CLI/IDE roots; identify skills, plugins, rules, hooks and MCP servers from their kind-specific locations. | `antiquota/stack.py:17`; `antiquota/stack.py:30` | `tests/test_stack.py::test_overview_compares_with_your_cli_and_ide` | [Pending](TEST_REPORT.md#f-083) |
| F-084 | [x] <a id="f-084"></a>Require valid kinds, supported sources and names of 1–100 allowed characters that do not start with a dot. | `antiquota/stack.py:35` | `tests/test_stack.py::test_copy_items_in_and_rewrite_paths_into_the_stack` | [Passed](TEST_REPORT.md#f-084) |
| F-085 | [ ] <a id="f-085"></a>List skills only when their directory contains SKILL.md; omit hidden entries and metadata hook keys beginning $. | `antiquota/stack.py:89` | Not covered | [Pending](TEST_REPORT.md#f-085) |
| F-086 | [ ] <a id="f-086"></a>Compare files/folders by content digest and JSON entries after path rewriting; expose CLI/IDE same/differs/absent status. | `antiquota/stack.py:61`; `antiquota/stack.py:139` | `tests/test_stack.py::test_overview_compares_with_your_cli_and_ide` | [Pending](TEST_REPORT.md#f-086) |
| F-087 | [ ] <a id="f-087"></a>Show skill description frontmatter, hook event names and MCP URL/command summaries. | `antiquota/stack.py:120` | `tests/test_stack.py::test_overview_compares_with_your_cli_and_ide` | [Pending](TEST_REPORT.md#f-087) |
| F-088 | [ ] <a id="f-088"></a>Show hook, MCP and plugin enabled status using own config and agy mcp list; tolerate an unreadable MCP status command. | `antiquota/stack.py:112`; `antiquota/stack.py:155` | `tests/test_stack.py::test_edit_and_delete_touch_only_the_stack` | [Pending](TEST_REPORT.md#f-088) |
| F-089 | [ ] <a id="f-089"></a>Expose own GEMINI.md comparison and both allow/deny permissions with CLI comparison. | `antiquota/stack.py:165` | `tests/test_stack.py::test_edit_and_delete_touch_only_the_stack` | [Pending](TEST_REPORT.md#f-089) |
| F-090 | [x] <a id="f-090"></a>Copy one item or all items of a kind from supported CLI/IDE sources into the own stack; reject missing source entries. | `antiquota/stack.py:173` | `tests/test_stack.py::test_copy_items_in_and_rewrite_paths_into_the_stack` | [Passed](TEST_REPORT.md#f-090) |
| F-091 | [x] <a id="f-091"></a>Copy everything only from CLI by reseeding own configuration while preserving conversation history. | `antiquota/stack.py:176` | `tests/test_accounts.py::test_stack_is_separate_from_your_agy_cli` | [Passed](TEST_REPORT.md#f-091) |
| F-092 | [x] <a id="f-092"></a>Copy GEMINI.md and permissions only from CLI, preserving other own settings when replacing the permission object. | `antiquota/stack.py:181` | `tests/test_stack.py::test_edit_and_delete_touch_only_the_stack` | [Passed](TEST_REPORT.md#f-092) |
| F-093 | [x] <a id="f-093"></a>Copy plugin directories plus plugin data and corresponding own config entry. | `antiquota/stack.py:200` | Not covered | [Passed](TEST_REPORT.md#f-093) |
| F-094 | [x] <a id="f-094"></a>Copy hook JSON with rewritten script paths and copy supporting hook scripts. | `antiquota/stack.py:211`; `antiquota/stack.py:221` | `tests/test_stack.py::test_copy_items_in_and_rewrite_paths_into_the_stack` | [Passed](TEST_REPORT.md#f-094) |
| F-095 | [x] <a id="f-095"></a>Copy MCP JSON with rewritten paths and bring its existing connection state only when no own state exists. | `antiquota/stack.py:217` | `tests/test_stack.py::test_copy_items_in_and_rewrite_paths_into_the_stack` | [Passed](TEST_REPORT.md#f-095) |
| F-096 | [x] <a id="f-096"></a>Delete own files/directories or JSON entries by kind; deleting a plugin also removes own plugin data. | `antiquota/stack.py:227` | `tests/test_stack.py::test_edit_and_delete_touch_only_the_stack` | [Passed](TEST_REPORT.md#f-096) |
| F-097 | [x] <a id="f-097"></a>Get/put hooks and MCP servers as JSON objects; reject file-backed kinds or non-object values. | `antiquota/stack.py:245` | `tests/test_stack.py::test_edit_and_delete_touch_only_the_stack` | [Passed](TEST_REPORT.md#f-097) |
| F-098 | [x] <a id="f-098"></a>Toggle hooks directly; toggle MCP/plugins via agy enable/disable, reporting CLI failure; reject toggling rules or skills. | `antiquota/stack.py:261` | `tests/test_stack.py::test_edit_and_delete_touch_only_the_stack` | [Passed](TEST_REPORT.md#f-098) |
| F-099 | [ ] <a id="f-099"></a>Edit only real paths within own-stack root, rejecting escapes and protected conversation/history/private-state directories. | `antiquota/stack.py:277` | `tests/test_stack.py::test_edit_and_delete_touch_only_the_stack` | [Pending](TEST_REPORT.md#f-099) |
| F-100 | [x] <a id="f-100"></a>Read a missing editable file as path/text empty/exists false; decode existing text with replacement and reject files above 1 MB. | `antiquota/stack.py:287` | Not covered | [Passed](TEST_REPORT.md#f-100) |
| F-101 | [x] <a id="f-101"></a>Write text up to 1 million characters atomically; validate JSON syntax for .json destinations and create parent folders. | `antiquota/stack.py:297` | `tests/test_stack.py::test_edit_and_delete_touch_only_the_stack` | [Passed](TEST_REPORT.md#f-101) |
| F-102 | [x] <a id="f-102"></a>Read only known permission lists, accepting missing allow/deny as empty. | `antiquota/stack.py:367` | `tests/test_policy.py::test_policy_sorts_tools_by_risk` | [Passed](TEST_REPORT.md#f-102) |
| F-103 | [x] <a id="f-103"></a>Update allow/deny lists independently, require nonempty string rules below 300 characters, deduplicate in order and preserve unrelated config. | `antiquota/server.py:175` | `tests/test_server.py::test_permissions_are_agy_loops_own_and_folder_listing` | [Passed](TEST_REPORT.md#f-103) |
### Permission policy and shell parsing

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-104 | [x] <a id="f-104"></a>In bypass mode allow every tool without adding a permanent rule. | `antiquota/policy.py:44` | `tests/test_policy.py::test_policy_sorts_tools_by_risk` | [Passed](TEST_REPORT.md#f-104) |
| F-105 | [x] <a id="f-105"></a>Default/plan read, search and unknown bookkeeping tools are allowed. | `antiquota/policy.py:72` | `tests/test_policy.py::test_policy_sorts_tools_by_risk` | [Passed](TEST_REPORT.md#f-105) |
| F-106 | [x] <a id="f-106"></a>Classify run_command and git_commit through command rules; git_commit uses the git commit command when no explicit CommandLine exists. | `antiquota/policy.py:46` | Not covered | [Passed](TEST_REPORT.md#f-106) |
| F-107 | [x] <a id="f-107"></a>Ask for shell_exec, send_command_input, execute_notebook, run_extension_code and restart_dev_server unless bypassed. | `antiquota/policy.py:15`; `antiquota/policy.py:54` | Not covered | [Passed](TEST_REPORT.md#f-107) |
| F-108 | [x] <a id="f-108"></a>Ask for default-mode writes; accept-edits allows every named WRITE_TOOLS edit/move/delete/blob/notebook tool; plan denies these writes. | `antiquota/policy.py:17`; `antiquota/policy.py:56` | `tests/test_policy.py::test_policy_sorts_tools_by_risk` | [Passed](TEST_REPORT.md#f-108) |
| F-109 | [x] <a id="f-109"></a>Ask for deploy/database/setup/package-install/PR-review side effects unless bypassed. | `antiquota/policy.py:20`; `antiquota/policy.py:70` | `tests/test_policy.py::test_policy_sorts_tools_by_risk` | [Passed](TEST_REPORT.md#f-109) |
| F-110 | [x] <a id="f-110"></a>Match MCP rules from either ServerName/ToolName or server/tool fields; absent server requires approval without a suggested rule. | `antiquota/policy.py:24` | `tests/test_policy.py::test_policy_sorts_tools_by_risk` | [Passed](TEST_REPORT.md#f-110) |
| F-111 | [x] <a id="f-111"></a>Support exact MCP tool rules, server-wide mcp(server/*) and global mcp(*) allows; exact/server deny takes precedence. | `antiquota/policy.py:29` | `tests/test_policy.py::test_policy_sorts_tools_by_risk` | [Passed](TEST_REPORT.md#f-111) |
| F-112 | [x] <a id="f-112"></a>Match command rules by exact command or whole-word prefix; command(*) matches every parsed part but still respects RISKY. | `antiquota/stack.py:412` | `tests/test_policy.py::test_policy_sorts_tools_by_risk` | [Passed](TEST_REPORT.md#f-112) |
| F-113 | [x] <a id="f-113"></a>Apply command denies before allows, including loose splitting when shell parsing fails, and return explicit denial reason. | `antiquota/stack.py:418`; `antiquota/policy.py:49` | Not covered | [Passed](TEST_REPORT.md#f-113) |
| F-114 | [x] <a id="f-114"></a>Allow a compound shell line only if every command part is allowed and none is RISKY. | `antiquota/stack.py:412` | `tests/test_policy.py::test_policy_sorts_tools_by_risk` | [Passed](TEST_REPORT.md#f-114) |
| F-115 | [x] <a id="f-115"></a>Split shell operators &&,\|\|,;,pipe,&,\|&,;; and newline while preserving quoted operator characters. | `antiquota/stack.py:383`; `antiquota/stack.py:387` | `tests/test_policy.py::test_command_parts_respect_quotes_and_catch_hidden_commands` | [Passed](TEST_REPORT.md#f-115) |
| F-116 | [x] <a id="f-116"></a>Allow quoted pipe expressions such as grep -E with internal pipes when every real pipeline stage is allowed. | `antiquota/stack.py:393` | `tests/test_policy.py::test_command_parts_respect_quotes_and_catch_hidden_commands` | [Passed](TEST_REPORT.md#f-116) |
| F-117 | [x] <a id="f-117"></a>Allow a quoted semicolon search pattern and quoted sed -n range without creating extra commands. | `antiquota/stack.py:393` | `tests/test_policy.py::test_command_parts_respect_quotes_and_catch_hidden_commands` | [Passed](TEST_REPORT.md#f-117) |
| F-118 | [x] <a id="f-118"></a>Require approval for a hidden second command separated by newline or background ampersand. | `antiquota/stack.py:401` | `tests/test_policy.py::test_command_parts_respect_quotes_and_catch_hidden_commands` | [Passed](TEST_REPORT.md#f-118) |
| F-119 | [x] <a id="f-119"></a>Require approval for $() and backtick command substitution, regardless of an echo allow rule. | `antiquota/stack.py:391` | `tests/test_policy.py::test_command_parts_respect_quotes_and_catch_hidden_commands` | [Passed](TEST_REPORT.md#f-119) |
| F-120 | [x] <a id="f-120"></a>Require approval for unbalanced shell quotes and empty/unreadable command lines. | `antiquota/stack.py:395` | `tests/test_policy.py::test_command_parts_respect_quotes_and_catch_hidden_commands` | [Passed](TEST_REPORT.md#f-120) |
| F-121 | [x] <a id="f-121"></a>Allow output redirection to /dev/null plus descriptor redirection such as 2>&1; require approval for output redirection into real files. | `antiquota/stack.py:405` | `tests/test_policy.py::test_command_parts_respect_quotes_and_catch_hidden_commands` | [Passed](TEST_REPORT.md#f-121) |
| F-122 | [x] <a id="f-122"></a>A find base allow does not cover -exec, -execdir, -ok or -okdir program execution. | `antiquota/stack.py:378` | `tests/test_policy.py::test_allow_rules_never_cover_risky_flags` | [Passed](TEST_REPORT.md#f-122) |
| F-123 | [x] <a id="f-123"></a>A find base allow does not cover -delete or -fprint variants; safe find -name remains allowed. | `antiquota/stack.py:378` | `tests/test_policy.py::test_allow_rules_never_cover_risky_flags` | [Passed](TEST_REPORT.md#f-123) |
| F-124 | [x] <a id="f-124"></a>A git push base allow does not cover --force variants or -f; ordinary push remains subject to its own explicit allow rule. | `antiquota/stack.py:379` | `tests/test_policy.py::test_allow_rules_never_cover_risky_flags` | [Passed](TEST_REPORT.md#f-124) |
| F-125 | [x] <a id="f-125"></a>A git push base allow does not cover --delete, --mirror, -d or a :ref deletion. | `antiquota/stack.py:379` | `tests/test_policy.py::test_allow_rules_never_cover_risky_flags` | [Passed](TEST_REPORT.md#f-125) |
| F-126 | [x] <a id="f-126"></a>A sed base allow does not cover -i in-place editing. | `antiquota/stack.py:380` | `tests/test_policy.py::test_allow_rules_never_cover_risky_flags` | [Passed](TEST_REPORT.md#f-126) |
| F-127 | [x] <a id="f-127"></a>A git reset base allow does not cover --hard. | `antiquota/stack.py:380` | `tests/test_policy.py::test_allow_rules_never_cover_risky_flags` | [Passed](TEST_REPORT.md#f-127) |
| F-128 | [x] <a id="f-128"></a>A git clean base allow does not cover force flags; git checkout base allow does not cover checkout -- path. | `antiquota/stack.py:380` | Not covered | [Passed](TEST_REPORT.md#f-128) |
| F-129 | [x] <a id="f-129"></a>A git branch base allow does not cover -D; git stash base allow does not cover drop or clear. | `antiquota/stack.py:380` | `tests/test_policy.py::test_allow_rules_never_cover_risky_flags` | [Passed](TEST_REPORT.md#f-129) |
| F-130 | [x] <a id="f-130"></a>A mixed ls && find -delete line asks even when both base commands have allow rules. | `antiquota/stack.py:426` | `tests/test_policy.py::test_allow_rules_never_cover_risky_flags` | [Passed](TEST_REPORT.md#f-130) |
| F-131 | [x] <a id="f-131"></a>Suggest a two-word rule for listed package/VCS/runtime/build tools when the second token is not a flag; use one word for other commands. | `antiquota/stack.py:431` | `tests/test_policy.py::test_policy_sorts_tools_by_risk` | [Passed](TEST_REPORT.md#f-131) |
| F-132 | [x] <a id="f-132"></a>Do not append duplicate or empty permanent rules; keep new rules shared across accounts and sessions. | `antiquota/stack.py:444` | `tests/test_server.py::test_approvals_through_the_hook_and_messages_mid_turn` | [Passed](TEST_REPORT.md#f-132) |
| F-133 | [ ] <a id="f-133"></a>Rewrite requirement: reject permanent allows for arbitrary program runners at permission PUT, approval-always and stack-import/file-edit entry points; one-time approved execution remains distinct. | `antiquota/server.py:175`; `antiquota/engine.py:1129`; `antiquota/stack.py:173`; `antiquota/stack.py:297` | `Pending; required by handoff hard rule5` | [Pending](TEST_REPORT.md#f-133) |
### Hooks, approvals and steering

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-134 | [x] <a id="f-134"></a>Install the owned antiquota hook with PreToolUse matcher *,3600 second tool timeout and 15 second PreInvocation timeout; preserve enabled=false. | `antiquota/stack.py:470` | `tests/test_server.py::test_approvals_through_the_hook_and_messages_mid_turn` | [Passed](TEST_REPORT.md#f-134) |
| F-135 | [x] <a id="f-135"></a>Refresh installed hook command paths for source or bundled execution, quote platform command arguments and replace the old hook entry on upgrade. | `antiquota/stack.py:452`; `antiquota/stack.py:475` | Not covered | [Passed](TEST_REPORT.md#f-135) |
| F-136 | [x] <a id="f-136"></a>When the hook is enabled, run every session mode with skip-permissions while owned policy denies; when disabled, pass agy's original mode flags. | `antiquota/engine.py:1070` | `tests/test_engine.py::test_plan_mode_plans_by_instruction_and_usage_history` | [Passed](TEST_REPORT.md#f-136) |
| F-137 | [x] <a id="f-137"></a>Read engine discovery and GUI token for each hook API call; long-poll pending approvals in 25 second chunks with 35 second request timeout. | `antiquota/hook.py:19`; `antiquota/hook.py:22` | `tests/test_server.py::test_approvals_through_the_hook_and_messages_mid_turn` | [Passed](TEST_REPORT.md#f-137) |
| F-138 | [x] <a id="f-138"></a>An invocation without ANTIQUOTA_SESSION remains neutral: pre-tool asks, pre-invocation returns an empty object. | `antiquota/hook.py:70` | `tests/test_server.py::test_approvals_through_the_hook_and_messages_mid_turn` | [Passed](TEST_REPORT.md#f-138) |
| F-139 | [x] <a id="f-139"></a>Handle malformed hook stdin JSON as an empty payload and always emit one flushed JSON result with exit 0. | `antiquota/hook.py:75` | Not covered | [Passed](TEST_REPORT.md#f-139) |
| F-140 | [x] <a id="f-140"></a>When engine access fails, decide pre-tool using local own rules; deny an ask because no approver is available; if local checks also fail, deny safely. | `antiquota/hook.py:51`; `antiquota/hook.py:83` | `tests/test_server.py::test_approvals_through_the_hook_and_messages_mid_turn` | [Passed](TEST_REPORT.md#f-140) |
| F-141 | [x] <a id="f-141"></a>An unknown session approval request returns ask; a stopped known session denies an otherwise-pending tool call. | `antiquota/engine.py:1084` | Not covered | [Passed](TEST_REPORT.md#f-141) |
| F-142 | [x] <a id="f-142"></a>Resolve allowed/denied policy cases immediately without creating an approval card. | `antiquota/engine.py:1087` | `tests/test_server.py::test_approvals_through_the_hook_and_messages_mid_turn` | [Passed](TEST_REPORT.md#f-142) |
| F-143 | [x] <a id="f-143"></a>Create pending approvals with id/session/tool/args/rule/state/why/timestamp/deadline and set session status to approval. | `antiquota/engine.py:1092` | `tests/test_server.py::test_approvals_through_the_hook_and_messages_mid_turn` | [Passed](TEST_REPORT.md#f-143) |
| F-144 | [x] <a id="f-144"></a>Legacy pending-card args retain scalar values and truncate strings to 4000 characters; rewrite must present nested arguments faithfully per known-bug ledger. | `antiquota/engine.py:1092` | Not covered | [Passed](TEST_REPORT.md#f-144) |
| F-145 | [ ] <a id="f-145"></a>Poll approvals with requested wait capped at 60 seconds; missing ids return ask; unresolved requests return pending id. | `antiquota/engine.py:1102` | Not covered | [Pending](TEST_REPORT.md#f-145) |
| F-146 | [x] <a id="f-146"></a>Deny an unanswered approval after the configured timeout with an explanatory reason and wake every waiting hook. | `antiquota/engine.py:1112` | Not covered | [Passed](TEST_REPORT.md#f-146) |
| F-147 | [x] <a id="f-147"></a>Answer allow or deny idempotently; preserve the first decision and optional reason; reject other decisions at HTTP boundary. | `antiquota/engine.py:1121`; `antiquota/server.py:334` | `tests/test_server.py::test_approvals_through_the_hook_and_messages_mid_turn` | [Passed](TEST_REPORT.md#f-147) |
| F-148 | [x] <a id="f-148"></a>Always-allow adds a safe suggested command/MCP rule; always-allow on an unruled write switches that session to accept-edits. | `antiquota/engine.py:1129` | `tests/test_server.py::test_approvals_through_the_hook_and_messages_mid_turn` | [Passed](TEST_REPORT.md#f-148) |
| F-149 | [x] <a id="f-149"></a>Publish pending and answered approval events to transcript/SSE; return to running only after no other pending approval remains. | `antiquota/engine.py:1098`; `antiquota/engine.py:1134` | `tests/test_server.py::test_approvals_through_the_hook_and_messages_mid_turn` | [Passed](TEST_REPORT.md#f-149) |
| F-150 | [x] <a id="f-150"></a>Default denial tells the agent not to retry or reword the declined action; user reason overrides that explanation. | `antiquota/engine.py:36`; `antiquota/engine.py:1140` | `tests/test_server.py::test_approvals_through_the_hook_and_messages_mid_turn` | [Passed](TEST_REPORT.md#f-150) |
| F-151 | [x] <a id="f-151"></a>Stopping a session denies all its pending approvals, clears queued messages and terminates the active process. | `antiquota/engine.py:521` | `tests/test_engine.py::test_stop_ends_the_turn_and_pauses_the_goal` | [Passed](TEST_REPORT.md#f-151) |
| F-152 | [x] <a id="f-152"></a>PreInvocation removes only plain user-string messages from the queue, expands mentions and injects each as a userMessage; attachment/job messages stay queued. | `antiquota/engine.py:1146`; `antiquota/hook.py:65` | `tests/test_server.py::test_approvals_through_the_hook_and_messages_mid_turn` | [Passed](TEST_REPORT.md#f-152) |
| F-153 | [x] <a id="f-153"></a>Publish steered user messages exactly once with steered=true and return no injections when none remain. | `antiquota/engine.py:1154` | `tests/test_server.py::test_approvals_through_the_hook_and_messages_mid_turn` | [Passed](TEST_REPORT.md#f-153) |
### Sessions, turns and event stream

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-154 | [x] <a id="f-154"></a>Create sessions with 12 character ids, absolute expanded existing cwd, validated mode, model/effort/title and all fixed metadata keys. | `antiquota/engine.py:44`; `antiquota/engine.py:320` | `tests/test_engine.py::test_turn_streams_persists_and_survives_restart` | [Passed](TEST_REPORT.md#f-154) |
| F-155 | [x] <a id="f-155"></a>Initialize session timestamps, idle status, zero usage/context/turns, empty queue/jobs, no conversation/goal/loop/worktree and false pinned/archived. | `antiquota/engine.py:44`; `antiquota/engine.py:331` | `tests/test_engine.py::test_turn_streams_persists_and_survives_restart` | [Passed](TEST_REPORT.md#f-155) |
| F-156 | [x] <a id="f-156"></a>Load valid existing meta files on startup, skip missing/malformed files and add backward-compatible missing metadata/usage defaults. | `antiquota/engine.py:248` | `tests/test_engine.py::test_turn_streams_persists_and_survives_restart` | [Passed](TEST_REPORT.md#f-156) |
| F-157 | [x] <a id="f-157"></a>Reset loaded session status to idle without inventing a running process; retain persisted conversation/goal/queue/loop/job state for compatible recovery. | `antiquota/engine.py:259` | `tests/test_engine.py::test_turn_streams_persists_and_survives_restart` | [Passed](TEST_REPORT.md#f-157) |
| F-158 | [x] <a id="f-158"></a>Update only title/model/effort/mode/pinned/archived fields, coercing pins/archive to booleans and rejecting invalid mode. | `antiquota/engine.py:338` | `tests/test_engine.py::test_paused_accounts_sit_out_and_sessions_pin_fork_search` | [Passed](TEST_REPORT.md#f-158) |
| F-159 | [x] <a id="f-159"></a>Assign an untitled session the first user message's first line, capped at 60 characters, or attachment fallback. | `antiquota/engine.py:566` | `tests/test_engine.py::test_turn_streams_persists_and_survives_restart` | [Passed](TEST_REPORT.md#f-159) |
| F-160 | [ ] <a id="f-160"></a>Send immediately when idle and return started; when busy, persist FIFO text or structured attachment/kind queue and return queued. | `antiquota/engine.py:506` | `tests/test_engine.py::test_messages_sent_while_busy_are_queued` | [Pending](TEST_REPORT.md#f-160) |
| F-161 | [ ] <a id="f-161"></a>Decide the next queued message under the session lock so a message arriving as the worker exits is never stranded. | `antiquota/engine.py:612` | `tests/test_engine.py::test_messages_sent_while_busy_are_queued` | [Pending](TEST_REPORT.md#f-161) |
| F-162 | [ ] <a id="f-162"></a>Record each actual user/goal/nudge/job/loop message with attachments before its turn; inject pending shell/rewind/compact/inherit note before the current request. | `antiquota/engine.py:572` | `tests/test_engine.py::test_shell_note_and_file_viewer` | [Pending](TEST_REPORT.md#f-162) |
| F-163 | [x] <a id="f-163"></a>Give fresh conversations the background-job instruction; in plan mode with hook enabled prepend PLAN_NOTE without agy's headless plan mode. | `antiquota/engine.py:580` | `tests/test_engine.py::test_plan_mode_plans_by_instruction_and_usage_history` | [Passed](TEST_REPORT.md#f-163) |
| F-164 | [ ] <a id="f-164"></a>Increment turn count per process attempt and persist turn_start with account/model, status running and start timestamp. | `antiquota/engine.py:723` | `tests/test_engine.py::test_turn_streams_persists_and_survives_restart` | [Pending](TEST_REPORT.md#f-164) |
| F-165 | [ ] <a id="f-165"></a>Spawn agy -p --output-format stream-json with model, selected permission flags, existing conversation and attachment/job --add-dir access. | `antiquota/engine.py:716` | `tests/test_engine.py::test_attachments_reach_agy_through_add_dir` | [Pending](TEST_REPORT.md#f-165) |
| F-166 | [ ] <a id="f-166"></a>Supply session/slot/state/mode environment variables so hooks reach the same shared engine. | `antiquota/engine.py:730` | `tests/test_server.py::test_approvals_through_the_hook_and_messages_mid_turn` | [Pending](TEST_REPORT.md#f-166) |
| F-167 | [x] <a id="f-167"></a>Drain stdout linewise and stderr concurrently; ignore non-JSON stdout lines and update conversation id from init. | `antiquota/engine.py:736` | `tests/test_engine.py::test_turn_streams_persists_and_survives_restart` | [Passed](TEST_REPORT.md#f-167) |
| F-168 | [ ] <a id="f-168"></a>Stream response deltas live without persisting them; persist complete assistant messages once a response leaves ACTIVE. | `antiquota/engine.py:807` | `tests/test_engine.py::test_turn_streams_persists_and_survives_restart` | [Pending](TEST_REPORT.md#f-168) |
| F-169 | [ ] <a id="f-169"></a>Publish tool ACTIVE as running live; persist completed/error tool state, parameters, elapsed seconds, ANSI-stripped output/error capped at 20000 characters. | `antiquota/engine.py:821` | `tests/test_engine.py::test_turn_streams_persists_and_survives_restart` | [Pending](TEST_REPORT.md#f-169) |
| F-170 | [ ] <a id="f-170"></a>Capture before/after file content for edit tools, generate at most 600 unified-diff lines and handle fast-created files or late snapshots using remembered content. | `antiquota/engine.py:197`; `antiquota/engine.py:218`; `antiquota/engine.py:840` | `tests/test_engine.py::test_turn_streams_persists_and_survives_restart` | [Pending](TEST_REPORT.md#f-170) |
| F-171 | [ ] <a id="f-171"></a>Remember readable file views and edits in a 200 file FIFO working set; omit binary or over 1 MB snapshots from diffs/checkpoints. | `antiquota/engine.py:197`; `antiquota/engine.py:1226` | Not covered | [Pending](TEST_REPORT.md#f-171) |
| F-172 | [ ] <a id="f-172"></a>Read thinking from field `20.3` of agent-response step type 15 in a read-only SQLite conversation DB; missing/non-response/malformed payloads return empty. | `antiquota/convdb.py:50`; `antiquota/convdb.py:59` | `tests/test_convdb.py::test_thinking_comes_from_field_20_3_of_a_reply` | [Pending](TEST_REPORT.md#f-172) |
| F-173 | [ ] <a id="f-173"></a>Retry thinking retrieval after process completion when thinking tokens exist but the database write was delayed; persist thinking text/tokens by turn and step. | `antiquota/engine.py:768`; `antiquota/engine.py:851` | Not covered | [Pending](TEST_REPORT.md#f-173) |
| F-174 | [ ] <a id="f-174"></a>Count only DONE step usage for this turn; never count result's cumulative conversation totals; context is largest input plus cached-input count. | `antiquota/engine.py:757`; `antiquota/engine.py:782` | `tests/test_engine.py::test_a_reply_wins_over_the_resumed_conversations_old_error` | [Pending](TEST_REPORT.md#f-174) |
| F-175 | [x] <a id="f-175"></a>Append per-account session/model input/output/thinking usage ledger when token usage is nonzero. | `antiquota/engine.py:788` | `tests/test_engine.py::test_plan_mode_plans_by_instruction_and_usage_history` | [Passed](TEST_REPORT.md#f-175) |
| F-176 | [x] <a id="f-176"></a>A completed final response wins over stale quota/error fields in a resumed result; otherwise use genuine process/result error classification. | `antiquota/engine.py:773` | `tests/test_engine.py::test_a_reply_wins_over_the_resumed_conversations_old_error` | [Passed](TEST_REPORT.md#f-176) |
| F-177 | [ ] <a id="f-177"></a>Persist turn_end account/status/usage/elapsed time, with statuses stopped, quota, auth, transient, SUCCESS or other error/exit text. | `antiquota/engine.py:798` | `tests/test_engine.py::test_quota_wall_passes_the_turn_on_in_the_same_conversation` | [Pending](TEST_REPORT.md#f-177) |
| F-178 | [x] <a id="f-178"></a>Use latest streamed assistant reply as final turn text, falling back to result.response. | `antiquota/engine.py:803` | `tests/test_engine.py::test_turn_streams_persists_and_survives_restart` | [Passed](TEST_REPORT.md#f-178) |
| F-179 | [ ] <a id="f-179"></a>Report denied tool commands/actions; for successful non-goal turns send a bounded no-retry continuation, resetting the allowance for each actual user message. | `antiquota/engine.py:590`; `antiquota/engine.py:609`; `antiquota/engine.py:793` | `tests/test_engine.py::test_a_denied_command_gets_one_follow_up_that_names_it` | [Pending](TEST_REPORT.md#f-179) |
| F-180 | [x] <a id="f-180"></a>Missing accounts emit noaccount and return idle; all paused live accounts emit a wall explanation instead of waiting indefinitely. | `antiquota/engine.py:632`; `antiquota/engine.py:644` | `tests/test_engine.py::test_no_account_says_so; tests/test_engine.py::test_paused_accounts_sit_out_and_sessions_pin_fork_search` | [Passed](TEST_REPORT.md#f-180) |
| F-181 | [x] <a id="f-181"></a>Without an active goal, all-walled accounts report next reset and finish; active goals sleep until reset plus 30 seconds, with minimum 5 seconds and cancellation support. | `antiquota/engine.py:649` | `tests/test_engine.py::test_every_account_walled_reports_instead_of_hanging` | [Passed](TEST_REPORT.md#f-181) |
| F-182 | [x] <a id="f-182"></a>Handoff on exhausted quota or short rate limit in the same conversation using a continuation nudge because the original prompt is already stored. | `antiquota/engine.py:673` | `tests/test_engine.py::test_quota_wall_passes_the_turn_on_in_the_same_conversation` | [Passed](TEST_REPORT.md#f-182) |
| F-183 | [x] <a id="f-183"></a>Handoff on rejected auth while parking the failed account; retain conversation/nudge behavior. | `antiquota/engine.py:684` | `tests/test_engine.py::test_temporary_errors_retry_then_move_on_and_bad_sign_in_parks` | [Passed](TEST_REPORT.md#f-183) |
| F-184 | [x] <a id="f-184"></a>Retry temporary errors on the same account after 10 then 30 seconds; after retries cool it for 300 seconds and hand off. | `antiquota/engine.py:689` | `tests/test_engine.py::test_temporary_errors_retry_then_move_on_and_bad_sign_in_parks` | [Passed](TEST_REPORT.md#f-184) |
| F-185 | [ ] <a id="f-185"></a>Bound turn handoffs to twice the signed-in account count plus one attempt allowance. | `antiquota/engine.py:638` | Not covered | [Pending](TEST_REPORT.md#f-185) |
| F-186 | [x] <a id="f-186"></a>If resume fails specifically because conversation is lost, clear conversation id and start anew with deterministic handoff state plus current request. | `antiquota/engine.py:706`; `antiquota/engine.py:780` | Not covered | [Passed](TEST_REPORT.md#f-186) |
| F-187 | [x] <a id="f-187"></a>After a healthy turn, refresh stale quota while still holding the account lease and broadcast updated accounts. | `antiquota/engine.py:667` | Not covered | [Passed](TEST_REPORT.md#f-187) |
| F-188 | [x] <a id="f-188"></a>On an internal worker exception, persist an error notice and return the session to idle without leaving a stuck running state. | `antiquota/engine.py:623` | Not covered | [Passed](TEST_REPORT.md#f-188) |
| F-189 | [ ] <a id="f-189"></a>Timestamp every event; persist final events append-only under session lock while live SSE includes sid and meta snapshots. | `antiquota/engine.py:282` | `tests/test_engine.py::test_turn_streams_persists_and_survives_restart` | [Pending](TEST_REPORT.md#f-189) |
| F-190 | [x] <a id="f-190"></a>Legacy: subscribe/unsubscribe independent event queues with a 20000-entry capacity. Rewrite correction: lagging consumers must recover every required event from the append-only log or receive an explicit resynchronization signal; no silent data loss. The legacy queue-overflow gap is confirmed by source, not claimed as observed in the Gemini run. | `antiquota/engine.py:273`; `antiquota/engine.py:292` | Not covered | [Passed](TEST_REPORT.md#f-190) |
| F-191 | [x] <a id="f-191"></a>Save updated metadata atomically and broadcast the same serialized snapshot to subscribers. | `antiquota/engine.py:298` | `tests/test_engine.py::test_turn_streams_persists_and_survives_restart` | [Passed](TEST_REPORT.md#f-191) |
| F-192 | [x] <a id="f-192"></a>Delete a session by killing its terminal, stopping its turn, removing its session folder and publishing deleted. | `antiquota/engine.py:480` | `tests/test_engine.py::test_terminal_runs_a_shell_in_the_session_folder` | [Passed](TEST_REPORT.md#f-192) |
| F-193 | [x] <a id="f-193"></a>Search user/assistant messages case-insensitively for queries of at least 2 characters, one newest match per session,40 results sorted by timestamp with a contextual snippet. | `antiquota/engine.py:414` | `tests/test_engine.py::test_paused_accounts_sit_out_and_sessions_pin_fork_search` | [Passed](TEST_REPORT.md#f-193) |
| F-194 | [x] <a id="f-194"></a>Fork into the same folder/model/effort/mode with '(fork)' title and pending handoff state, starting a fresh conversation without inherited goal/jobs/worktree ownership. | `antiquota/engine.py:407` | `tests/test_engine.py::test_paused_accounts_sit_out_and_sessions_pin_fork_search` | [Passed](TEST_REPORT.md#f-194) |
| F-195 | [x] <a id="f-195"></a>Inherit another session's deterministic handoff state into an idle destination, preserving the destination's pending note and recording source id. | `antiquota/engine.py:1413` | `tests/test_engine.py::test_handoff_state_and_picking_up_another_session` | [Passed](TEST_REPORT.md#f-195) |
| F-196 | [x] <a id="f-196"></a>Build and persist STATE.md after every turn from objective/check/verdict, last 8 requests, last 12 turns, changed files, last 6 jobs, last 3 problems and last 1500 reply characters. | `antiquota/engine.py:1347` | `tests/test_engine.py::test_handoff_state_and_picking_up_another_session` | [Passed](TEST_REPORT.md#f-196) |
| F-197 | [ ] <a id="f-197"></a>Keep raw failure logs and calls.jsonl records for direct CLI calls; do not confuse that ledger with session usage. | `antiquota/accounts.py:1147` | `tests/test_accounts.py::test_run_is_sticky_and_fails_over` | [Pending](TEST_REPORT.md#f-197) |
### Goals and independent verifier

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-198 | [x] <a id="f-198"></a>Set an objective/check/verifier/model/max-rounds goal with trimmed text, round 0 and active state, publish goal notice and start a goal-kind prompt by default. | `antiquota/engine.py:534` | `tests/test_engine.py::test_goal_pushes_on_until_claimed_and_verified` | [Passed](TEST_REPORT.md#f-198) |
| F-199 | [x] <a id="f-199"></a>Reject an empty HTTP goal objective; default verifier=true, verifier_model empty and max_rounds 20 at the legacy API boundary. | `antiquota/server.py:465` | Not covered | [Passed](TEST_REPORT.md#f-199) |
| F-200 | [x] <a id="f-200"></a>Goal prompts require autonomous progress and a final GOAL_COMPLETE marker only after the objective is fully verified. | `antiquota/engine.py:61` | `tests/test_engine.py::test_goal_pushes_on_until_claimed_and_verified` | [Passed](TEST_REPORT.md#f-200) |
| F-201 | [x] <a id="f-201"></a>Each successful goal evaluation increments round; abandon as failed when the legacy counter exceeds max_rounds, documenting boundary behavior for parity. | `antiquota/engine.py:1237` | `tests/test_engine.py::test_goal_feeds_failed_checks_and_verdicts_back` | [Passed](TEST_REPORT.md#f-201) |
| F-202 | [x] <a id="f-202"></a>A reply without the completion mark triggers a nudge toward the objective. | `antiquota/engine.py:1247` | `tests/test_engine.py::test_goal_pushes_on_until_claimed_and_verified` | [Passed](TEST_REPORT.md#f-202) |
| F-203 | [x] <a id="f-203"></a>A goal waits for unfinished notifying background jobs without spending a goal round, then continues when a job reports completion. | `antiquota/engine.py:588`; `antiquota/engine.py:595` | `tests/test_engine.py::test_background_job_from_reply_wakes_the_goal` | [Passed](TEST_REPORT.md#f-203) |
| F-204 | [ ] <a id="f-204"></a>For failed turns in an active goal, retry with 30, 60 and 90 second waits, at most 3 retry nudges, respecting cancellation. | `antiquota/engine.py:604` | Not covered | [Pending](TEST_REPORT.md#f-204) |
| F-205 | [x] <a id="f-205"></a>Stopping a goal pauses it; resume resets state active and round 0 with a nudge; clear removes only the goal record. | `antiquota/engine.py:529`; `antiquota/engine.py:545`; `antiquota/engine.py:555` | `tests/test_engine.py::test_stop_ends_the_turn_and_pauses_the_goal` | [Passed](TEST_REPORT.md#f-205) |
| F-206 | [x] <a id="f-206"></a>Reject resume when the session has no goal. | `antiquota/engine.py:548` | Not covered | [Passed](TEST_REPORT.md#f-206) |
| F-207 | [x] <a id="f-207"></a>Run a goal check in project cwd with shell semantics, a 1800 second timeout, last 3000 stdout/stderr characters and passed=(exit 0). | `antiquota/engine.py:1253` | `tests/test_engine.py::test_goal_feeds_failed_checks_and_verdicts_back` | [Passed](TEST_REPORT.md#f-207) |
| F-208 | [x] <a id="f-208"></a>Do not run the verifier after a failed check; feed exact failed-check result to the next goal nudge. | `antiquota/engine.py:1262` | `tests/test_engine.py::test_goal_feeds_failed_checks_and_verdicts_back` | [Passed](TEST_REPORT.md#f-208) |
| F-209 | [x] <a id="f-209"></a>Allow verifier-disabled goals to become met after their completion mark and successful optional check. | `antiquota/engine.py:1264` | `tests/test_engine.py::test_background_job_from_reply_wakes_the_goal` | [Passed](TEST_REPORT.md#f-209) |
| F-210 | [x] <a id="f-210"></a>Run the verifier in a fresh agy conversation with viewing/listing only instructions, original permission mode and no session hooks. | `antiquota/engine.py:1303` | `tests/test_engine.py::test_goal_pushes_on_until_claimed_and_verified` | [Passed](TEST_REPORT.md#f-210) |
| F-211 | [x] <a id="f-211"></a>Try distinct verifier model then session model when the verifier's pool has no live quota; default model remains flash-high in settings/CLI. | `antiquota/engine.py:1287` | `tests/test_engine.py::test_verifier_falls_back_to_session_model_without_its_quota` | [Passed](TEST_REPORT.md#f-211) |
| F-212 | [ ] <a id="f-212"></a>Handle verifier quota error by walling/cooling that account and trying another, releasing leases in every case. | `antiquota/engine.py:1290`; `antiquota/engine.py:1322` | Not covered | [Pending](TEST_REPORT.md#f-212) |
| F-213 | [x] <a id="f-213"></a>Parse the final matching VERDICT line case-insensitively, supporting surrounding punctuation and DONE/NOT_DONE/NOT DONE. | `antiquota/engine.py:42`; `antiquota/engine.py:69` | `tests/test_engine.py::test_verdict_line_is_parsed_from_free_text` | [Passed](TEST_REPORT.md#f-213) |
| F-214 | [x] <a id="f-214"></a>Return up to 20 missing bullet items and 1500 reason characters; a no-verdict response yields no verdict rather than done. | `antiquota/engine.py:77` | `tests/test_engine.py::test_verdict_line_is_parsed_from_free_text` | [Passed](TEST_REPORT.md#f-214) |
| F-215 | [x] <a id="f-215"></a>Record check/verdict notices, verifier slot/model and done/missing fields; feed negative or unavailable verifier result back to the goal. | `antiquota/engine.py:1268`; `antiquota/engine.py:1332` | `tests/test_engine.py::test_goal_feeds_failed_checks_and_verdicts_back` | [Passed](TEST_REPORT.md#f-215) |
| F-216 | [x] <a id="f-216"></a>Mark goal met only when the completion mark, optional successful check and optional positive verifier agree. | `antiquota/engine.py:1276` | `tests/test_engine.py::test_goal_pushes_on_until_claimed_and_verified` | [Passed](TEST_REPORT.md#f-216) |
| F-217 | [x] <a id="f-217"></a>Cancel a verifier through the active session process and stop without setting met. | `antiquota/engine.py:1266`; `antiquota/engine.py:1319` | `tests/test_engine.py::test_stop_ends_the_turn_and_pauses_the_goal` | [Passed](TEST_REPORT.md#f-217) |
### Background jobs, intervals, schedules and keep-awake

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-218 | [x] <a id="f-218"></a>Start explicit nonempty shell jobs detached in session cwd with no stdin, merged log output, NO_COLOR and unbuffered Python environment. | `antiquota/engine.py:938`; `antiquota/server.py:422` | `tests/test_engine.py::test_user_job_stop_and_loop` | [Passed](TEST_REPORT.md#f-218) |
| F-219 | [x] <a id="f-219"></a>Record job id 8 characters, cmd/pid/log/started/ended/code/every/notify/by/stopped and retain at most 20 jobs in session metadata. | `antiquota/engine.py:951` | `tests/test_engine.py::test_background_job_from_reply_wakes_the_goal` | [Passed](TEST_REPORT.md#f-219) |
| F-220 | [x] <a id="f-220"></a>Parse agent RUN_IN_BACKGROUND requests and optional CHECK_EVERY units; avoid starting a duplicate running command. | `antiquota/engine.py:92`; `antiquota/engine.py:960` | `tests/test_engine.py::test_background_job_from_reply_wakes_the_goal` | [Passed](TEST_REPORT.md#f-220) |
| F-221 | [x] <a id="f-221"></a>Rewrite requirement: model-origin background requests must pass owned permission policy/approval, and quoted/example lines must not trigger unwanted execution. | `antiquota/engine.py:960` | `Pending; docs/rewrite/known-bugs.md` | [Passed](TEST_REPORT.md#f-221) |
| F-222 | [x] <a id="f-222"></a>Publish job started/ended events and save metadata as each phase changes. | `antiquota/engine.py:956`; `antiquota/engine.py:988` | `tests/test_engine.py::test_background_job_from_reply_wakes_the_goal` | [Passed](TEST_REPORT.md#f-222) |
| F-223 | [x] <a id="f-223"></a>Watch child processes with 5 second waits and adopt unfinished jobs after engine restart by PID checks, preserving logs and unknown adopted exit code. | `antiquota/engine.py:262`; `antiquota/engine.py:967` | Not covered | [Passed](TEST_REPORT.md#f-223) |
| F-224 | [x] <a id="f-224"></a>Send optional progress messages only while session idle and not stopped, including elapsed duration, PID and last 4000 log characters. | `antiquota/engine.py:981` | Not covered | [Passed](TEST_REPORT.md#f-224) |
| F-225 | [x] <a id="f-225"></a>On natural job completion, send a job-kind result/log prompt and continue queued work or goal; notify=false or explicit stop suppresses it. | `antiquota/engine.py:992` | `tests/test_engine.py::test_background_job_from_reply_wakes_the_goal` | [Passed](TEST_REPORT.md#f-225) |
| F-226 | [x] <a id="f-226"></a>Stop a running job's whole process tree/group (Windows taskkill or POSIX SIGTERM); repeated stop of an ended job is idempotent. | `antiquota/engine.py:998` | `tests/test_engine.py::test_user_job_stop_and_loop` | [Passed](TEST_REPORT.md#f-226) |
| F-227 | [x] <a id="f-227"></a>Return job metadata plus up to 200000 characters of ANSI-stripped recent log for the log view. | `antiquota/engine.py:1014` | Not covered | [Passed](TEST_REPORT.md#f-227) |
| F-228 | [x] <a id="f-228"></a>Background jobs survive ordinary engine shutdown and are included in busy_count independently of session turns. | `antiquota/engine.py:1032`; `antiquota/engine.py:1037` | Not covered | [Passed](TEST_REPORT.md#f-228) |
| F-229 | [x] <a id="f-229"></a>Parse positive decimal intervals with s/m/h or default minutes, case-insensitively; invalid syntax returns 0 and HTTP loop rejects it. | `antiquota/engine.py:135`; `antiquota/server.py:430` | Not covered | [Passed](TEST_REPORT.md#f-229) |
| F-230 | [x] <a id="f-230"></a>Create an interval loop with at least 60 seconds between prompts and initial next=now; require a nonempty prompt. | `antiquota/engine.py:1018` | `tests/test_engine.py::test_user_job_stop_and_loop` | [Passed](TEST_REPORT.md#f-230) |
| F-231 | [ ] <a id="f-231"></a>Only dispatch due loop prompts while the session is idle and not stopped; advance next before sending and tag the event kind loop. | `antiquota/engine.py:1056` | `tests/test_engine.py::test_user_job_stop_and_loop` | [Pending](TEST_REPORT.md#f-231) |
| F-232 | [x] <a id="f-232"></a>Daily schedules accept local H:MM/HH:MM from 00:00 through 23:59; select later today or tomorrow when the time is equal/past. | `antiquota/engine.py:143` | `tests/test_engine.py::test_daily_schedule_fires_at_its_time` | [Passed](TEST_REPORT.md#f-232) |
| F-233 | [ ] <a id="f-233"></a>A daily schedule persists at/every 86400/prompt/next, waits until its due local time and recalculates the next local day after dispatch. | `antiquota/engine.py:1024`; `antiquota/engine.py:1059` | `tests/test_engine.py::test_daily_schedule_fires_at_its_time` | [Pending](TEST_REPORT.md#f-233) |
| F-234 | [ ] <a id="f-234"></a>Stop pauses dispatch without deleting the loop; installing/replacing a loop clears session stop; deleting clears only the loop. | `antiquota/engine.py:1027`; `antiquota/engine.py:1029`; `antiquota/server.py:439` | `tests/test_engine.py::test_user_job_stop_and_loop` | [Pending](TEST_REPORT.md#f-234) |
| F-235 | [ ] <a id="f-235"></a>Poll scheduler work every 5 seconds and refresh stale idle quota asynchronously, broadcasting account changes. | `antiquota/engine.py:1045` | Not covered | [Pending](TEST_REPORT.md#f-235) |
| F-236 | [x] <a id="f-236"></a>When keep_awake=on and any turn/job runs, inhibit system sleep without inhibiting display sleep; release while idle or disabled. | `antiquota/engine.py:98`; `antiquota/engine.py:1049` | Not covered | [Passed](TEST_REPORT.md#f-236) |
| F-237 | [ ] <a id="f-237"></a>Use caffeinate on macOS, SetThreadExecutionState on Windows and systemd-inhibit when available on Linux; tolerate missing support. | `antiquota/engine.py:107` | Not covered | [Pending](TEST_REPORT.md#f-237) |
| F-238 | [ ] <a id="f-238"></a>Release sleep inhibition on shutdown while pausing active session turns and leaving detached jobs alive. | `antiquota/engine.py:1037` | Not covered | [Pending](TEST_REPORT.md#f-238) |
### Files, attachments, checkpoints and project notes

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-239 | [ ] <a id="f-239"></a>Expand @path mentions only for real files/folders, deduplicate normalized absolute references and avoid treating an email address as a mention. | `antiquota/engine.py:182`; `antiquota/engine.py:185` | `tests/test_engine.py::test_mentions_clear_and_compact` | [Pending](TEST_REPORT.md#f-239) |
| F-240 | [ ] <a id="f-240"></a>Search project files recursively for @-mention completion, skip hidden/heavy folders, cap discovery after 400 matches, prioritize matching basenames then shorter paths and return at most 30. | `antiquota/server.py:193` | Not covered | [Pending](TEST_REPORT.md#f-240) |
| F-241 | [x] <a id="f-241"></a>Upload attachments under the owning session folder; sanitize basename, replace unsupported characters and avoid collisions with -2,-3 suffixes. | `antiquota/engine.py:491` | `tests/test_engine.py::test_attachments_reach_agy_through_add_dir` | [Passed](TEST_REPORT.md#f-241) |
| F-242 | [x] <a id="f-242"></a>Classify PNG/JPG/JPEG/GIF/WEBP/BMP/HEIC attachments as images; return name/path/size/image metadata. | `antiquota/engine.py:503` | `tests/test_engine.py::test_attachments_reach_agy_through_add_dir` | [Passed](TEST_REPORT.md#f-242) |
| F-243 | [x] <a id="f-243"></a>Only pass actual files whose real paths remain in that session's attachment directory to agy; drop external or symlink-escaping attachment paths. | `antiquota/engine.py:508` | `tests/test_engine.py::test_attachments_reach_agy_through_add_dir` | [Passed](TEST_REPORT.md#f-243) |
| F-244 | [x] <a id="f-244"></a>Permit attachment-only messages, preserve queued attachment metadata and include attachment paths plus folder --add-dir in the eventual turn. | `antiquota/engine.py:512`; `antiquota/engine.py:569`; `antiquota/server.py:413` | `tests/test_engine.py::test_attachments_reach_agy_through_add_dir` | [Passed](TEST_REPORT.md#f-244) |
| F-245 | [x] <a id="f-245"></a>Serve session attachments with inferred MIME or application/octet-stream and reject missing/traversal/escaping files. | `antiquota/server.py:308` | Not covered | [Passed](TEST_REPORT.md#f-245) |
| F-246 | [x] <a id="f-246"></a>Rewrite correction: decode the URL filename path segment so uploaded filenames with spaces/Unicode download correctly; do not carry forward B-021. | `antiquota/server.py:308` | `Pending; docs/rewrite/known-bugs.md` | [Passed](TEST_REPORT.md#f-246) |
| F-247 | [x] <a id="f-247"></a>Store each turn's first-before and latest-after readable file contents as atomic checkpoint JSON for rewind. | `antiquota/engine.py:860` | `tests/test_engine.py::test_rewind_puts_files_back_and_tells_the_agent` | [Passed](TEST_REPORT.md#f-247) |
| F-248 | [x] <a id="f-248"></a>Rewind an idle session's selected turn and every later checkpoint, restoring original content or deleting files created after that point. | `antiquota/engine.py:875` | `tests/test_engine.py::test_rewind_puts_files_back_and_tells_the_agent` | [Passed](TEST_REPORT.md#f-248) |
| F-249 | [x] <a id="f-249"></a>Reject rewind while busy or when no changes exist; remove used checkpoints, clear remembered file state, record changed/outside files and inject a reread note. | `antiquota/engine.py:878` | `tests/test_engine.py::test_rewind_puts_files_back_and_tells_the_agent` | [Passed](TEST_REPORT.md#f-249) |
| F-250 | [x] <a id="f-250"></a>Rewrite correction: rewind must preserve an originally empty file and guard user edits made after the last agent checkpoint, per known-bug ledger. | `antiquota/engine.py:892` | `Pending; docs/rewrite/known-bugs.md` | [Passed](TEST_REPORT.md#f-250) |
| F-251 | [x] <a id="f-251"></a>Clear an idle conversation id/context/pending note while retaining transcript and goal/session metadata; reject while busy. | `antiquota/engine.py:915` | `tests/test_engine.py::test_mentions_clear_and_compact` | [Passed](TEST_REPORT.md#f-251) |
| F-252 | [x] <a id="f-252"></a>Compact an idle session into a fresh conversation with deterministic handoff summary as the next pending note; retain transcript and reject while busy. | `antiquota/engine.py:924` | `tests/test_engine.py::test_mentions_clear_and_compact` | [Passed](TEST_REPORT.md#f-252) |
| F-253 | [x] <a id="f-253"></a>List one project folder with real-path containment, directories first, hidden entries after ordinary entries, size/dim flags and at most 400 entries. | `antiquota/engine.py:1191` | `tests/test_engine.py::test_files_tree_stays_inside_the_session_folder` | [Passed](TEST_REPORT.md#f-253) |
| F-254 | [x] <a id="f-254"></a>Reject project-tree traversal and symlink escapes; return root and relative path for valid subfolders. | `antiquota/engine.py:1194` | `tests/test_engine.py::test_files_tree_stays_inside_the_session_folder` | [Passed](TEST_REPORT.md#f-254) |
| F-255 | [x] <a id="f-255"></a>Read text only within project or session attachments, cap at 2 MB, reject a NUL in first 8000 bytes and decode UTF-8 with replacement. | `antiquota/engine.py:1211` | `tests/test_engine.py::test_shell_note_and_file_viewer` | [Passed](TEST_REPORT.md#f-255) |
| F-256 | [x] <a id="f-256"></a>Append a trimmed project note to existing GEMINI.md, otherwise existing AGENTS.md, otherwise new GEMINI.md; preserve needed separating newline. | `antiquota/engine.py:1179` | `tests/test_engine.py::test_shell_note_and_file_viewer` | [Passed](TEST_REPORT.md#f-256) |
| F-257 | [ ] <a id="f-257"></a>Run user !shell commands in project cwd with merged output, NO_COLOR, no stdin and 600 second timeout; return exit code or null timeout. | `antiquota/engine.py:1158` | `tests/test_engine.py::test_shell_note_and_file_viewer` | [Pending](TEST_REPORT.md#f-257) |
| F-258 | [x] <a id="f-258"></a>Keep last 50000 ANSI-stripped shell output characters in transcript and last 6000 in the next agent's pending note, preserving older pending notes. | `antiquota/engine.py:1168` | `tests/test_engine.py::test_shell_note_and_file_viewer` | [Passed](TEST_REPORT.md#f-258) |
| F-259 | [x] <a id="f-259"></a>Restrict the folder picker to real directories under real HOME, fall back to HOME for outside/invalid paths, omit hidden dirs and return sorted names/parent. | `antiquota/server.py:210` | `tests/test_server.py::test_permissions_are_agy_loops_own_and_folder_listing` | [Passed](TEST_REPORT.md#f-259) |
### Git worktrees and change review

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-260 | [x] <a id="f-260"></a>Detect a repository root through git CLI, disable terminal credential prompts and optional locks, and report command errors without pushing. | `antiquota/gitops.py:7` | `tests/test_gitops.py::test_worktree_session_review_commit_merge` | [Passed](TEST_REPORT.md#f-260) |
| F-261 | [x] <a id="f-261"></a>Create a session worktree only for a repository with at least one commit, starting at current HEAD rather than copying dirty edits. | `antiquota/gitops.py:31` | `tests/test_gitops.py::test_worktree_needs_a_repository` | [Passed](TEST_REPORT.md#f-261) |
| F-262 | [x] <a id="f-262"></a>Name new branches antiquota/<title-slug>-<sid-prefix> and folders <repo>-<sid>; slug is lowercase allowed words capped 32 characters with session fallback. | `antiquota/gitops.py:27`; `antiquota/gitops.py:41` | `tests/test_gitops.py::test_worktree_session_review_commit_merge` | [Passed](TEST_REPORT.md#f-262) |
| F-263 | [x] <a id="f-263"></a>Preserve the original working subdirectory inside the new worktree and store repo/path/branch/base/base_sha. | `antiquota/gitops.py:45` | `tests/test_gitops.py::test_worktree_session_review_commit_merge` | [Passed](TEST_REPORT.md#f-263) |
| F-264 | [x] <a id="f-264"></a>Rename an untitled worktree branch after the first user request only when it still has the original session name and the rename actually succeeds. | `antiquota/engine.py:353` | `tests/test_gitops.py::test_untitled_worktree_branch_takes_the_first_request` | [Passed](TEST_REPORT.md#f-264) |
| F-265 | [ ] <a id="f-265"></a>Show branch/root/worktree metadata plus porcelain changed/untracked files with added/deleted counts; handle renamed paths and binary numstat. | `antiquota/gitops.py:51`; `antiquota/gitops.py:60` | `tests/test_gitops.py::test_worktree_session_review_commit_merge` | [Pending](TEST_REPORT.md#f-265) |
| F-266 | [x] <a id="f-266"></a>For worktrees, expose commits ahead of base_sha, committed changes and whether the branch is already an ancestor of the original base. | `antiquota/gitops.py:81` | `tests/test_gitops.py::test_worktree_session_review_commit_merge` | [Passed](TEST_REPORT.md#f-266) |
| F-267 | [x] <a id="f-267"></a>Show unified unstaged/staged-vs-HEAD diffs plus added untracked files, optionally narrowed to a path. | `antiquota/gitops.py:90` | `tests/test_gitops.py::test_worktree_session_review_commit_merge` | [Passed](TEST_REPORT.md#f-267) |
| F-268 | [ ] <a id="f-268"></a>Show committed worktree diff against base_sha; reject committed mode for non-worktree sessions and diff outside a repository. | `antiquota/gitops.py:96` | `tests/test_gitops.py::test_worktree_session_review_commit_merge` | [Pending](TEST_REPORT.md#f-268) |
| F-269 | [x] <a id="f-269"></a>Commit stages all changes under the repository (or explicit internal paths), requires nonblank message and rejects nothing-to-commit; return short SHA and persist notice. | `antiquota/gitops.py:107`; `antiquota/engine.py:372` | `tests/test_gitops.py::test_worktree_session_review_commit_merge` | [Passed](TEST_REPORT.md#f-269) |
| F-270 | [x] <a id="f-270"></a>Discard one repository-relative file by restoring tracked HEAD content or removing a new file; reject absolute/traversal/empty paths. | `antiquota/gitops.py:120` | `tests/test_gitops.py::test_worktree_session_review_commit_merge` | [Passed](TEST_REPORT.md#f-270) |
| F-271 | [ ] <a id="f-271"></a>Merge worktree branch with --no-ff into the stored original base only when main checkout is on that branch and both main tracked state and worktree state are clean. | `antiquota/gitops.py:132` | `tests/test_gitops.py::test_merge_refuses_a_dirty_main_checkout` | [Pending](TEST_REPORT.md#f-271) |
| F-272 | [ ] <a id="f-272"></a>Reject merge without a worktree, preserve dirty main files on refusal and report conflicts/errors from git. | `antiquota/engine.py:384`; `antiquota/gitops.py:137` | `tests/test_gitops.py::test_merge_refuses_a_dirty_main_checkout` | [Pending](TEST_REPORT.md#f-272) |
| F-273 | [ ] <a id="f-273"></a>Remove a worktree and prune registrations, optionally delete its branch; reject removal while the session turn is busy. | `antiquota/gitops.py:146`; `antiquota/engine.py:393` | `tests/test_gitops.py::test_worktree_session_review_commit_merge` | [Pending](TEST_REPORT.md#f-273) |
| F-274 | [x] <a id="f-274"></a>After worktree removal, reset session cwd to main repo, clear conversation/worktree, archive the session and queue a main-checkout handoff note. | `antiquota/engine.py:400` | `tests/test_gitops.py::test_worktree_session_review_commit_merge` | [Passed](TEST_REPORT.md#f-274) |
### Terminal and dictation

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-275 | [x] <a id="f-275"></a>Start/reuse one terminal per session in that session's cwd, default 100 columns 30 rows, with TERM=xterm-256 color and COLORTERM=truecolor. | `antiquota/engine.py:469`; `antiquota/terminal.py:15` | `tests/test_engine.py::test_terminal_runs_a_shell_in_the_session_folder` | [Passed](TEST_REPORT.md#f-275) |
| F-276 | [ ] <a id="f-276"></a>On POSIX run the user's SHELL or /bin/zsh as a login shell in its own process group; on Windows use COMSPEC or powershell.exe via ConPTY. | `antiquota/terminal.py:19` | `tests/test_engine.py::test_terminal_runs_a_shell_in_the_session_folder` | [Pending](TEST_REPORT.md#f-276) |
| F-277 | [x] <a id="f-277"></a>Stream raw terminal bytes and retain the latest 256 KiB for new/reconnected subscribers. | `antiquota/terminal.py:11`; `antiquota/terminal.py:73` | `tests/test_engine.py::test_terminal_runs_a_shell_in_the_session_folder` | [Passed](TEST_REPORT.md#f-277) |
| F-278 | [x] <a id="f-278"></a>Give terminal subscribers recent output plus bounded 4000 entry queues; unsubscribe cleanly and handle lag without losing exit state in the rewrite. | `antiquota/terminal.py:84` | Not covered | [Passed](TEST_REPORT.md#f-278) |
| F-279 | [x] <a id="f-279"></a>Write UTF-8 terminal input only while alive; a dead shell gives an actionable restart error. | `antiquota/terminal.py:96` | `tests/test_engine.py::test_terminal_runs_a_shell_in_the_session_folder` | [Passed](TEST_REPORT.md#f-279) |
| F-280 | [x] <a id="f-280"></a>Resize live terminals through platform PTY support, clamping POSIX rows/columns to at least 2; resizing dead terminals is a no-op. | `antiquota/terminal.py:36`; `antiquota/terminal.py:104` | `tests/test_engine.py::test_terminal_runs_a_shell_in_the_session_folder` | [Passed](TEST_REPORT.md#f-280) |
| F-281 | [x] <a id="f-281"></a>Mark shell exit, emit the process-exited line, then notify subscriber termination; restart replaces dead or explicitly restarted shells. | `antiquota/terminal.py:63`; `antiquota/engine.py:472` | Not covered | [Passed](TEST_REPORT.md#f-281) |
| F-282 | [x] <a id="f-282"></a>Kill terminal process/group on explicit kill, session deletion or restart, tolerating already-ended processes. | `antiquota/terminal.py:112`; `antiquota/engine.py:480` | `tests/test_engine.py::test_terminal_runs_a_shell_in_the_session_folder` | [Passed](TEST_REPORT.md#f-282) |
| F-283 | [x] <a id="f-283"></a>Terminal SSE begins with base 64 recent data plus shell/cwd/alive, then base 64 chunks,15 second pings and exit=true. | `antiquota/server.py:548` | Not covered | [Passed](TEST_REPORT.md#f-283) |
| F-284 | [ ] <a id="f-284"></a>Coalesce up to 64 queued terminal chunks per SSE frame and unsubscribe after stream disconnect. | `antiquota/server.py:572` | Not covered | [Pending](TEST_REPORT.md#f-284) |
| F-285 | [x] <a id="f-285"></a>Dictation rejects empty recordings and payloads above 25 MB; keep a temporary owned recording with sanitized extension capped 5 characters. | `antiquota/engine.py:430` | `tests/test_server.py::test_transcribe_route_needs_audio` | [Passed](TEST_REPORT.md#f-285) |
| F-286 | [x] <a id="f-286"></a>Acquire a gemini account for dictation, report no available quota, run flash-high in a fresh JSON conversation with folder access and 180 second timeout. | `antiquota/engine.py:441` | Not covered | [Passed](TEST_REPORT.md#f-286) |
| F-287 | [x] <a id="f-287"></a>Use the explicit clear-speech/NO_SPEECH guard, preserve the spoken language and return only text plus account identity. | `antiquota/engine.py:447` | Not covered | [Passed](TEST_REPORT.md#f-287) |
| F-288 | [x] <a id="f-288"></a>Treat empty or NO_SPEECH-containing transcriptions as empty; malformed agy JSON reports a transcription error. | `antiquota/engine.py:457` | Not covered | [Passed](TEST_REPORT.md#f-288) |
| F-289 | [x] <a id="f-289"></a>Delete the temporary recording and release the account lease after every success or failure; dictation creates no session conversation. | `antiquota/engine.py:455`; `antiquota/engine.py:463` | Not covered | [Passed](TEST_REPORT.md#f-289) |
### CLI commands and options

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-290 | [x] <a id="f-290"></a>No command defaults to accounts; unknown commands or invalid positional forms print usage to stderr and exit 2. | `antiquota/__main__.py:173` | Not covered | [Passed](TEST_REPORT.md#f-290) |
| F-291 | [x] <a id="f-291"></a>accounts prints signed-in account quota/walls and lease state, or an add-account hint when empty. | `antiquota/__main__.py:28` | `tests/test_accounts.py::test_run_is_sticky_and_fails_over` | [Passed](TEST_REPORT.md#f-291) |
| F-292 | [ ] <a id="f-292"></a>login opens agy's own URL, reads authorization codes from terminal without changing real CLI config and reports adopted account or failure. | `antiquota/accounts.py:616`; `antiquota/__main__.py:192` | `tests/test_accounts.py::test_sign_in_needs_the_right_code_and_never_half_creates_an_account` | [Pending](TEST_REPORT.md#f-292) |
| F-293 | [x] <a id="f-293"></a>probe refreshes every idle account without model tokens, skips busy accounts and prints unreadable/ok results. | `antiquota/accounts.py:1077`; `antiquota/__main__.py:198` | `tests/test_accounts.py::test_probe_walls_spent_basket_until_panel_reset` | [Passed](TEST_REPORT.md#f-293) |
| F-294 | [ ] <a id="f-294"></a>doctor reports own stack skills/hooks/MCP, file-token isolation, conversation store link, shared-entry parity and actual account identity; exit 1 if problems. | `antiquota/__main__.py:57` | `tests/test_accounts.py::test_accounts_are_named_by_email_and_use_their_own_stack` | [Pending](TEST_REPORT.md#f-294) |
| F-295 | [x] <a id="f-295"></a>stack prints the own-stack folder; stack reseed recopies source configuration while retaining conversations. | `antiquota/__main__.py:93` | `tests/test_accounts.py::test_stack_is_separate_from_your_agy_cli` | [Passed](TEST_REPORT.md#f-295) |
| F-296 | [x] <a id="f-296"></a>unwall <name\|all> clears both quota walls and prints the target. | `antiquota/__main__.py:103` | Not covered | [Passed](TEST_REPORT.md#f-296) |
| F-297 | [x] <a id="f-297"></a>run accepts leading --slot S and optional -- before forwarding all remaining agy arguments, rejecting absent accounts or unknown slot. | `antiquota/__main__.py:176`; `antiquota/accounts.py:1119` | `tests/test_accounts.py::test_run_is_sticky_and_fails_over` | [Passed](TEST_REPORT.md#f-297) |
| F-298 | [x] <a id="f-298"></a>run uses the selected account's isolated environment and returns its exit code; quota errors/all-walled return 75, leaving a failure/call audit record. | `antiquota/accounts.py:1124` | `tests/test_accounts.py::test_run_is_sticky_and_fails_over` | [Passed](TEST_REPORT.md#f-298) |
| F-299 | [ ] <a id="f-299"></a>Interactive run inherits the user's terminal and temporarily ignores SIGINT in the parent; noninteractive run tees stdout/stderr while keeping last 64 KiB each. | `antiquota/accounts.py:1092`; `antiquota/accounts.py:1135` | Not covered | [Pending](TEST_REPORT.md#f-299) |
| F-300 | [ ] <a id="f-300"></a>gui supports --port N default 8765 or 0 ephemeral and --no-open; it opens the browser unless disabled. | `antiquota/__main__.py:111` | `tests/test_server.py::test_engine_daemon_is_single_and_discoverable` | [Pending](TEST_REPORT.md#f-300) |
| F-301 | [x] <a id="f-301"></a>serve accepts the same port/no-open options but starts engine without automatically opening a browser. | `antiquota/__main__.py:185` | `tests/test_server.py::test_engine_daemon_is_single_and_discoverable` | [Passed](TEST_REPORT.md#f-301) |
| F-302 | [x] <a id="f-302"></a>hook accepts pre-tool/pre-invocation and runs the low-dependency hook entrypoint suitable for per-tool startup. | `antiquota/__main__.py:187`; `antiquota/hook.py:70` | `tests/test_server.py::test_approvals_through_the_hook_and_messages_mid_turn` | [Passed](TEST_REPORT.md#f-302) |
| F-303 | [x] <a id="f-303"></a>goal <objective> supports --cwd default., --model flash-high, --mode accept-edits with all four allowed modes, --check empty and --no-verifier. | `antiquota/__main__.py:141` | `tests/test_engine.py::test_goal_pushes_on_until_claimed_and_verified` | [Passed](TEST_REPORT.md#f-303) |
| F-304 | [x] <a id="f-304"></a>goal supports --verifier-model flash-high and --max-rounds 20, creates a GUI-visible session and returns 0 only when met. | `antiquota/__main__.py:150` | `tests/test_engine.py::test_goal_pushes_on_until_claimed_and_verified` | [Passed](TEST_REPORT.md#f-304) |
| F-305 | [x] <a id="f-305"></a>Goal CLI prints streamed deltas, turn/account/model, tool outcome, notices and elapsed time; Ctrl-C stops and preserves resumability with exit 1 unless met. | `antiquota/__main__.py:120`; `antiquota/__main__.py:166` | `tests/test_engine.py::test_stop_ends_the_turn_and_pauses_the_goal` | [Passed](TEST_REPORT.md#f-305) |
### HTTP safety, daemon discovery and streaming

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-306 | [ ] <a id="f-306"></a>Bind only 127.0.0.1 and require exact localhost:<port> or 127.0.0.1:<port> Host, including health/static requests; reject other hosts with 403 bad host. | `antiquota/server.py:271`; `antiquota/server.py:606` | `tests/test_server.py::test_page_carries_token_and_api_demands_it` | [Pending](TEST_REPORT.md#f-306) |
| F-307 | [ ] <a id="f-307"></a>Reuse a per-install GUI token of at least 32 characters across restarts; create new token mode `0600` only when missing/short. | `antiquota/server.py:47` | `tests/test_server.py::test_page_carries_token_and_api_demands_it` | [Pending](TEST_REPORT.md#f-307) |
| F-308 | [ ] <a id="f-308"></a>Require X-Antiquota-Token for ordinary API routes using constant-time comparison; reject missing/wrong token or non-API paths with 403 forbidden. | `antiquota/server.py:283` | `tests/test_server.py::test_page_carries_token_and_api_demands_it` | [Pending](TEST_REPORT.md#f-308) |
| F-309 | [x] <a id="f-309"></a>Use query token for EventSource, terminal stream and attachment GET; ordinary API query tokens do not substitute for the header. | `antiquota/server.py:284` | `tests/test_server.py::test_page_carries_token_and_api_demands_it` | [Passed](TEST_REPORT.md#f-309) |
| F-310 | [ ] <a id="f-310"></a>Serve /,/app.js,/app.css and flat allowed JS/CSS/HTML vendor files; inject token into index, reject vendor traversal/hidden names. | `antiquota/server.py:24`; `antiquota/server.py:276` | `tests/test_server.py::test_page_carries_token_and_api_demands_it` | [Pending](TEST_REPORT.md#f-310) |
| F-311 | [ ] <a id="f-311"></a>JSON responses include application/json, byte Content-Length, no-store and nosniff; success status is 200 for all legacy API mutations. | `antiquota/server.py:250` | `tests/test_server.py::test_session_round_trip_over_http` | [Pending](TEST_REPORT.md#f-311) |
| F-312 | [ ] <a id="f-312"></a>Read JSON mutation bodies up to 5 MB, using {}for absent/empty body; malformed JSON or oversized body returns 400. | `antiquota/server.py:262` | Not covered | [Pending](TEST_REPORT.md#f-312) |
| F-313 | [x] <a id="f-313"></a>Read attachment/audio binary bodies up to 25 MB with X-Filename or X-Audio-Type headers; report distinct oversized errors. | `antiquota/server.py:296` | `tests/test_server.py::test_transcribe_route_needs_audio` | [Passed](TEST_REPORT.md#f-313) |
| F-314 | [ ] <a id="f-314"></a>Map unknown route/session/item KeyError to 404 not found, validation ValueError to 400 and FileNotFoundError to 404 no such file. | `antiquota/server.py:318` | `tests/test_server.py::test_session_round_trip_over_http` | [Pending](TEST_REPORT.md#f-314) |
| F-315 | [ ] <a id="f-315"></a>Global SSE emits data: JSON plus blank line, sid on session events,15 second comment pings and no-store; unsubscribe on disconnect. | `antiquota/server.py:585` | Not covered | [Pending](TEST_REPORT.md#f-315) |
| F-316 | [ ] <a id="f-316"></a>Use an exclusive engine.lock per state folder; a second serve attaches/prints existing engine URL and exits 0 instead of creating a second daemon. | `antiquota/server.py:635`; `antiquota/server.py:660` | `tests/test_server.py::test_engine_daemon_is_single_and_discoverable` | [Pending](TEST_REPORT.md#f-316) |
| F-317 | [ ] <a id="f-317"></a>Write engine.json with pid/port/url/version/started/by; stale discovery is ignored when no lock holder exists. | `antiquota/server.py:621`; `antiquota/server.py:681` | `tests/test_server.py::test_engine_daemon_is_single_and_discoverable` | [Pending](TEST_REPORT.md#f-317) |
| F-318 | [ ] <a id="f-318"></a>If the requested engine port is occupied, pick a free port and publish its actual URL/port. | `antiquota/server.py:674` | Not covered | [Pending](TEST_REPORT.md#f-318) |
| F-319 | [ ] <a id="f-319"></a>Honor ANTIQUOTA_FIX_PATH on non-Windows using the login shell's PATH with 5 second timeout and marker parsing. | `antiquota/server.py:648`; `antiquota/server.py:670` | Not covered | [Pending](TEST_REPORT.md#f-319) |
| F-320 | [ ] <a id="f-320"></a>On shutdown/SIGTERM/Ctrl-C, pause active turns, release keep-awake, remove own discovery file and unlock engine; keep background jobs running. | `antiquota/server.py:684`; `antiquota/server.py:693` | `tests/test_server.py::test_engine_daemon_is_single_and_discoverable` | [Pending](TEST_REPORT.md#f-320) |
| F-321 | [ ] <a id="f-321"></a>Cache model id/label list from agy models in models.json, serve it immediately on restart, refresh through a signed-in slot and retain old cache on failure. | `antiquota/server.py:71`; `antiquota/server.py:81` | Not covered | [Pending](TEST_REPORT.md#f-321) |
| F-322 | [ ] <a id="f-322"></a>Run background account probing and model refresh every 600 seconds while exposing async manual probe/check. | `antiquota/server.py:38`; `antiquota/server.py:97` | Not covered | [Pending](TEST_REPORT.md#f-322) |
### HTTP API route checklist

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-323 | [x] <a id="f-323"></a>`GET /api/health` returns ok/app/version/pid without a token, after Host validation. | `antiquota/server.py:273` | `tests/test_server.py::test_engine_daemon_is_single_and_discoverable` | [Passed](TEST_REPORT.md#f-323) |
| F-324 | [ ] <a id="f-324"></a>`GET /api/events?token=…` opens the shared SSE stream with global/session events and keepalive comments. | `antiquota/server.py:292` | `tests/test_server.py::test_page_carries_token_and_api_demands_it` | [Pending](TEST_REPORT.md#f-324) |
| F-325 | [x] <a id="f-325"></a>`GET /api/sessions/{sid}/term/stream?token=…` returns terminal SSE; missing terminal returns 404. | `antiquota/server.py:294` | Not covered | [Passed](TEST_REPORT.md#f-325) |
| F-326 | [x] <a id="f-326"></a>`POST /api/transcribe` consumes raw audio plus X-Audio-Type(default wav), returns text/slot, rejects empty/overlarge recording. | `antiquota/server.py:296` | `tests/test_server.py::test_transcribe_route_needs_audio` | [Passed](TEST_REPORT.md#f-326) |
| F-327 | [x] <a id="f-327"></a>`POST /api/sessions/{sid}/attach` consumes bytes plus URL-encoded X-Filename(default file), returns name/path/size/image. | `antiquota/server.py:301` | `tests/test_engine.py::test_attachments_reach_agy_through_add_dir` | [Passed](TEST_REPORT.md#f-327) |
| F-328 | [x] <a id="f-328"></a>`GET /api/sessions/{sid}/attachments/{name}?token=…` returns raw attachment bytes with guessed MIME; filename path decoding correction is explicit. | `antiquota/server.py:308` | `Pending; B-021` | [Passed](TEST_REPORT.md#f-328) |
| F-329 | [ ] <a id="f-329"></a>`GET /api/state` returns updated-descending sessions, accounts, models, modes, defaults with cwd, and engine version/agy/busy. | `antiquota/server.py:328` | `tests/test_server.py::test_page_carries_token_and_api_demands_it` | [Pending](TEST_REPORT.md#f-329) |
| F-330 | [x] <a id="f-330"></a>`POST /api/approvals` accepts session/tool/args, returning decision/reason or pending id; conversation/step payload fields are legacy-ignored. | `antiquota/server.py:330` | `tests/test_server.py::test_approvals_through_the_hook_and_messages_mid_turn` | [Passed](TEST_REPORT.md#f-330) |
| F-331 | [x] <a id="f-331"></a>`GET /api/approvals/{aid}?wait=25` returns resolved decision or pending id after bounded wait, and ask for unknown id. | `antiquota/server.py:332` | `tests/test_server.py::test_approvals_through_the_hook_and_messages_mid_turn` | [Passed](TEST_REPORT.md#f-331) |
| F-332 | [x] <a id="f-332"></a>`POST /api/approvals/{aid}/answer` accepts decision allow/deny, always boolean and why truncated 500 characters; returns final decision/reason. | `antiquota/server.py:334` | `tests/test_server.py::test_approvals_through_the_hook_and_messages_mid_turn` | [Passed](TEST_REPORT.md#f-332) |
| F-333 | [x] <a id="f-333"></a>`POST /api/sessions/{sid}/fork` returns new session metadata after handoff inheritance. | `antiquota/server.py:338` | `tests/test_engine.py::test_paused_accounts_sit_out_and_sessions_pin_fork_search` | [Passed](TEST_REPORT.md#f-333) |
| F-334 | [x] <a id="f-334"></a>`GET /api/search?q=…` returns newest message-search hits; absent/short query returns []. | `antiquota/server.py:340` | `tests/test_engine.py::test_paused_accounts_sit_out_and_sessions_pin_fork_search` | [Passed](TEST_REPORT.md#f-334) |
| F-335 | [x] <a id="f-335"></a>`POST /api/accounts/{name}/pause` accepts on default true, rejects unknown account, broadcasts and returns updated account list. | `antiquota/server.py:342` | `tests/test_engine.py::test_paused_accounts_sit_out_and_sessions_pin_fork_search` | [Passed](TEST_REPORT.md#f-335) |
| F-336 | [x] <a id="f-336"></a>`POST /api/sessions/{sid}/steer` drains eligible queued strings and returns messages for the hook. | `antiquota/server.py:349` | `tests/test_server.py::test_approvals_through_the_hook_and_messages_mid_turn` | [Passed](TEST_REPORT.md#f-336) |
| F-337 | [x] <a id="f-337"></a>`GET /api/engine` returns version/pid/url/by/started/base/stack/log/hook/agy/session-count/account-count. | `antiquota/server.py:351` | Not covered | [Passed](TEST_REPORT.md#f-337) |
| F-338 | [x] <a id="f-338"></a>`GET /api/engine/log` returns last 400 log lines or the no-engine-log explanation. | `antiquota/server.py:358` | Not covered | [Passed](TEST_REPORT.md#f-338) |
| F-339 | [x] <a id="f-339"></a>`POST /api/shutdown` returns ok true and initiates background server shutdown. | `antiquota/server.py:364` | `tests/test_server.py::test_engine_daemon_is_single_and_discoverable` | [Passed](TEST_REPORT.md#f-339) |
| F-340 | [x] <a id="f-340"></a>`POST /api/sessions` accepts cwd/model/mode/title/worktree, defaults cwd to home and model/mode to persisted defaults, returns metadata; legacy effort is empty. | `antiquota/server.py:369` | `tests/test_server.py::test_session_round_trip_over_http` | [Passed](TEST_REPORT.md#f-340) |
| F-341 | [x] <a id="f-341"></a>`POST /api/sessions/{sid}/term` accepts cols 100/rows 30/restart false and returns alive/shell/cwd. | `antiquota/server.py:373` | `tests/test_engine.py::test_terminal_runs_a_shell_in_the_session_folder` | [Passed](TEST_REPORT.md#f-341) |
| F-342 | [x] <a id="f-342"></a>`POST /api/sessions/{sid}/term/input` accepts data string, returns ok true; no terminal 404, dead terminal 400. | `antiquota/server.py:376` | Not covered | [Passed](TEST_REPORT.md#f-342) |
| F-343 | [x] <a id="f-343"></a>`POST /api/sessions/{sid}/term/resize` accepts cols 100/rows 30, returns ok true even if no terminal exists. | `antiquota/server.py:382` | Not covered | [Passed](TEST_REPORT.md#f-343) |
| F-344 | [x] <a id="f-344"></a>`POST /api/sessions/{sid}/term/kill` removes/kills an existing terminal and returns ok true idempotently. | `antiquota/server.py:387` | Not covered | [Passed](TEST_REPORT.md#f-344) |
| F-345 | [x] <a id="f-345"></a>`GET /api/sessions/{sid}/tree?path=…` returns root/path/entries for one project directory. | `antiquota/server.py:392` | `tests/test_engine.py::test_files_tree_stays_inside_the_session_folder` | [Passed](TEST_REPORT.md#f-345) |
| F-346 | [x] <a id="f-346"></a>`GET /api/sessions/{sid}/git` returns git status/worktree metadata or {}outside a repository. | `antiquota/server.py:394` | `tests/test_gitops.py::test_worktree_session_review_commit_merge` | [Passed](TEST_REPORT.md#f-346) |
| F-347 | [x] <a id="f-347"></a>`GET /api/sessions/{sid}/git/diff` accepts optional path and committed=1 only, returning diff string. | `antiquota/server.py:396` | `tests/test_gitops.py::test_worktree_session_review_commit_merge` | [Passed](TEST_REPORT.md#f-347) |
| F-348 | [x] <a id="f-348"></a>`POST /api/sessions/{sid}/git/commit` accepts message, returns sha and records a notice. | `antiquota/server.py:398` | `tests/test_gitops.py::test_worktree_session_review_commit_merge` | [Passed](TEST_REPORT.md#f-348) |
| F-349 | [x] <a id="f-349"></a>`POST /api/sessions/{sid}/git/discard` accepts path, restores/removes that file and returns ok true. | `antiquota/server.py:400` | `tests/test_gitops.py::test_worktree_session_review_commit_merge` | [Passed](TEST_REPORT.md#f-349) |
| F-350 | [x] <a id="f-350"></a>`POST /api/sessions/{sid}/git/merge` merges the session worktree into its original base and returns sha. | `antiquota/server.py:402` | `tests/test_gitops.py::test_worktree_session_review_commit_merge` | [Passed](TEST_REPORT.md#f-350) |
| F-351 | [x] <a id="f-351"></a>`POST /api/sessions/{sid}/git/remove` accepts delete_branch default false and returns archived main-checkout session metadata. | `antiquota/server.py:404` | `tests/test_gitops.py::test_worktree_session_review_commit_merge` | [Passed](TEST_REPORT.md#f-351) |
| F-352 | [ ] <a id="f-352"></a>`GET /api/sessions/{sid}` returns meta plus all persisted events. | `antiquota/server.py:406` | `tests/test_server.py::test_session_round_trip_over_http` | [Pending](TEST_REPORT.md#f-352) |
| F-353 | [ ] <a id="f-353"></a>`PATCH /api/sessions/{sid}` updates allowed fields only and returns session metadata; bad mode 400. | `antiquota/server.py:408` | `tests/test_server.py::test_session_round_trip_over_http` | [Pending](TEST_REPORT.md#f-353) |
| F-354 | [ ] <a id="f-354"></a>`DELETE /api/sessions/{sid}` stops/removes the session and returns ok true; unknown id 404. | `antiquota/server.py:410` | `tests/test_engine.py::test_terminal_runs_a_shell_in_the_session_folder` | [Pending](TEST_REPORT.md#f-354) |
| F-355 | [ ] <a id="f-355"></a>`POST /api/sessions/{sid}/send` accepts text/attachments, rejects neither text nor attachment, and returns result started/queued. | `antiquota/server.py:413` | `tests/test_server.py::test_session_round_trip_over_http` | [Pending](TEST_REPORT.md#f-355) |
| F-356 | [ ] <a id="f-356"></a>`POST /api/sessions/{sid}/rewind` accepts integer turn default 0 and returns restored relative files. | `antiquota/server.py:417` | `tests/test_engine.py::test_rewind_puts_files_back_and_tells_the_agent` | [Pending](TEST_REPORT.md#f-356) |
| F-357 | [x] <a id="f-357"></a>`POST /api/sessions/{sid}/clear` clears the next conversation and returns ok true. | `antiquota/server.py:419` | `tests/test_engine.py::test_mentions_clear_and_compact` | [Passed](TEST_REPORT.md#f-357) |
| F-358 | [x] <a id="f-358"></a>`POST /api/sessions/{sid}/jobs` accepts nonempty cmd, parsed every and notify default true, returning job metadata. | `antiquota/server.py:422` | `tests/test_engine.py::test_user_job_stop_and_loop` | [Passed](TEST_REPORT.md#f-358) |
| F-359 | [x] <a id="f-359"></a>`POST /api/sessions/{sid}/jobs/{jid}/stop` stops the job tree and returns job metadata. | `antiquota/server.py:426` | `tests/test_engine.py::test_user_job_stop_and_loop` | [Passed](TEST_REPORT.md#f-359) |
| F-360 | [x] <a id="f-360"></a>`GET /api/sessions/{sid}/jobs/{jid}/log` returns job metadata plus bounded log text. | `antiquota/server.py:428` | Not covered | [Passed](TEST_REPORT.md#f-360) |
| F-361 | [x] <a id="f-361"></a>`POST /api/sessions/{sid}/loop` accepts every/prompt or at/prompt, returning ok true and next for daily schedules; invalid interval/prompt/time 400. | `antiquota/server.py:430` | `tests/test_engine.py::test_user_job_stop_and_loop; tests/test_engine.py::test_daily_schedule_fires_at_its_time` | [Passed](TEST_REPORT.md#f-361) |
| F-362 | [x] <a id="f-362"></a>`DELETE /api/sessions/{sid}/loop` removes the loop/schedule and returns ok true. | `antiquota/server.py:439` | `tests/test_engine.py::test_user_job_stop_and_loop` | [Passed](TEST_REPORT.md#f-362) |
| F-363 | [x] <a id="f-363"></a>`GET /api/sessions/{sid}/state` returns the deterministic handoff document as text. | `antiquota/server.py:442` | `tests/test_engine.py::test_handoff_state_and_picking_up_another_session` | [Passed](TEST_REPORT.md#f-363) |
| F-364 | [x] <a id="f-364"></a>`POST /api/sessions/{sid}/inherit` accepts from source session id, adds its handoff note and returns ok true. | `antiquota/server.py:444` | `tests/test_engine.py::test_handoff_state_and_picking_up_another_session` | [Passed](TEST_REPORT.md#f-364) |
| F-365 | [x] <a id="f-365"></a>`POST /api/sessions/{sid}/shell` accepts nonempty cmd and returns persisted shell event with output/code/secs. | `antiquota/server.py:447` | `tests/test_engine.py::test_shell_note_and_file_viewer` | [Passed](TEST_REPORT.md#f-365) |
| F-366 | [x] <a id="f-366"></a>`POST /api/sessions/{sid}/note` accepts nonempty text and returns path/file of updated project instructions. | `antiquota/server.py:451` | `tests/test_engine.py::test_shell_note_and_file_viewer` | [Passed](TEST_REPORT.md#f-366) |
| F-367 | [ ] <a id="f-367"></a>`GET /api/sessions/{sid}/file?path=…` returns absolute path, relative path and decoded text within permitted roots. | `antiquota/server.py:455` | `tests/test_engine.py::test_shell_note_and_file_viewer` | [Pending](TEST_REPORT.md#f-367) |
| F-368 | [x] <a id="f-368"></a>`POST /api/sessions/{sid}/compact` queues a fresh-conversation handoff summary and returns ok true. | `antiquota/server.py:457` | `tests/test_engine.py::test_mentions_clear_and_compact` | [Passed](TEST_REPORT.md#f-368) |
| F-369 | [x] <a id="f-369"></a>`GET /api/sessions/{sid}/files?q=…` returns project file paths for @-mention completion. | `antiquota/server.py:460` | Not covered | [Passed](TEST_REPORT.md#f-369) |
| F-370 | [ ] <a id="f-370"></a>`POST /api/sessions/{sid}/stop` cancels active process/approvals/queue, pauses active goal and returns ok true. | `antiquota/server.py:462` | `tests/test_engine.py::test_stop_ends_the_turn_and_pauses_the_goal` | [Pending](TEST_REPORT.md#f-370) |
| F-371 | [x] <a id="f-371"></a>`POST /api/sessions/{sid}/goal` accepts objective/check/verifier/verifier_model/max_rounds, starts the goal and returns metadata. | `antiquota/server.py:465` | `tests/test_engine.py::test_goal_pushes_on_until_claimed_and_verified` | [Passed](TEST_REPORT.md#f-371) |
| F-372 | [x] <a id="f-372"></a>`POST /api/sessions/{sid}/goal/resume` resets goal round, resumes work and returns ok true. | `antiquota/server.py:470` | Not covered | [Passed](TEST_REPORT.md#f-372) |
| F-373 | [x] <a id="f-373"></a>`DELETE /api/sessions/{sid}/goal` clears only the goal and returns ok true. | `antiquota/server.py:473` | Not covered | [Passed](TEST_REPORT.md#f-373) |
| F-374 | [x] <a id="f-374"></a>`GET /api/usage?days=14` returns local-day/account/model token summaries with days clamped 1–90. | `antiquota/server.py:476` | `tests/test_engine.py::test_plan_mode_plans_by_instruction_and_usage_history` | [Passed](TEST_REPORT.md#f-374) |
| F-375 | [x] <a id="f-375"></a>`GET /api/routing?n=60` returns newest requested routing records in stored order, skipping malformed lines. | `antiquota/server.py:478` | Not covered | [Passed](TEST_REPORT.md#f-375) |
| F-376 | [x] <a id="f-376"></a>`GET /api/accounts` returns account overview rows without credentials. | `antiquota/server.py:480` | `tests/test_server.py::test_add_account_from_the_gui` | [Passed](TEST_REPORT.md#f-376) |
| F-377 | [x] <a id="f-377"></a>`POST /api/accounts/probe` triggers async account probing and returns ok true immediately. | `antiquota/server.py:482` | `tests/test_accounts.py::test_probe_walls_spent_basket_until_panel_reset` | [Passed](TEST_REPORT.md#f-377) |
| F-378 | [x] <a id="f-378"></a>`POST /api/accounts/login` starts/replaces agy's sign-in, returns id/url/seconds 60 or a validation error. | `antiquota/server.py:485` | `tests/test_server.py::test_add_account_from_the_gui` | [Passed](TEST_REPORT.md#f-378) |
| F-379 | [x] <a id="f-379"></a>`GET /api/accounts/login/{lid}` returns state/error/name/verify_url or failed/replaced explanation for unknown/replaced links. | `antiquota/server.py:487` | `tests/test_server.py::test_a_new_sign_in_link_replaces_the_old_one` | [Passed](TEST_REPORT.md#f-379) |
| F-380 | [x] <a id="f-380"></a>`POST /api/accounts/login/{lid}` accepts code and returns state/error/name/verify_url; replaced link or invalid code 400. | `antiquota/server.py:492` | `tests/test_server.py::test_add_account_from_the_gui` | [Passed](TEST_REPORT.md#f-380) |
| F-381 | [x] <a id="f-381"></a>`DELETE /api/accounts/login/{lid}` cancels/removes a pending link and returns ok true even when absent. | `antiquota/server.py:498` | Not covered | [Passed](TEST_REPORT.md#f-381) |
| F-382 | [x] <a id="f-382"></a>`POST /api/accounts/{name}/check` synchronously probes a known idle account and returns updated account list; busy 400 or unknown 404. | `antiquota/server.py:502` | Not covered | [Passed](TEST_REPORT.md#f-382) |
| F-383 | [x] <a id="f-383"></a>`DELETE /api/accounts/{name}` removes an idle valid account and returns ok true, then probes remaining accounts asynchronously. | `antiquota/server.py:504` | `tests/test_server.py::test_add_account_from_the_gui` | [Passed](TEST_REPORT.md#f-383) |
| F-384 | [x] <a id="f-384"></a>`GET /api/settings` returns stack overview plus persisted defaults. | `antiquota/server.py:508` | `tests/test_stack.py::test_overview_compares_with_your_cli_and_ide` | [Passed](TEST_REPORT.md#f-384) |
| F-385 | [x] <a id="f-385"></a>`PUT /api/settings/defaults` updates recognized defaults, validates them and returns the merged setting map. | `antiquota/server.py:510` | `tests/test_stack.py::test_gui_defaults_round_trip` | [Passed](TEST_REPORT.md#f-385) |
| F-386 | [x] <a id="f-386"></a>`GET /api/settings/file?path=…` returns editable file path/text/exists, with containment and size validation. | `antiquota/server.py:512` | `tests/test_stack.py::test_edit_and_delete_touch_only_the_stack` | [Passed](TEST_REPORT.md#f-386) |
| F-387 | [x] <a id="f-387"></a>`PUT /api/settings/file` accepts path/text, writes owned stack only, invalidates stack cache and returns ok true. | `antiquota/server.py:514` | `tests/test_stack.py::test_edit_and_delete_touch_only_the_stack` | [Passed](TEST_REPORT.md#f-387) |
| F-388 | [x] <a id="f-388"></a>`GET /api/settings/{kind}/{name}` returns a hook/MCP JSON object and validates kind/name. | `antiquota/server.py:518` | `tests/test_stack.py::test_edit_and_delete_touch_only_the_stack` | [Passed](TEST_REPORT.md#f-388) |
| F-389 | [x] <a id="f-389"></a>`PUT /api/settings/{kind}/{name}` accepts value object for hook/MCP, invalidates cache and returns ok true. | `antiquota/server.py:520` | `tests/test_stack.py::test_edit_and_delete_touch_only_the_stack` | [Passed](TEST_REPORT.md#f-389) |
| F-390 | [x] <a id="f-390"></a>`DELETE /api/settings/{kind}/{name}` removes an owned skill/plugin/rule/hook/MCP item, invalidates cache and returns ok true. | `antiquota/server.py:524` | `tests/test_stack.py::test_edit_and_delete_touch_only_the_stack` | [Passed](TEST_REPORT.md#f-390) |
| F-391 | [x] <a id="f-391"></a>`POST /api/settings/{kind}/{name}/enabled` accepts on, toggles hook/MCP/plugin, invalidates cache and returns ok true. | `antiquota/server.py:528` | `tests/test_stack.py::test_edit_and_delete_touch_only_the_stack` | [Passed](TEST_REPORT.md#f-391) |
| F-392 | [x] <a id="f-392"></a>`POST /api/settings/copy` accepts source/kind/name optional, copies supported configuration into own stack, invalidates cache and returns ok true. | `antiquota/server.py:532` | `tests/test_stack.py::test_copy_items_in_and_rewrite_paths_into_the_stack` | [Passed](TEST_REPORT.md#f-392) |
| F-393 | [x] <a id="f-393"></a>`GET /api/stack` returns own stack path/loaded skills/hooks/MCP; presence of refresh query forces reload. | `antiquota/server.py:536` | `tests/test_stack.py::test_overview_compares_with_your_cli_and_ide` | [Passed](TEST_REPORT.md#f-393) |
| F-394 | [x] <a id="f-394"></a>`POST /api/stack/reseed` recopies source config preserving history, refreshes stack info and returns path/skills/hooks/MCP. | `antiquota/server.py:538` | `tests/test_accounts.py::test_stack_is_separate_from_your_agy_cli` | [Passed](TEST_REPORT.md#f-394) |
| F-395 | [ ] <a id="f-395"></a>`GET /api/permissions` returns only allow in the legacy contract. | `antiquota/server.py:540` | `tests/test_server.py::test_permissions_are_agy_loops_own_and_folder_listing` | [Pending](TEST_REPORT.md#f-395) |
| F-396 | [ ] <a id="f-396"></a>`PUT /api/permissions` accepts optional allow and deny lists, preserves unspecified list, validates/deduplicates and returns permission object. | `antiquota/server.py:542` | `tests/test_server.py::test_permissions_are_agy_loops_own_and_folder_listing` | [Pending](TEST_REPORT.md#f-396) |
| F-397 | [x] <a id="f-397"></a>`GET /api/fs?path=…` returns home-contained folder picker path/parent/dirs with fallback to home. | `antiquota/server.py:544` | `tests/test_server.py::test_permissions_are_agy_loops_own_and_folder_listing` | [Passed](TEST_REPORT.md#f-397) |

## UI and desktop baseline

### Application foundation

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-398 | [x] <a id="f-398"></a>Authenticated JSON calls send X-Antiquota-Token and display API errors as a toast. | `antiquota/ui/app.js:98` | None identified | [Passed](TEST_REPORT.md#f-398) |
| F-399 | [x] <a id="f-399"></a>The session event stream uses a token-authenticated EventSource. | `antiquota/ui/app.js:1029` | None identified | [Passed](TEST_REPORT.md#f-399) |
| F-400 | [x] <a id="f-400"></a>A reconnected event stream reloads authoritative state after a disconnect. | `antiquota/ui/app.js:1029` | None identified | [Passed](TEST_REPORT.md#f-400) |
| F-401 | [x] <a id="f-401"></a>Initial load selects the URL-fragment session when it exists. | `antiquota/ui/app.js:2511` | None identified | [Passed](TEST_REPORT.md#f-401) |
| F-402 | [x] <a id="f-402"></a>Initial load otherwise opens the first session or the start screen. | `antiquota/ui/app.js:2511` | None identified | [Passed](TEST_REPORT.md#f-402) |
| F-403 | [x] <a id="f-403"></a>A missing initial model list is retried after four seconds. | `antiquota/ui/app.js:2511` | None identified | [Passed](TEST_REPORT.md#f-403) |
| F-404 | [x] <a id="f-404"></a>A toast replaces the prior timer and clears after 2.6 seconds. | `antiquota/ui/app.js:111` | None identified | [Passed](TEST_REPORT.md#f-404) |
| F-405 | [x] <a id="f-405"></a>Confirmation dialogs support Continue, Cancel, Escape and dangerous-action styling. | `antiquota/ui/app.js:24` | None identified | [Passed](TEST_REPORT.md#f-405) |
| F-406 | [ ] <a id="f-406"></a>Desktop export uses the native save dialog. | `antiquota/ui/app.js:39` | None identified | [Pending](TEST_REPORT.md#f-406) |
| F-407 | [x] <a id="f-407"></a>Browser export creates a downloadable file. | `antiquota/ui/app.js:39` | None identified | [Passed](TEST_REPORT.md#f-407) |
| F-408 | [ ] <a id="f-408"></a>External desktop links open in the system browser. | `antiquota/ui/app.js:18` | None identified | [Pending](TEST_REPORT.md#f-408) |
| F-409 | [ ] <a id="f-409"></a>Clipboard reads use native APIs in desktop mode and browser APIs otherwise. | `antiquota/ui/app.js:19` | None identified | [Pending](TEST_REPORT.md#f-409) |
| F-410 | [ ] <a id="f-410"></a>Clipboard writes use native APIs in desktop mode and browser APIs otherwise. | `antiquota/ui/app.js:20` | None identified | [Pending](TEST_REPORT.md#f-410) |
| F-411 | [x] <a id="f-411"></a>Malformed or unavailable localStorage does not prevent preference initialization. | `antiquota/ui/app.js:69` | None identified | [Passed](TEST_REPORT.md#f-411) |
| F-412 | [x] <a id="f-412"></a>Preference writes tolerate unavailable localStorage. | `antiquota/ui/app.js:96` | None identified | [Passed](TEST_REPORT.md#f-412) |
### Sessions and projects

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-413 | [x] <a id="f-413"></a>Session rows display title, relative update time and status. | `antiquota/ui/app.js:224` | None identified | [Passed](TEST_REPORT.md#f-413) |
| F-414 | [x] <a id="f-414"></a>Session rows identify pinned sessions and worktree sessions. | `antiquota/ui/app.js:224` | None identified | [Passed](TEST_REPORT.md#f-414) |
| F-415 | [x] <a id="f-415"></a>Sleeping sessions show the wake time. | `antiquota/ui/app.js:224` | None identified | [Passed](TEST_REPORT.md#f-415) |
| F-416 | [x] <a id="f-416"></a>Sessions awaiting approval display an attention message. | `antiquota/ui/app.js:224` | None identified | [Passed](TEST_REPORT.md#f-416) |
| F-417 | [x] <a id="f-417"></a>Goal sessions show the current round and limit or final goal state. | `antiquota/ui/app.js:224` | None identified | [Passed](TEST_REPORT.md#f-417) |
| F-418 | [x] <a id="f-418"></a>Sessions sort by most recent update within project groups. | `antiquota/ui/app.js:246` | None identified | [Passed](TEST_REPORT.md#f-418) |
| F-419 | [x] <a id="f-419"></a>Pinned sessions appear in a separate Pinned group. | `antiquota/ui/app.js:246` | None identified | [Passed](TEST_REPORT.md#f-419) |
| F-420 | [x] <a id="f-420"></a>Worktree sessions group by their parent repository. | `antiquota/ui/app.js:246` | None identified | [Passed](TEST_REPORT.md#f-420) |
| F-421 | [x] <a id="f-421"></a>Project collapse state persists per device. | `antiquota/ui/app.js:246` | None identified | [Passed](TEST_REPORT.md#f-421) |
| F-422 | [x] <a id="f-422"></a>The current session's project remains expanded despite a collapsed preference. | `antiquota/ui/app.js:246` | None identified | [Passed](TEST_REPORT.md#f-422) |
| F-423 | [x] <a id="f-423"></a>Collapsed projects show session count and working-session indicator. | `antiquota/ui/app.js:246` | None identified | [Passed](TEST_REPORT.md#f-423) |
| F-424 | [x] <a id="f-424"></a>Search temporarily expands matching project groups. | `antiquota/ui/app.js:246` | None identified | [Passed](TEST_REPORT.md#f-424) |
| F-425 | [x] <a id="f-425"></a>Session filtering matches title and working folder without case sensitivity. | `antiquota/ui/app.js:246` | None identified | [Passed](TEST_REPORT.md#f-425) |
| F-426 | [x] <a id="f-426"></a>No sessions and no title matches have distinct empty messages. | `antiquota/ui/app.js:246` | None identified | [Passed](TEST_REPORT.md#f-426) |
| F-427 | [x] <a id="f-427"></a>Searches of two or more characters query message contents after 220 ms. | `antiquota/ui/app.js:292` | None identified | [Passed](TEST_REPORT.md#f-427) |
| F-428 | [x] <a id="f-428"></a>Older message-search responses cannot replace newer results. | `antiquota/ui/app.js:292` | None identified | [Passed](TEST_REPORT.md#f-428) |
| F-429 | [x] <a id="f-429"></a>Search hits show title, author, snippet, time and folder tooltip. | `antiquota/ui/app.js:292` | None identified | [Passed](TEST_REPORT.md#f-429) |
| F-430 | [x] <a id="f-430"></a>Show archived exposes archived sessions and Hide archived conceals them. | `antiquota/ui/app.js:246` | None identified | [Passed](TEST_REPORT.md#f-430) |
| F-431 | [x] <a id="f-431"></a>The current archived session stays visible while archives are hidden. | `antiquota/ui/app.js:246` | None identified | [Passed](TEST_REPORT.md#f-431) |
| F-432 | [x] <a id="f-432"></a>Session Rename opens the session and its title editor. | `antiquota/ui/app.js:306` | None identified | [Passed](TEST_REPORT.md#f-432) |
| F-433 | [x] <a id="f-433"></a>Session Pin to top persists the pinned state. | `antiquota/ui/app.js:306` | None identified | [Passed](TEST_REPORT.md#f-433) |
| F-434 | [x] <a id="f-434"></a>Session Unpin clears the pinned state. | `antiquota/ui/app.js:306` | None identified | [Passed](TEST_REPORT.md#f-434) |
| F-435 | [x] <a id="f-435"></a>Session Fork creates and opens a session from handoff state. | `antiquota/ui/app.js:306` | None identified | [Passed](TEST_REPORT.md#f-435) |
| F-436 | [x] <a id="f-436"></a>Session Archive persists archived state. | `antiquota/ui/app.js:306` | None identified | [Passed](TEST_REPORT.md#f-436) |
| F-437 | [x] <a id="f-437"></a>Session Unarchive clears archived state. | `antiquota/ui/app.js:306` | None identified | [Passed](TEST_REPORT.md#f-437) |
| F-438 | [x] <a id="f-438"></a>Session Delete requires confirmation and preserves project files. | `antiquota/ui/app.js:306` | None identified | [Passed](TEST_REPORT.md#f-438) |
| F-439 | [x] <a id="f-439"></a>Project New session here preselects its folder without a worktree. | `antiquota/ui/app.js:329` | None identified | [Passed](TEST_REPORT.md#f-439) |
| F-440 | [x] <a id="f-440"></a>Project New session in a git worktree preselects its folder with a worktree. | `antiquota/ui/app.js:329` | None identified | [Passed](TEST_REPORT.md#f-440) |
| F-441 | [x] <a id="f-441"></a>Collapse every other project persists the other project folders as collapsed. | `antiquota/ui/app.js:329` | None identified | [Passed](TEST_REPORT.md#f-441) |
| F-442 | [x] <a id="f-442"></a>Expand every project clears project collapse preferences. | `antiquota/ui/app.js:329` | None identified | [Passed](TEST_REPORT.md#f-442) |
| F-443 | [ ] <a id="f-443"></a>Show in Finder / Explorer reveals the project folder in desktop mode. | `antiquota/ui/app.js:329` | None identified | [Pending](TEST_REPORT.md#f-443) |
| F-444 | [x] <a id="f-444"></a>Copy path copies the complete project-folder path. | `antiquota/ui/app.js:329` | None identified | [Passed](TEST_REPORT.md#f-444) |
| F-445 | [x] <a id="f-445"></a>Archive its sessions archives every listed unarchived session. | `antiquota/ui/app.js:329` | None identified | [Passed](TEST_REPORT.md#f-445) |
| F-446 | [x] <a id="f-446"></a>Delete archived sessions confirms before deleting listed archived transcripts. | `antiquota/ui/app.js:329` | None identified | [Passed](TEST_REPORT.md#f-446) |
| F-447 | [x] <a id="f-447"></a>Delete all sessions confirms before deleting every listed transcript. | `antiquota/ui/app.js:329` | None identified | [Passed](TEST_REPORT.md#f-447) |
| F-448 | [x] <a id="f-448"></a>Session actions open from right-click and the More button. | `antiquota/ui/app.js:306` | None identified | [Passed](TEST_REPORT.md#f-448) |
| F-449 | [x] <a id="f-449"></a>Project actions open from right-click and the Project actions button. | `antiquota/ui/app.js:329` | None identified | [Passed](TEST_REPORT.md#f-449) |
| F-450 | [x] <a id="f-450"></a>Context menus keep their right and bottom edges inside the viewport. | `antiquota/ui/app.js:306` | None identified | [Passed](TEST_REPORT.md#f-450) |
| F-451 | [x] <a id="f-451"></a>Clicking outside a context menu closes it. | `antiquota/ui/app.js:367` | None identified | [Passed](TEST_REPORT.md#f-451) |
| F-452 | [x] <a id="f-452"></a>Opening a session replaces its transcript and focuses the composer. | `antiquota/ui/app.js:968` | None identified | [Passed](TEST_REPORT.md#f-452) |
| F-453 | [x] <a id="f-453"></a>Opening a session restores that session's user-prompt history. | `antiquota/ui/app.js:968` | None identified | [Passed](TEST_REPORT.md#f-453) |
| F-454 | [x] <a id="f-454"></a>A stale open-session response does not overwrite a newer selection. | `antiquota/ui/app.js:968` | None identified | [Passed](TEST_REPORT.md#f-454) |
| F-455 | [x] <a id="f-455"></a>Switching sessions clears unsent attachment chips. | `antiquota/ui/app.js:968` | None identified | [Passed](TEST_REPORT.md#f-455) |
| F-456 | [x] <a id="f-456"></a>Deleting the current session opens the next recent session or start screen. | `antiquota/ui/app.js:996` | None identified | [Passed](TEST_REPORT.md#f-456) |
| F-457 | [x] <a id="f-457"></a>An empty session offers up to three earlier sessions from the same folder. | `antiquota/ui/app.js:606` | None identified | [Passed](TEST_REPORT.md#f-457) |
| F-458 | [x] <a id="f-458"></a>Selecting an earlier session inherits its handoff state for the next prompt. | `antiquota/ui/app.js:606` | None identified | [Passed](TEST_REPORT.md#f-458) |
| F-459 | [x] <a id="f-459"></a>Start and empty-session screens offer sign-in when no accounts exist. | `antiquota/ui/app.js:584` | None identified | [Passed](TEST_REPORT.md#f-459) |
| F-460 | [x] <a id="f-460"></a>The empty-session screen offers Set a goal and explains its working folder. | `antiquota/ui/app.js:590` | None identified | [Passed](TEST_REPORT.md#f-460) |
| F-461 | [x] <a id="f-461"></a>The no-session start screen offers New session. | `antiquota/ui/app.js:616` | None identified | [Passed](TEST_REPORT.md#f-461) |
### Composer, models and header

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-462 | [x] <a id="f-462"></a>The header shows session title and shortened folder or worktree branch. | `antiquota/ui/app.js:416` | None identified | [Passed](TEST_REPORT.md#f-462) |
| F-463 | [x] <a id="f-463"></a>The header path tooltip exposes the complete folder or worktree origin/base. | `antiquota/ui/app.js:416` | None identified | [Passed](TEST_REPORT.md#f-463) |
| F-464 | [x] <a id="f-464"></a>The header shows working, approval, waiting, checking, verifying and sleeping states. | `antiquota/ui/app.js:416` | None identified | [Passed](TEST_REPORT.md#f-464) |
| F-465 | [x] <a id="f-465"></a>The header shows cumulative input/output tokens and queued-message count. | `antiquota/ui/app.js:416` | None identified | [Passed](TEST_REPORT.md#f-465) |
| F-466 | [x] <a id="f-466"></a>The page title marks working state and includes the session title. | `antiquota/ui/app.js:416` | None identified | [Passed](TEST_REPORT.md#f-466) |
| F-467 | [x] <a id="f-467"></a>An idle empty composer disables Send. | `antiquota/ui/app.js:445` | None identified | [Passed](TEST_REPORT.md#f-467) |
| F-468 | [x] <a id="f-468"></a>A busy empty composer exposes Stop. | `antiquota/ui/app.js:445` | None identified | [Passed](TEST_REPORT.md#f-468) |
| F-469 | [x] <a id="f-469"></a>A busy nonempty composer exposes Queue. | `antiquota/ui/app.js:445` | None identified | [Passed](TEST_REPORT.md#f-469) |
| F-470 | [x] <a id="f-470"></a>The composer grows to fit text up to 40 percent of window height. | `antiquota/ui/app.js:1095` | None identified | [Passed](TEST_REPORT.md#f-470) |
| F-471 | [x] <a id="f-471"></a>Sending posts text and completed attachment paths to the current session. | `antiquota/ui/app.js:1100` | None identified | [Passed](TEST_REPORT.md#f-471) |
| F-472 | [x] <a id="f-472"></a>Sending is blocked while an attachment is uploading. | `antiquota/ui/app.js:1100` | None identified | [Passed](TEST_REPORT.md#f-472) |
| F-473 | [x] <a id="f-473"></a>A failed send restores message text for retry. | `antiquota/ui/app.js:1100` | None identified | [Passed](TEST_REPORT.md#f-473) |
| F-474 | [x] <a id="f-474"></a>Text entered without a session is sent after creating a session. | `antiquota/ui/app.js:1100` | None identified | [Passed](TEST_REPORT.md#f-474) |
| F-475 | [x] <a id="f-475"></a>Attachment-only messages can be sent without text. | `antiquota/ui/app.js:1100` | None identified | [Passed](TEST_REPORT.md#f-475) |
| F-476 | [x] <a id="f-476"></a>Queued text explains mid-turn delivery at the next step. | `antiquota/ui/app.js:1100` | None identified | [Passed](TEST_REPORT.md#f-476) |
| F-477 | [x] <a id="f-477"></a>Queued attachments explain that delivery follows the current turn. | `antiquota/ui/app.js:1100` | None identified | [Passed](TEST_REPORT.md#f-477) |
| F-478 | [x] <a id="f-478"></a>A ! command runs in the project folder and supplies output to the next prompt. | `antiquota/ui/app.js:1123` | None identified | [Passed](TEST_REPORT.md#f-478) |
| F-479 | [x] <a id="f-479"></a>A # note appends to project GEMINI.md and reports the saved file. | `antiquota/ui/app.js:1123` | None identified | [Passed](TEST_REPORT.md#f-479) |
| F-480 | [x] <a id="f-480"></a>Shell prefixes change styling, hints and the action label to Run. | `antiquota/ui/app.js:445` | None identified | [Passed](TEST_REPORT.md#f-480) |
| F-481 | [x] <a id="f-481"></a>Note prefixes change styling, hints and the action label to Save. | `antiquota/ui/app.js:445` | None identified | [Passed](TEST_REPORT.md#f-481) |
| F-482 | [x] <a id="f-482"></a>Prompt history restores earlier messages and returns to the current draft. | `antiquota/ui/app.js:1386` | None identified | [Passed](TEST_REPORT.md#f-482) |
| F-483 | [x] <a id="f-483"></a>Models group into families and Gemini versus Claude/GPT quota pools. | `antiquota/ui/app.js:1045` | None identified | [Passed](TEST_REPORT.md#f-483) |
| F-484 | [x] <a id="f-484"></a>An unknown current model remains selectable. | `antiquota/ui/app.js:1045` | None identified | [Passed](TEST_REPORT.md#f-484) |
| F-485 | [x] <a id="f-485"></a>Families with one effort disable effort selection. | `antiquota/ui/app.js:1066` | None identified | [Passed](TEST_REPORT.md#f-485) |
| F-486 | [x] <a id="f-486"></a>Changing family preserves matching effort, then prefers high effort. | `antiquota/ui/app.js:1075` | None identified | [Passed](TEST_REPORT.md#f-486) |
| F-487 | [x] <a id="f-487"></a>Verifier choices expose concrete model IDs and effort labels. | `antiquota/ui/app.js:1082` | None identified | [Passed](TEST_REPORT.md#f-487) |
| F-488 | [x] <a id="f-488"></a>Changing the composer model family patches the session model. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-488) |
| F-489 | [x] <a id="f-489"></a>Changing the composer effort patches the session model. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-489) |
| F-490 | [x] <a id="f-490"></a>Changing the composer permission mode patches the session. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-490) |
| F-491 | [x] <a id="f-491"></a>Selecting bypass from the composer requires dangerous-action confirmation. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-491) |
| F-492 | [x] <a id="f-492"></a>Header rename saves a changed nonempty title on Enter. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-492) |
| F-493 | [x] <a id="f-493"></a>Header rename saves a changed nonempty title on blur. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-493) |
| F-494 | [x] <a id="f-494"></a>Escape cancels header rename. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-494) |
| F-495 | [x] <a id="f-495"></a>The meter shows context fraction with warm and hot thresholds. | `antiquota/ui/app.js:2486` | None identified | [Passed](TEST_REPORT.md#f-495) |
| F-496 | [x] <a id="f-496"></a>The meter shows cumulative input, output and thinking tokens. | `antiquota/ui/app.js:2486` | None identified | [Passed](TEST_REPORT.md#f-496) |
| F-497 | [x] <a id="f-497"></a>The meter shows serving account and its lowest-window quota in the selected pool. | `antiquota/ui/app.js:2486` | None identified | [Passed](TEST_REPORT.md#f-497) |
| F-498 | [x] <a id="f-498"></a>The meter shows the number of currently available accounts in the pool. | `antiquota/ui/app.js:2486` | None identified | [Passed](TEST_REPORT.md#f-498) |
| F-499 | [x] <a id="f-499"></a>Model context capacities are 200000 for Claude, 131072 for GPT-OSS and 1048576 otherwise. | `antiquota/ui/app.js:2484` | None identified | [Passed](TEST_REPORT.md#f-499) |
### Transcript rendering

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-500 | [x] <a id="f-500"></a>Model content is escaped before supported Markdown is inserted. | `antiquota/ui/app.js:142` | None identified | [Passed](TEST_REPORT.md#f-500) |
| F-501 | [x] <a id="f-501"></a>Markdown renders fenced code blocks. | `antiquota/ui/app.js:156` | None identified | [Passed](TEST_REPORT.md#f-501) |
| F-502 | [x] <a id="f-502"></a>Markdown renders four heading levels. | `antiquota/ui/app.js:156` | None identified | [Passed](TEST_REPORT.md#f-502) |
| F-503 | [x] <a id="f-503"></a>Markdown renders ordered and unordered lists. | `antiquota/ui/app.js:156` | None identified | [Passed](TEST_REPORT.md#f-503) |
| F-504 | [x] <a id="f-504"></a>Markdown renders blockquotes. | `antiquota/ui/app.js:156` | None identified | [Passed](TEST_REPORT.md#f-504) |
| F-505 | [x] <a id="f-505"></a>Markdown renders tables with a header separator. | `antiquota/ui/app.js:156` | None identified | [Passed](TEST_REPORT.md#f-505) |
| F-506 | [x] <a id="f-506"></a>Markdown renders paragraphs and inline bold, emphasis and code. | `antiquota/ui/app.js:156` | None identified | [Passed](TEST_REPORT.md#f-506) |
| F-507 | [x] <a id="f-507"></a>HTTP Markdown links open externally and unsupported schemes remain text. | `antiquota/ui/app.js:142` | None identified | [Passed](TEST_REPORT.md#f-507) |
| F-508 | [x] <a id="f-508"></a>File Markdown links open the file viewer. | `antiquota/ui/app.js:142` | None identified | [Passed](TEST_REPORT.md#f-508) |
| F-509 | [x] <a id="f-509"></a>Code-block Copy copies only the code contents. | `antiquota/ui/app.js:200` | None identified | [Passed](TEST_REPORT.md#f-509) |
| F-510 | [x] <a id="f-510"></a>Copy message copies the completed assistant reply. | `antiquota/ui/app.js:865` | None identified | [Passed](TEST_REPORT.md#f-510) |
| F-511 | [x] <a id="f-511"></a>User messages identify user, goal, nudge, job and loop origins. | `antiquota/ui/app.js:626` | None identified | [Passed](TEST_REPORT.md#f-511) |
| F-512 | [x] <a id="f-512"></a>Visible goal prompts omit repeated standing instructions but retain full tooltip text. | `antiquota/ui/app.js:626` | None identified | [Passed](TEST_REPORT.md#f-512) |
| F-513 | [x] <a id="f-513"></a>Steered messages are marked as arriving while the agent worked. | `antiquota/ui/app.js:626` | None identified | [Passed](TEST_REPORT.md#f-513) |
| F-514 | [x] <a id="f-514"></a>Turn markers show number, account, model, clock time and rewind. | `antiquota/ui/app.js:871` | None identified | [Passed](TEST_REPORT.md#f-514) |
| F-515 | [x] <a id="f-515"></a>Streaming deltas update one assistant node until its completed reply replaces them. | `antiquota/ui/app.js:871` | None identified | [Passed](TEST_REPORT.md#f-515) |
| F-516 | [x] <a id="f-516"></a>Completed tool rows replace running rows without losing expanded state. | `antiquota/ui/app.js:871` | None identified | [Passed](TEST_REPORT.md#f-516) |
| F-517 | [x] <a id="f-517"></a>Tool rows identify run, read, list, search, find, edit, web and MCP actions. | `antiquota/ui/app.js:636` | None identified | [Passed](TEST_REPORT.md#f-517) |
| F-518 | [x] <a id="f-518"></a>Unknown tool rows display a readable name and short string argument. | `antiquota/ui/app.js:636` | None identified | [Passed](TEST_REPORT.md#f-518) |
| F-519 | [x] <a id="f-519"></a>Tool rows show running, completed or error state with elapsed time. | `antiquota/ui/app.js:691` | None identified | [Passed](TEST_REPORT.md#f-519) |
| F-520 | [x] <a id="f-520"></a>Tool rows show added and deleted line counts for diffs. | `antiquota/ui/app.js:691` | None identified | [Passed](TEST_REPORT.md#f-520) |
| F-521 | [x] <a id="f-521"></a>ANSI terminal color sequences are removed from command output. | `antiquota/ui/app.js:691` | None identified | [Passed](TEST_REPORT.md#f-521) |
| F-522 | [x] <a id="f-522"></a>Command previews show the final four lines with an additional-line indicator. | `antiquota/ui/app.js:669` | None identified | [Passed](TEST_REPORT.md#f-522) |
| F-523 | [x] <a id="f-523"></a>Diff previews show up to six changed lines with an additional-line indicator. | `antiquota/ui/app.js:669` | None identified | [Passed](TEST_REPORT.md#f-523) |
| F-524 | [x] <a id="f-524"></a>Expanded tools display complete diff or output. | `antiquota/ui/app.js:683` | None identified | [Passed](TEST_REPORT.md#f-524) |
| F-525 | [x] <a id="f-525"></a>Expanded tools include extended errors or JSON parameters when output is absent. | `antiquota/ui/app.js:683` | None identified | [Passed](TEST_REPORT.md#f-525) |
| F-526 | [x] <a id="f-526"></a>Tool headers and previews toggle complete output expansion. | `antiquota/ui/app.js:691` | None identified | [Passed](TEST_REPORT.md#f-526) |
| F-527 | [x] <a id="f-527"></a>Grouped mode folds adjacent tool steps into one expandable summary. | `antiquota/ui/app.js:730` | None identified | [Passed](TEST_REPORT.md#f-527) |
| F-528 | [x] <a id="f-528"></a>Grouped summaries count unique read and edited paths. | `antiquota/ui/app.js:730` | None identified | [Passed](TEST_REPORT.md#f-528) |
| F-529 | [x] <a id="f-529"></a>Grouped summaries show additions, deletions, failures and step count. | `antiquota/ui/app.js:730` | None identified | [Passed](TEST_REPORT.md#f-529) |
| F-530 | [x] <a id="f-530"></a>Grouped summaries show the active running tool. | `antiquota/ui/app.js:730` | None identified | [Passed](TEST_REPORT.md#f-530) |
| F-531 | [x] <a id="f-531"></a>Thinking blocks display reasoning Markdown and token counts when present. | `antiquota/ui/app.js:767` | None identified | [Passed](TEST_REPORT.md#f-531) |
| F-532 | [x] <a id="f-532"></a>Short thinking blocks start expanded in detailed mode. | `antiquota/ui/app.js:767` | None identified | [Passed](TEST_REPORT.md#f-532) |
| F-533 | [x] <a id="f-533"></a>Compact mode hides previews and changes thinking expansion. | `antiquota/ui/app.js:2470` | None identified | [Passed](TEST_REPORT.md#f-533) |
| F-534 | [x] <a id="f-534"></a>Detailed or compact mode persists per device. | `antiquota/ui/app.js:2470` | None identified | [Passed](TEST_REPORT.md#f-534) |
| F-535 | [x] <a id="f-535"></a>Expand all controls tool groups, individual output and thinking blocks. | `antiquota/ui/app.js:2478` | None identified | [Passed](TEST_REPORT.md#f-535) |
| F-536 | [x] <a id="f-536"></a>Turn completion shows status, duration, input/output use and serving account. | `antiquota/ui/app.js:838` | None identified | [Passed](TEST_REPORT.md#f-536) |
| F-537 | [x] <a id="f-537"></a>Completed and failed turns remove live transcript markers. | `antiquota/ui/app.js:871` | None identified | [Passed](TEST_REPORT.md#f-537) |
| F-538 | [x] <a id="f-538"></a>Events auto-scroll only within 120 pixels of the transcript bottom. | `antiquota/ui/app.js:576` | None identified | [Passed](TEST_REPORT.md#f-538) |
| F-539 | [x] <a id="f-539"></a>Handoff notices identify the transition to another account. | `antiquota/ui/app.js:785` | None identified | [Passed](TEST_REPORT.md#f-539) |
| F-540 | [x] <a id="f-540"></a>Quota-wall and no-account notices have distinct actionable presentations. | `antiquota/ui/app.js:785` | None identified | [Passed](TEST_REPORT.md#f-540) |
| F-541 | [x] <a id="f-541"></a>Goal notices indicate whether the objective is met. | `antiquota/ui/app.js:785` | None identified | [Passed](TEST_REPORT.md#f-541) |
| F-542 | [x] <a id="f-542"></a>Check notices show pass/fail and expandable output. | `antiquota/ui/app.js:785` | None identified | [Passed](TEST_REPORT.md#f-542) |
| F-543 | [x] <a id="f-543"></a>Verifier notices show done, not met or indeterminate state. | `antiquota/ui/app.js:785` | None identified | [Passed](TEST_REPORT.md#f-543) |
| F-544 | [x] <a id="f-544"></a>Verifier notices show missing work and verifier account/model. | `antiquota/ui/app.js:785` | None identified | [Passed](TEST_REPORT.md#f-544) |
| F-545 | [x] <a id="f-545"></a>Errors display their error text without markup interpretation. | `antiquota/ui/app.js:785` | None identified | [Passed](TEST_REPORT.md#f-545) |
| F-546 | [x] <a id="f-546"></a>Denied-command notices offer adding the suggested rules. | `antiquota/ui/app.js:785` | None identified | [Passed](TEST_REPORT.md#f-546) |
| F-547 | [x] <a id="f-547"></a>Allow and retry grants suggested command rules and sends a retry prompt. | `antiquota/ui/app.js:785` | None identified | [Passed](TEST_REPORT.md#f-547) |
| F-548 | [x] <a id="f-548"></a>Denied edits without command rules offer accept edits and retry. | `antiquota/ui/app.js:785` | None identified | [Passed](TEST_REPORT.md#f-548) |
| F-549 | [x] <a id="f-549"></a>Rewind requires a specified turn and confirmation before restoring files. | `antiquota/ui/app.js:855` | None identified | [Passed](TEST_REPORT.md#f-549) |
| F-550 | [x] <a id="f-550"></a>Rewind notices identify files changed outside the agent after its edit. | `antiquota/ui/app.js:785` | None identified | [Passed](TEST_REPORT.md#f-550) |
| F-551 | [x] <a id="f-551"></a>Markdown export includes prompts, replies, reasoning, tools, turns and notices. | `antiquota/ui/app.js:1414` | None identified | [Passed](TEST_REPORT.md#f-551) |
| F-552 | [x] <a id="f-552"></a>Export uses a sanitized title limited to sixty characters plus .md. | `antiquota/ui/app.js:1414` | None identified | [Passed](TEST_REPORT.md#f-552) |
### Goals, plans, jobs and approvals

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-553 | [x] <a id="f-553"></a>The goal dialog reuses an existing goal or composer objective. | `antiquota/ui/app.js:1970` | None identified | [Passed](TEST_REPORT.md#f-553) |
| F-554 | [x] <a id="f-554"></a>Goal controls expose objective, optional check, verifier, model and round limit. | `antiquota/ui/app.js:1970` | None identified | [Passed](TEST_REPORT.md#f-554) |
| F-555 | [x] <a id="f-555"></a>Start goal posts all selected controls. | `antiquota/ui/app.js:1981` | None identified | [Passed](TEST_REPORT.md#f-555) |
| F-556 | [x] <a id="f-556"></a>Starting an identical composer objective clears that composer text. | `antiquota/ui/app.js:1981` | None identified | [Passed](TEST_REPORT.md#f-556) |
| F-557 | [x] <a id="f-557"></a>Goal bars show objective, state/rounds, check and verifier model. | `antiquota/ui/app.js:462` | None identified | [Passed](TEST_REPORT.md#f-557) |
| F-558 | [x] <a id="f-558"></a>Goal Pause stops an active session. | `antiquota/ui/app.js:462` | None identified | [Passed](TEST_REPORT.md#f-558) |
| F-559 | [x] <a id="f-559"></a>Goal Resume restarts an unmet inactive goal. | `antiquota/ui/app.js:462` | None identified | [Passed](TEST_REPORT.md#f-559) |
| F-560 | [x] <a id="f-560"></a>Goal Edit opens the existing goal dialog. | `antiquota/ui/app.js:462` | None identified | [Passed](TEST_REPORT.md#f-560) |
| F-561 | [x] <a id="f-561"></a>Goal Remove deletes its goal. | `antiquota/ui/app.js:462` | None identified | [Passed](TEST_REPORT.md#f-561) |
| F-562 | [x] <a id="f-562"></a>A successful completed idle plan-mode turn exposes plan-ready controls. | `antiquota/ui/app.js:486` | None identified | [Passed](TEST_REPORT.md#f-562) |
| F-563 | [x] <a id="f-563"></a>Execute plan switches to accept-edits before sending its execution instruction. | `antiquota/ui/app.js:486` | None identified | [Passed](TEST_REPORT.md#f-563) |
| F-564 | [x] <a id="f-564"></a>Execute with approvals switches to default before sending its execution instruction. | `antiquota/ui/app.js:486` | None identified | [Passed](TEST_REPORT.md#f-564) |
| F-565 | [x] <a id="f-565"></a>Revise focuses the composer with a revision placeholder. | `antiquota/ui/app.js:486` | None identified | [Passed](TEST_REPORT.md#f-565) |
| F-566 | [x] <a id="f-566"></a>Running jobs show command, elapsed time, check-in and agent attribution. | `antiquota/ui/app.js:507` | None identified | [Passed](TEST_REPORT.md#f-566) |
| F-567 | [x] <a id="f-567"></a>Job Log opens that job's output. | `antiquota/ui/app.js:507` | None identified | [Passed](TEST_REPORT.md#f-567) |
| F-568 | [x] <a id="f-568"></a>Job Stop requests ending that job. | `antiquota/ui/app.js:507` | None identified | [Passed](TEST_REPORT.md#f-568) |
| F-569 | [x] <a id="f-569"></a>Job logs show no-output text when empty and scroll to the latest output. | `antiquota/ui/app.js:528` | None identified | [Passed](TEST_REPORT.md#f-569) |
| F-570 | [x] <a id="f-570"></a>Job events distinguish started, stopped, success, failure and unknown exit code. | `antiquota/ui/app.js:565` | None identified | [Passed](TEST_REPORT.md#f-570) |
| F-571 | [x] <a id="f-571"></a>Loop bars show prompt, interval and next run. | `antiquota/ui/app.js:507` | None identified | [Passed](TEST_REPORT.md#f-571) |
| F-572 | [x] <a id="f-572"></a>Daily schedule bars show prompt, daily time and next run. | `antiquota/ui/app.js:507` | None identified | [Passed](TEST_REPORT.md#f-572) |
| F-573 | [x] <a id="f-573"></a>Loop Stop deletes the configured loop. | `antiquota/ui/app.js:507` | None identified | [Passed](TEST_REPORT.md#f-573) |
| F-574 | [x] <a id="f-574"></a>Schedule Remove deletes the configured schedule. | `antiquota/ui/app.js:507` | None identified | [Passed](TEST_REPORT.md#f-574) |
| F-575 | [x] <a id="f-575"></a>Pending approval cards show command or edit target, preview and wait duration. | `antiquota/ui/app.js:537` | None identified | [Passed](TEST_REPORT.md#f-575) |
| F-576 | [x] <a id="f-576"></a>Allow once answers without persisting a rule. | `antiquota/ui/app.js:537` | None identified | [Passed](TEST_REPORT.md#f-576) |
| F-577 | [x] <a id="f-577"></a>Always allow answers with the suggested permanent command rule. | `antiquota/ui/app.js:537` | None identified | [Passed](TEST_REPORT.md#f-577) |
| F-578 | [x] <a id="f-578"></a>Allow all edits switches the session's edit permission behavior. | `antiquota/ui/app.js:537` | None identified | [Passed](TEST_REPORT.md#f-578) |
| F-579 | [x] <a id="f-579"></a>Decline sends a denial. | `antiquota/ui/app.js:537` | None identified | [Passed](TEST_REPORT.md#f-579) |
| F-580 | [x] <a id="f-580"></a>Answered cards show allow/decline, once/permanent state and reason. | `antiquota/ui/app.js:537` | None identified | [Passed](TEST_REPORT.md#f-580) |
| F-581 | [ ] <a id="f-581"></a>Missing approval answers display the expired-request explanation. | `antiquota/ui/app.js:537` | None identified | [Pending](TEST_REPORT.md#f-581) |
| F-582 | [ ] <a id="f-582"></a>A new pending approval requests native attention and an approval notification. | `antiquota/ui/app.js:871` | None identified | [Pending](TEST_REPORT.md#f-582) |
### Files and Git review

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-583 | [x] <a id="f-583"></a>The Files tab lists the selected session's folders and files. | `antiquota/ui/app.js:1545` | None identified | [Passed](TEST_REPORT.md#f-583) |
| F-584 | [x] <a id="f-584"></a>Breadcrumbs navigate to earlier directories. | `antiquota/ui/app.js:1545` | None identified | [Passed](TEST_REPORT.md#f-584) |
| F-585 | [x] <a id="f-585"></a>The parent row navigates to the parent directory. | `antiquota/ui/app.js:1545` | None identified | [Passed](TEST_REPORT.md#f-585) |
| F-586 | [x] <a id="f-586"></a>File rows display size and dimmed entries. | `antiquota/ui/app.js:1545` | None identified | [Passed](TEST_REPORT.md#f-586) |
| F-587 | [x] <a id="f-587"></a>An empty directory shows Empty folder. | `antiquota/ui/app.js:1545` | None identified | [Passed](TEST_REPORT.md#f-587) |
| F-588 | [x] <a id="f-588"></a>Mention inserts a file's @ path into the composer. | `antiquota/ui/app.js:1545` | None identified | [Passed](TEST_REPORT.md#f-588) |
| F-589 | [ ] <a id="f-589"></a>Desktop Open launches a file's default application. | `antiquota/ui/app.js:1545` | None identified | [Pending](TEST_REPORT.md#f-589) |
| F-590 | [ ] <a id="f-590"></a>Desktop Reveal opens Finder / Explorer for a file or directory. | `antiquota/ui/app.js:1545` | None identified | [Pending](TEST_REPORT.md#f-590) |
| F-591 | [x] <a id="f-591"></a>File links open the numbered file viewer. | `antiquota/ui/app.js:1219` | None identified | [Passed](TEST_REPORT.md#f-591) |
| F-592 | [x] <a id="f-592"></a>The viewer shows relative path and full-path tooltip. | `antiquota/ui/app.js:1224` | None identified | [Passed](TEST_REPORT.md#f-592) |
| F-593 | [x] <a id="f-593"></a>Line or line-range links highlight and center those lines. | `antiquota/ui/app.js:1224` | None identified | [Passed](TEST_REPORT.md#f-593) |
| F-594 | [x] <a id="f-594"></a>Viewer Copy copies complete contents. | `antiquota/ui/app.js:1224` | None identified | [Passed](TEST_REPORT.md#f-594) |
| F-595 | [x] <a id="f-595"></a>Viewer Close dismisses the viewer. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-595) |
| F-596 | [x] <a id="f-596"></a>Clicking the viewer backdrop dismisses the viewer. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-596) |
| F-597 | [x] <a id="f-597"></a>Files and Terminal tabs explain that a session is required when none is open. | `antiquota/ui/app.js:1895` | None identified | [Passed](TEST_REPORT.md#f-597) |
| F-598 | [x] <a id="f-598"></a>Git review shows branch and repository or worktree base/ahead/merged state. | `antiquota/ui/app.js:1573` | None identified | [Passed](TEST_REPORT.md#f-598) |
| F-599 | [x] <a id="f-599"></a>Uncommitted file rows show status, additions and deletions. | `antiquota/ui/app.js:1573` | None identified | [Passed](TEST_REPORT.md#f-599) |
| F-600 | [x] <a id="f-600"></a>Expanding uncommitted files lazily requests their diff. | `antiquota/ui/app.js:1573` | None identified | [Passed](TEST_REPORT.md#f-600) |
| F-601 | [x] <a id="f-601"></a>Expanding worktree committed files lazily requests the base-relative diff. | `antiquota/ui/app.js:1573` | None identified | [Passed](TEST_REPORT.md#f-601) |
| F-602 | [x] <a id="f-602"></a>Discard confirms deletion of a new file or loss of uncommitted edits. | `antiquota/ui/app.js:1573` | None identified | [Passed](TEST_REPORT.md#f-602) |
| F-603 | [x] <a id="f-603"></a>Commit all sends the message and refreshes Git review. | `antiquota/ui/app.js:1573` | None identified | [Passed](TEST_REPORT.md#f-603) |
| F-604 | [x] <a id="f-604"></a>The initial commit message uses the selected session title. | `antiquota/ui/app.js:1573` | None identified | [Passed](TEST_REPORT.md#f-604) |
| F-605 | [x] <a id="f-605"></a>Worktree review lists files committed relative to its base. | `antiquota/ui/app.js:1573` | None identified | [Passed](TEST_REPORT.md#f-605) |
| F-606 | [x] <a id="f-606"></a>Merge is enabled only for an unmerged worktree with commits ahead. | `antiquota/ui/app.js:1573` | None identified | [Passed](TEST_REPORT.md#f-606) |
| F-607 | [x] <a id="f-607"></a>Remove worktree confirms removal, archives the session and keeps its branch. | `antiquota/ui/app.js:1573` | None identified | [Passed](TEST_REPORT.md#f-607) |
| F-608 | [x] <a id="f-608"></a>Agent edit history groups recorded diffs by file, newest first. | `antiquota/ui/app.js:1895` | None identified | [Passed](TEST_REPORT.md#f-608) |
| F-609 | [x] <a id="f-609"></a>Expanding an edit-history file lazily displays all recorded diffs. | `antiquota/ui/app.js:1895` | None identified | [Passed](TEST_REPORT.md#f-609) |
| F-610 | [x] <a id="f-610"></a>Changes displays distinct no-session, no-edits and no-uncommitted messages. | `antiquota/ui/app.js:1895` | None identified | [Passed](TEST_REPORT.md#f-610) |
### Split, drawer and terminal

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-611 | [x] <a id="f-611"></a>Split opens a second independently selected session. | `antiquota/ui/app.js:1626` | None identified | [Passed](TEST_REPORT.md#f-611) |
| F-612 | [x] <a id="f-612"></a>Split selection persists and restores on reload when still valid. | `antiquota/ui/app.js:1626` | None identified | [Passed](TEST_REPORT.md#f-612) |
| F-613 | [x] <a id="f-613"></a>Opening split hides the side panel. | `antiquota/ui/app.js:1626` | None identified | [Passed](TEST_REPORT.md#f-613) |
| F-614 | [x] <a id="f-614"></a>Split choices list up to fourteen recent unarchived other sessions. | `antiquota/ui/app.js:1644` | None identified | [Passed](TEST_REPORT.md#f-614) |
| F-615 | [x] <a id="f-615"></a>Split choices identify busy sessions and no-other-session state. | `antiquota/ui/app.js:1644` | None identified | [Passed](TEST_REPORT.md#f-615) |
| F-616 | [x] <a id="f-616"></a>Close split removes the second session and persisted selection. | `antiquota/ui/app.js:1637` | None identified | [Passed](TEST_REPORT.md#f-616) |
| F-617 | [x] <a id="f-617"></a>Swap sides exchanges the left and right sessions. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-617) |
| F-618 | [x] <a id="f-618"></a>The embedded Close side by side action closes split view. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-618) |
| F-619 | [x] <a id="f-619"></a>Cross-origin split control messages are ignored. | `antiquota/ui/app.js:1661` | None identified | [Passed](TEST_REPORT.md#f-619) |
| F-620 | [x] <a id="f-620"></a>Embedded views omit sidebar, drawer and nested split controls. | `antiquota/ui/app.js:17` | None identified | [Passed](TEST_REPORT.md#f-620) |
| F-621 | [x] <a id="f-621"></a>Side-panel visibility persists per device. | `antiquota/ui/app.js:1671` | None identified | [Passed](TEST_REPORT.md#f-621) |
| F-622 | [x] <a id="f-622"></a>The Changes, Files or Terminal tab selection persists per device. | `antiquota/ui/app.js:1671` | None identified | [Passed](TEST_REPORT.md#f-622) |
| F-623 | [x] <a id="f-623"></a>Open side-panel content follows session selection. | `antiquota/ui/app.js:968` | None identified | [Passed](TEST_REPORT.md#f-623) |
| F-624 | [x] <a id="f-624"></a>Terminal assets are loaded lazily once. | `antiquota/ui/app.js:1820` | None identified | [Passed](TEST_REPORT.md#f-624) |
| F-625 | [x] <a id="f-625"></a>A terminal asset failure displays a useful load error. | `antiquota/ui/app.js:1833` | None identified | [Passed](TEST_REPORT.md#f-625) |
| F-626 | [x] <a id="f-626"></a>Each session retains an independent xterm instance across tab/session switches. | `antiquota/ui/app.js:1833` | None identified | [Passed](TEST_REPORT.md#f-626) |
| F-627 | [x] <a id="f-627"></a>Terminal starts a shell in the session folder and shows shell/folder in its title. | `antiquota/ui/app.js:1833` | None identified | [Passed](TEST_REPORT.md#f-627) |
| F-628 | [x] <a id="f-628"></a>Terminal keyboard input is posted to the engine. | `antiquota/ui/app.js:1833` | None identified | [Passed](TEST_REPORT.md#f-628) |
| F-629 | [x] <a id="f-629"></a>Streamed base64 bytes render as terminal output. | `antiquota/ui/app.js:1833` | None identified | [Passed](TEST_REPORT.md#f-629) |
| F-630 | [x] <a id="f-630"></a>First replayed output resets the terminal display. | `antiquota/ui/app.js:1833` | None identified | [Passed](TEST_REPORT.md#f-630) |
| F-631 | [x] <a id="f-631"></a>Resize fits xterm and posts dimensions after an 80 ms debounce. | `antiquota/ui/app.js:1833` | None identified | [Passed](TEST_REPORT.md#f-631) |
| F-632 | [x] <a id="f-632"></a>Clear clears the display without restarting the shell. | `antiquota/ui/app.js:1833` | None identified | [Passed](TEST_REPORT.md#f-632) |
| F-633 | [x] <a id="f-633"></a>Restart requests a fresh shell and reconnects output. | `antiquota/ui/app.js:1833` | None identified | [Passed](TEST_REPORT.md#f-633) |
| F-634 | [x] <a id="f-634"></a>Kill ends the shell. | `antiquota/ui/app.js:1833` | None identified | [Passed](TEST_REPORT.md#f-634) |
| F-635 | [x] <a id="f-635"></a>Exit marks the title exited and closes the output stream. | `antiquota/ui/app.js:1833` | None identified | [Passed](TEST_REPORT.md#f-635) |
| F-636 | [x] <a id="f-636"></a>Terminal input and startup errors render in red inside xterm. | `antiquota/ui/app.js:1833` | None identified | [Passed](TEST_REPORT.md#f-636) |
| F-637 | [x] <a id="f-637"></a>Opening or revisiting the Terminal tab focuses it. | `antiquota/ui/app.js:1833` | None identified | [Passed](TEST_REPORT.md#f-637) |
| F-638 | [x] <a id="f-638"></a>Terminal scrollback retains 5000 lines. | `antiquota/ui/app.js:1833` | None identified | [Passed](TEST_REPORT.md#f-638) |
### Attachments and dictation

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-639 | [x] <a id="f-639"></a>The picker uploads multiple files for the selected session. | `antiquota/ui/app.js:1351` | None identified | [Passed](TEST_REPORT.md#f-639) |
| F-640 | [x] <a id="f-640"></a>Pasted screenshot files upload with generic image.png renamed by timestamp. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-640) |
| F-641 | [x] <a id="f-641"></a>Dragging files into the window displays the drop overlay. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-641) |
| F-642 | [x] <a id="f-642"></a>Dropping files hides the overlay and attaches those files. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-642) |
| F-643 | [x] <a id="f-643"></a>Nested drag tracking keeps the overlay until every file drag leaves. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-643) |
| F-644 | [x] <a id="f-644"></a>Attaching without a session requests opening or creating one. | `antiquota/ui/app.js:1351` | None identified | [Passed](TEST_REPORT.md#f-644) |
| F-645 | [x] <a id="f-645"></a>Files over 25 MB are rejected by filename. | `antiquota/ui/app.js:1351` | None identified | [Passed](TEST_REPORT.md#f-645) |
| F-646 | [x] <a id="f-646"></a>Chips display thumbnail or type, filename, size and upload state. | `antiquota/ui/app.js:1341` | None identified | [Passed](TEST_REPORT.md#f-646) |
| F-647 | [x] <a id="f-647"></a>Remove excludes an unsent attachment from the next message. | `antiquota/ui/app.js:1341` | None identified | [Passed](TEST_REPORT.md#f-647) |
| F-648 | [x] <a id="f-648"></a>Failed uploads remove their chip and display the error. | `antiquota/ui/app.js:1351` | None identified | [Passed](TEST_REPORT.md#f-648) |
| F-649 | [x] <a id="f-649"></a>Sent image attachments render as authenticated clickable thumbnails. | `antiquota/ui/app.js:1377` | None identified | [Passed](TEST_REPORT.md#f-649) |
| F-650 | [x] <a id="f-650"></a>Sent non-image attachments render as filename/type chips. | `antiquota/ui/app.js:1377` | None identified | [Passed](TEST_REPORT.md#f-650) |
| F-651 | [x] <a id="f-651"></a>Dictation starts from idle and stops from listening or recording. | `antiquota/ui/app.js:1266` | None identified | [Passed](TEST_REPORT.md#f-651) |
| F-652 | [x] <a id="f-652"></a>Dictation cannot restart while transcription is running. | `antiquota/ui/app.js:1266` | None identified | [Passed](TEST_REPORT.md#f-652) |
| F-653 | [x] <a id="f-653"></a>The mic and status label indicate listening, recording and transcribing. | `antiquota/ui/app.js:1245` | None identified | [Passed](TEST_REPORT.md#f-653) |
| F-654 | [x] <a id="f-654"></a>Recording status shows elapsed time and an Escape-cancel hint. | `antiquota/ui/app.js:1245` | None identified | [Passed](TEST_REPORT.md#f-654) |
| F-655 | [x] <a id="f-655"></a>Browser recognition uses continuous interim results in navigator language. | `antiquota/ui/app.js:1272` | None identified | [Passed](TEST_REPORT.md#f-655) |
| F-656 | [x] <a id="f-656"></a>Recognition appends recognized text after current composer content. | `antiquota/ui/app.js:1272` | None identified | [Passed](TEST_REPORT.md#f-656) |
| F-657 | [x] <a id="f-657"></a>Permission, service and network errors fall back to audio recording. | `antiquota/ui/app.js:1272` | None identified | [Passed](TEST_REPORT.md#f-657) |
| F-658 | [x] <a id="f-658"></a>Aborted or no-speech recognition ends without an error toast. | `antiquota/ui/app.js:1272` | None identified | [Passed](TEST_REPORT.md#f-658) |
| F-659 | [x] <a id="f-659"></a>Capture requests mono microphone input with echo cancellation and noise suppression. | `antiquota/ui/app.js:1286` | None identified | [Passed](TEST_REPORT.md#f-659) |
| F-660 | [x] <a id="f-660"></a>Microphone access failure displays its error. | `antiquota/ui/app.js:1286` | None identified | [Passed](TEST_REPORT.md#f-660) |
| F-661 | [x] <a id="f-661"></a>Stopping or canceling disconnects audio nodes and stops microphone tracks. | `antiquota/ui/app.js:1297` | None identified | [Passed](TEST_REPORT.md#f-661) |
| F-662 | [x] <a id="f-662"></a>Captures shorter than half a second are rejected before upload. | `antiquota/ui/app.js:1297` | None identified | [Passed](TEST_REPORT.md#f-662) |
| F-663 | [x] <a id="f-663"></a>Peak amplitude below 0.01 is rejected as silence before upload. | `antiquota/ui/app.js:1297` | None identified | [Passed](TEST_REPORT.md#f-663) |
| F-664 | [x] <a id="f-664"></a>Legacy recording uploads 16000 Hz mono PCM WAV. Authorized replacement: use MediaRecorder directly and upload a supported recorded container with the correct X-Audio-Type; preserve recorded-WAV input tests and the client/server silence guard. | `antiquota/ui/app.js:1319` | None identified | [Passed](TEST_REPORT.md#f-664) |
| F-665 | [x] <a id="f-665"></a>Empty transcription reports Nothing heard without inserting text. | `antiquota/ui/app.js:1297` | None identified | [Passed](TEST_REPORT.md#f-665) |
| F-666 | [x] <a id="f-666"></a>Transcription inserts at the selection and restores composer focus. | `antiquota/ui/app.js:1258` | None identified | [Passed](TEST_REPORT.md#f-666) |
| F-667 | [x] <a id="f-667"></a>Escape cancels recording. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-667) |
| F-668 | [x] <a id="f-668"></a>Escape aborts live speech recognition. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-668) |
### Settings sections

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-669 | [ ] <a id="f-669"></a>General settings exposes new-session defaults, keep-awake, autostart and notification controls. | `antiquota/ui/app.js:2098` | None identified | [Pending](TEST_REPORT.md#f-669) |
| F-670 | [ ] <a id="f-670"></a>Appearance settings exposes desktop size and transcript presentation controls. | `antiquota/ui/app.js:2098` | None identified | [Pending](TEST_REPORT.md#f-670) |
| F-671 | [x] <a id="f-671"></a>Keyboard settings exposes shortcuts and slash-command reference. | `antiquota/ui/app.js:2098` | None identified | [Passed](TEST_REPORT.md#f-671) |
| F-672 | [x] <a id="f-672"></a>Accounts settings exposes accounts, sign-in, rotation controls and routing history. | `antiquota/ui/app.js:2098` | None identified | [Passed](TEST_REPORT.md#f-672) |
| F-673 | [x] <a id="f-673"></a>Usage settings exposes period controls, statistics, chart and account/model breakdowns. | `antiquota/ui/app.js:2098` | None identified | [Passed](TEST_REPORT.md#f-673) |
| F-674 | [x] <a id="f-674"></a>Agents & goals settings exposes verifier, round, approval and denial controls. | `antiquota/ui/app.js:2098` | None identified | [Passed](TEST_REPORT.md#f-674) |
| F-675 | [x] <a id="f-675"></a>Permissions settings exposes independent allow and deny rules. | `antiquota/ui/app.js:2098` | None identified | [Passed](TEST_REPORT.md#f-675) |
| F-676 | [x] <a id="f-676"></a>Rules settings exposes private GEMINI.md and triggered rule files. | `antiquota/ui/app.js:2098` | None identified | [Passed](TEST_REPORT.md#f-676) |
| F-677 | [x] <a id="f-677"></a>Skills settings exposes installed and source skill comparisons and private editors. | `antiquota/ui/app.js:2098` | None identified | [Passed](TEST_REPORT.md#f-677) |
| F-678 | [x] <a id="f-678"></a>Hooks settings exposes event hook entries and enable/edit/copy/delete controls. | `antiquota/ui/app.js:2098` | None identified | [Passed](TEST_REPORT.md#f-678) |
| F-679 | [x] <a id="f-679"></a>MCP servers settings exposes tool servers and their configuration controls. | `antiquota/ui/app.js:2098` | None identified | [Passed](TEST_REPORT.md#f-679) |
| F-680 | [x] <a id="f-680"></a>Plugins settings exposes installed bundles and copying differing CLI bundles. | `antiquota/ui/app.js:2098` | None identified | [Passed](TEST_REPORT.md#f-680) |
| F-681 | [ ] <a id="f-681"></a>Engine settings exposes diagnostics, log, data directory, restart and private-stack replacement. | `antiquota/ui/app.js:2098` | None identified | [Pending](TEST_REPORT.md#f-681) |
| F-682 | [x] <a id="f-682"></a>Settings immediately displays cached content or a loading message while reading fresh data. | `antiquota/ui/app.js:2115` | None identified | [Passed](TEST_REPORT.md#f-682) |
| F-683 | [x] <a id="f-683"></a>Fresh data does not redraw an active editor or focused settings input. | `antiquota/ui/app.js:2124` | None identified | [Passed](TEST_REPORT.md#f-683) |
| F-684 | [ ] <a id="f-684"></a>Navigation shows installed item counts, account attention and private stack path. | `antiquota/ui/app.js:2131` | None identified | [Pending](TEST_REPORT.md#f-684) |
| F-685 | [x] <a id="f-685"></a>Close settings hides the panel and closes the active editor. | `antiquota/ui/app.js:2123` | None identified | [Passed](TEST_REPORT.md#f-685) |
### General and appearance controls

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-686 | [x] <a id="f-686"></a>Default model family saves the selected concrete model and effort. | `antiquota/ui/app.js:2251` | None identified | [Passed](TEST_REPORT.md#f-686) |
| F-687 | [x] <a id="f-687"></a>Default effort saves the chosen model variant. | `antiquota/ui/app.js:2251` | None identified | [Passed](TEST_REPORT.md#f-687) |
| F-688 | [x] <a id="f-688"></a>Default mode persists default, accept-edits, plan or bypass. | `antiquota/ui/app.js:2251` | None identified | [Passed](TEST_REPORT.md#f-688) |
| F-689 | [x] <a id="f-689"></a>Default folder persists the entered folder and falls back to home when empty. | `antiquota/ui/app.js:2251` | None identified | [Passed](TEST_REPORT.md#f-689) |
| F-690 | [x] <a id="f-690"></a>Separate git worktree by default persists on/off. | `antiquota/ui/app.js:2251` | None identified | [Passed](TEST_REPORT.md#f-690) |
| F-691 | [x] <a id="f-691"></a>Keep awake while agents work persists on/off. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-691) |
| F-692 | [ ] <a id="f-692"></a>Open at login reads current native autostart state. | `antiquota/ui/app.js:2226` | None identified | [Pending](TEST_REPORT.md#f-692) |
| F-693 | [ ] <a id="f-693"></a>Open at login enables or disables native autostart. | `antiquota/ui/app.js:2226` | None identified | [Pending](TEST_REPORT.md#f-693) |
| F-694 | [ ] <a id="f-694"></a>An unavailable native autostart adapter removes its settings row. | `antiquota/ui/app.js:2226` | None identified | [Pending](TEST_REPORT.md#f-694) |
| F-695 | [x] <a id="f-695"></a>Turn-finished notifications can be enabled or disabled independently. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-695) |
| F-696 | [x] <a id="f-696"></a>Goal-met notifications can be enabled or disabled independently. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-696) |
| F-697 | [x] <a id="f-697"></a>Approval-needed notifications can be enabled or disabled independently. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-697) |
| F-698 | [x] <a id="f-698"></a>Job-finished notifications can be enabled or disabled independently. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-698) |
| F-699 | [x] <a id="f-699"></a>Exhausted-quota notifications can be enabled or disabled independently. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-699) |
| F-700 | [x] <a id="f-700"></a>Also while antiquota is in front persists focused-window notification preference. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-700) |
| F-701 | [x] <a id="f-701"></a>Focused visible app windows suppress notifications unless their focused preference is on. | `antiquota/ui/app.js:1398` | None identified | [Passed](TEST_REPORT.md#f-701) |
| F-702 | [ ] <a id="f-702"></a>Native notification permission is requested before sending an allowed notification. | `antiquota/ui/app.js:1398` | None identified | [Pending](TEST_REPORT.md#f-702) |
| F-703 | [x] <a id="f-703"></a>Browser notifications require granted permission and tolerate notification failures. | `antiquota/ui/app.js:1398` | None identified | [Passed](TEST_REPORT.md#f-703) |
| F-704 | [x] <a id="f-704"></a>The first browser message requests notification permission when its state is default. | `antiquota/ui/app.js:1100` | None identified | [Passed](TEST_REPORT.md#f-704) |
| F-705 | [ ] <a id="f-705"></a>Native Interface size offers 80, 90, 100, 110, 125 and 140 percent presets. | `antiquota/ui/app.js:2318` | None identified | [Pending](TEST_REPORT.md#f-705) |
| F-706 | [x] <a id="f-706"></a>Browser Interface size explains the browser zoom controls instead of applying CSS scale. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-706) |
| F-707 | [x] <a id="f-707"></a>Grouped steps saves grouped mode and rebuilds the selected transcript. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-707) |
| F-708 | [x] <a id="f-708"></a>Every step saves ungrouped mode and rebuilds the selected transcript. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-708) |
| F-709 | [x] <a id="f-709"></a>Previews and thinking selects detailed or compact transcript mode. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-709) |
| F-710 | [x] <a id="f-710"></a>Times on turns persists timestamp visibility. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-710) |
### Account cards, rotation and usage

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-711 | [x] <a id="f-711"></a>Quotas refresh without a manual button. Legacy Settings had Probe now; on the owner's request (2026-10-09) it is removed because the engine refreshes idle accounts every Quota check minutes. The Accounts view says so, a background refresh updates the open cards, and `/quota` and each card's Check still refresh on demand. The UI says refresh, not probe. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-711) |
| F-712 | [x] <a id="f-712"></a>Accounts Add account opens sign-in. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-712) |
| F-713 | [x] <a id="f-713"></a>Account cards show identity and paused, busy and last-probed states. | `antiquota/ui/app.js:1773` | None identified | [Passed](TEST_REPORT.md#f-713) |
| F-714 | [x] <a id="f-714"></a>Account cards show today's input, output, thinking and turn counts. | `antiquota/ui/app.js:1773` | None identified | [Passed](TEST_REPORT.md#f-714) |
| F-715 | [x] <a id="f-715"></a>Gemini and Claude/GPT quota windows appear separately. | `antiquota/ui/app.js:1773` | None identified | [Passed](TEST_REPORT.md#f-715) |
| F-716 | [x] <a id="f-716"></a>Quota windows show their reset time or disabled-window explanation in a tooltip. | `antiquota/ui/app.js:1773` | None identified | [Passed](TEST_REPORT.md#f-716) |
| F-717 | [x] <a id="f-717"></a>Account cards show walls, signed-out, cooldown and verification reasons. | `antiquota/ui/app.js:1773` | None identified | [Passed](TEST_REPORT.md#f-717) |
| F-718 | [x] <a id="f-718"></a>Pause keeps an account out of rotation. | `antiquota/ui/app.js:1773` | None identified | [Passed](TEST_REPORT.md#f-718) |
| F-719 | [x] <a id="f-719"></a>Resume puts a paused account back into rotation. | `antiquota/ui/app.js:1773` | None identified | [Passed](TEST_REPORT.md#f-719) |
| F-720 | [x] <a id="f-720"></a>Check refreshes quota and sign-in state. | `antiquota/ui/app.js:1773` | None identified | [Passed](TEST_REPORT.md#f-720) |
| F-721 | [x] <a id="f-721"></a>Sign in again opens the account sign-in dialog. | `antiquota/ui/app.js:1773` | None identified | [Passed](TEST_REPORT.md#f-721) |
| F-722 | [ ] <a id="f-722"></a>Remove requires confirmation and preserves separate CLI sign-in. | `antiquota/ui/app.js:1773` | None identified | [Pending](TEST_REPORT.md#f-722) |
| F-723 | [x] <a id="f-723"></a>A verification-required account opens its exact Google verification URL. | `antiquota/ui/app.js:1687` | None identified | [Passed](TEST_REPORT.md#f-723) |
| F-724 | [x] <a id="f-724"></a>Check again reports readiness or continued verification after probing. | `antiquota/ui/app.js:1687` | None identified | [Passed](TEST_REPORT.md#f-724) |
| F-725 | [x] <a id="f-725"></a>Next account selects soonest-reset-first, one-by-one or round-robin rotation. | `antiquota/ui/app.js:2273` | None identified | [Passed](TEST_REPORT.md#f-725) |
| F-726 | [x] <a id="f-726"></a>Reserve accepts and persists 0–50 percent. | `antiquota/ui/app.js:2273` | None identified | [Passed](TEST_REPORT.md#f-726) |
| F-727 | [x] <a id="f-727"></a>Quota check accepts and persists 2–240 minutes. | `antiquota/ui/app.js:2273` | None identified | [Passed](TEST_REPORT.md#f-727) |
| F-728 | [x] <a id="f-728"></a>Routing history displays newest-first pick decisions with session and prior account. | `antiquota/ui/app.js:1799` | None identified | [Passed](TEST_REPORT.md#f-728) |
| F-729 | [x] <a id="f-729"></a>Routing history displays walls, cooldowns, retries, signed-out, paused and resumed decisions. | `antiquota/ui/app.js:1799` | None identified | [Passed](TEST_REPORT.md#f-729) |
| F-730 | [x] <a id="f-730"></a>No routing decisions has an explicit empty state. | `antiquota/ui/app.js:1799` | None identified | [Passed](TEST_REPORT.md#f-730) |
| F-731 | [x] <a id="f-731"></a>Usage selects seven, fourteen or thirty days. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-731) |
| F-732 | [x] <a id="f-732"></a>Usage displays period total tokens and turns. | `antiquota/ui/app.js:1708` | None identified | [Passed](TEST_REPORT.md#f-732) |
| F-733 | [x] <a id="f-733"></a>Usage displays today's tokens and turns. | `antiquota/ui/app.js:1708` | None identified | [Passed](TEST_REPORT.md#f-733) |
| F-734 | [x] <a id="f-734"></a>Usage displays peak day and mean tokens per turn. | `antiquota/ui/app.js:1708` | None identified | [Passed](TEST_REPORT.md#f-734) |
| F-735 | [x] <a id="f-735"></a>Daily chart bars highlight the latest day with the accent color. | `antiquota/ui/app.js:1708` | None identified | [Passed](TEST_REPORT.md#f-735) |
| F-736 | [x] <a id="f-736"></a>Hover shows date, tokens, turns and account contributions sorted by tokens. | `antiquota/ui/app.js:1708` | None identified | [Passed](TEST_REPORT.md#f-736) |
| F-737 | [x] <a id="f-737"></a>Chart tooltips stay inside left, middle and right chart boundaries. | `antiquota/ui/app.js:1708` | None identified | [Passed](TEST_REPORT.md#f-737) |
| F-738 | [x] <a id="f-738"></a>Usage account tables show period tokens, today tokens and turns ordered by total tokens. | `antiquota/ui/app.js:1756` | None identified | [Passed](TEST_REPORT.md#f-738) |
| F-739 | [x] <a id="f-739"></a>Usage model tables show tokens and percentage share ordered by tokens. | `antiquota/ui/app.js:1762` | None identified | [Passed](TEST_REPORT.md#f-739) |
| F-740 | [x] <a id="f-740"></a>Empty periods display No turns in this period yet. | `antiquota/ui/app.js:1759` | None identified | [Passed](TEST_REPORT.md#f-740) |
| F-741 | [x] <a id="f-741"></a>The sidebar account pill displays available count and serving account/quota. | `antiquota/ui/app.js:385` | None identified | [Passed](TEST_REPORT.md#f-741) |
| F-742 | [x] <a id="f-742"></a>The sidebar account strip shows account quota and paused/wall/attention status. | `antiquota/ui/app.js:385` | None identified | [Passed](TEST_REPORT.md#f-742) |
| F-743 | [x] <a id="f-743"></a>The account pill opens Usage when accounts exist. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-743) |
| F-744 | [x] <a id="f-744"></a>The account pill opens Add Google account when no accounts exist. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-744) |
### Agent, permission and stack controls

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-745 | [x] <a id="f-745"></a>Verifier model saves a concrete verifier model ID. | `antiquota/ui/app.js:2286` | None identified | [Passed](TEST_REPORT.md#f-745) |
| F-746 | [x] <a id="f-746"></a>Rounds accepts and saves 1–500 rounds. | `antiquota/ui/app.js:2286` | None identified | [Passed](TEST_REPORT.md#f-746) |
| F-747 | [x] <a id="f-747"></a>Approval wait accepts and saves 1–240 minutes. | `antiquota/ui/app.js:2286` | None identified | [Passed](TEST_REPORT.md#f-747) |
| F-748 | [x] <a id="f-748"></a>Retries after a denial accepts and saves 0–10 retries. | `antiquota/ui/app.js:2286` | None identified | [Passed](TEST_REPORT.md#f-748) |
| F-749 | [x] <a id="f-749"></a>Plan mode explains its read-only planning and execution workflow. | `antiquota/ui/app.js:2286` | None identified | [Passed](TEST_REPORT.md#f-749) |
| F-750 | [x] <a id="f-750"></a>Allow rules can be listed, added and individually removed. | `antiquota/ui/app.js:2307` | None identified | [Passed](TEST_REPORT.md#f-750) |
| F-751 | [x] <a id="f-751"></a>Deny rules can be listed, added and individually removed. | `antiquota/ui/app.js:2307` | None identified | [Passed](TEST_REPORT.md#f-751) |
| F-752 | [x] <a id="f-752"></a>Copy from agy CLI replaces differing private permission rules. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-752) |
| F-753 | [x] <a id="f-753"></a>Rules edits private GEMINI.md with Save and Cancel. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-753) |
| F-754 | [x] <a id="f-754"></a>New rule creates an always_on template and adds .md when absent. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-754) |
| F-755 | [x] <a id="f-755"></a>New skill creates SKILL.md with name and description metadata. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-755) |
| F-756 | [x] <a id="f-756"></a>New hook creates a named PostToolUse command-hook template. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-756) |
| F-757 | [x] <a id="f-757"></a>Add server creates a named MCP command/argument template. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-757) |
| F-758 | [x] <a id="f-758"></a>Plugins lists private bundles and imports differing CLI bundles. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-758) |
| F-759 | [x] <a id="f-759"></a>Stack rows show same, different or source-only state for each supported source. | `antiquota/ui/app.js:2188` | None identified | [Passed](TEST_REPORT.md#f-759) |
| F-760 | [x] <a id="f-760"></a>Enable or disable preserves a stack item without deleting it. | `antiquota/ui/app.js:2188` | None identified | [Passed](TEST_REPORT.md#f-760) |
| F-761 | [x] <a id="f-761"></a>Edit opens the private rule or skill text editor. | `antiquota/ui/app.js:2188` | None identified | [Passed](TEST_REPORT.md#f-761) |
| F-762 | [x] <a id="f-762"></a>Edit opens the private hook or MCP JSON editor. | `antiquota/ui/app.js:2188` | None identified | [Passed](TEST_REPORT.md#f-762) |
| F-763 | [x] <a id="f-763"></a>Copy or Replace imports the selected source, kind and optional name into the private stack. | `antiquota/ui/app.js:2144` | None identified | [Passed](TEST_REPORT.md#f-763) |
| F-764 | [x] <a id="f-764"></a>Copy all differing items imports every differing item from its supported source. | `antiquota/ui/app.js:2155` | None identified | [Passed](TEST_REPORT.md#f-764) |
| F-765 | [x] <a id="f-765"></a>Delete requires confirmation and preserves the source CLI or IDE item. | `antiquota/ui/app.js:2188` | None identified | [Passed](TEST_REPORT.md#f-765) |
| F-766 | [x] <a id="f-766"></a>Invalid editor JSON is rejected before saving. | `antiquota/ui/app.js:2178` | None identified | [Passed](TEST_REPORT.md#f-766) |
| F-767 | [x] <a id="f-767"></a>Text editors load the private file, save changes or cancel the editor. | `antiquota/ui/app.js:2171` | None identified | [Passed](TEST_REPORT.md#f-767) |
| F-768 | [x] <a id="f-768"></a>JSON editors load the private item, save valid changes or cancel the editor. | `antiquota/ui/app.js:2178` | None identified | [Passed](TEST_REPORT.md#f-768) |
| F-769 | [x] <a id="f-769"></a>New item names allow letters, digits, dot, underscore, colon, at, plus and minus. | `antiquota/ui/app.js:2210` | None identified | [Passed](TEST_REPORT.md#f-769) |
| F-770 | [x] <a id="f-770"></a>New item names reject unsafe characters and a leading dot. | `antiquota/ui/app.js:2210` | None identified | [Passed](TEST_REPORT.md#f-770) |
| F-771 | [x] <a id="f-771"></a>An empty stack section displays Nothing here yet. | `antiquota/ui/app.js:250` | None identified | [Passed](TEST_REPORT.md#f-771) |
| F-772 | [x] <a id="f-772"></a>Engine diagnostics shows version, URL/PID/ownership, data, stack, agy, hook and counts. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-772) |
| F-773 | [x] <a id="f-773"></a>Engine log opens engine output in the viewer. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-773) |
| F-774 | [ ] <a id="f-774"></a>Show data folder reveals the private directory in desktop mode. | `antiquota/ui/app.js:2318` | None identified | [Pending](TEST_REPORT.md#f-774) |
| F-775 | [ ] <a id="f-775"></a>Restart engine confirms before requesting shutdown and reports the expected app restart. | `antiquota/ui/app.js:2318` | None identified | [Pending](TEST_REPORT.md#f-775) |
| F-776 | [x] <a id="f-776"></a>Copy everything again from agy CLI confirms replacement of private configuration edits. | `antiquota/ui/app.js:2318` | None identified | [Passed](TEST_REPORT.md#f-776) |
### Account sign-in states

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-777 | [x] <a id="f-777"></a>Opening Add Google account resets prior code, sign-in state and timers. | `antiquota/ui/app.js:1996` | None identified | [Passed](TEST_REPORT.md#f-777) |
| F-778 | [x] <a id="f-778"></a>Browser sign-in reserves a tab before requesting the URL to avoid popup blocking. | `antiquota/ui/app.js:2013` | None identified | [Passed](TEST_REPORT.md#f-778) |
| F-779 | [ ] <a id="f-779"></a>Native sign-in opens the returned consent URL in the system browser. | `antiquota/ui/app.js:2013` | None identified | [Pending](TEST_REPORT.md#f-779) |
| F-780 | [x] <a id="f-780"></a>Sign-in exposes a manual link when opening the browser fails. | `antiquota/ui/app.js:2013` | None identified | [Passed](TEST_REPORT.md#f-780) |
| F-781 | [x] <a id="f-781"></a>New sign-in link cancels the previous login before creating another. | `antiquota/ui/app.js:2013` | None identified | [Passed](TEST_REPORT.md#f-781) |
| F-782 | [x] <a id="f-782"></a>Sign-in countdown highlights its last fifteen seconds and expiry. | `antiquota/ui/app.js:2013` | None identified | [Passed](TEST_REPORT.md#f-782) |
| F-783 | [x] <a id="f-783"></a>State polling runs every two seconds and accepts completion without pasting a code. | `antiquota/ui/app.js:2013` | None identified | [Passed](TEST_REPORT.md#f-783) |
| F-784 | [x] <a id="f-784"></a>Returning focus reads a valid code from the clipboard when the field is empty. | `antiquota/ui/app.js:2074` | None identified | [Passed](TEST_REPORT.md#f-784) |
| F-785 | [x] <a id="f-785"></a>Paste rejects values outside the 4/ code format with at least twenty permitted characters. | `antiquota/ui/app.js:2074` | None identified | [Passed](TEST_REPORT.md#f-785) |
| F-786 | [x] <a id="f-786"></a>Clipboard access failure explains manual paste. | `antiquota/ui/app.js:2074` | None identified | [Passed](TEST_REPORT.md#f-786) |
| F-787 | [x] <a id="f-787"></a>Submitting a code disables Add account while its check runs. | `antiquota/ui/app.js:2087` | None identified | [Passed](TEST_REPORT.md#f-787) |
| F-788 | [x] <a id="f-788"></a>Successful sign-in reports identity, refreshes accounts and closes after 1.2 seconds. | `antiquota/ui/app.js:2051` | None identified | [Passed](TEST_REPORT.md#f-788) |
| F-789 | [x] <a id="f-789"></a>Verification state disables code controls and shows verification actions. | `antiquota/ui/app.js:2051` | None identified | [Passed](TEST_REPORT.md#f-789) |
| F-790 | [x] <a id="f-790"></a>Failed sign-in disables expired controls and offers a new sign-in link. | `antiquota/ui/app.js:2051` | None identified | [Passed](TEST_REPORT.md#f-790) |
| F-791 | [x] <a id="f-791"></a>A code-submission API error restores Add account and displays the error. | `antiquota/ui/app.js:2087` | None identified | [Passed](TEST_REPORT.md#f-791) |
| F-792 | [x] <a id="f-792"></a>Closing or restarting sign-in cancels its pending login and clears timers. | `antiquota/ui/app.js:2007` | None identified | [Passed](TEST_REPORT.md#f-792) |
### Every slash-command entry point

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-793 | [x] <a id="f-793"></a>/help opens the shortcut and command sheet. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-793) |
| F-794 | [x] <a id="f-794"></a>/goal opens the dialog or starts a verified goal from its objective argument. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-794) |
| F-795 | [x] <a id="f-795"></a>/clear clears the conversation while preserving the transcript. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-795) |
| F-796 | [x] <a id="f-796"></a>/compact starts a fresh conversation with handoff summary. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-796) |
| F-797 | [x] <a id="f-797"></a>/rewind confirms file rewind before a specified turn. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-797) |
| F-798 | [x] <a id="f-798"></a>/export exports the transcript as Markdown. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-798) |
| F-799 | [x] <a id="f-799"></a>/init asks agy to create or improve concise project instructions. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-799) |
| F-800 | [x] <a id="f-800"></a>/bg starts a background command or explains the required argument. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-800) |
| F-801 | [x] <a id="f-801"></a>/loop sets an interval prompt or removes it with off/stop or an empty argument. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-801) |
| F-802 | [x] <a id="f-802"></a>/schedule sets a daily-time prompt or removes it with off/stop or an empty argument. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-802) |
| F-803 | [x] <a id="f-803"></a>/memory opens GEMINI.md, falls back to AGENTS.md or explains how to create instructions. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-803) |
| F-804 | [x] <a id="f-804"></a>/state opens the stored handoff state. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-804) |
| F-805 | [x] <a id="f-805"></a>/stop stops the turn and pauses its goal. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-805) |
| F-806 | [x] <a id="f-806"></a>/resume resumes a paused goal. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-806) |
| F-807 | [x] <a id="f-807"></a>/cleargoal removes the goal. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-807) |
| F-808 | [x] <a id="f-808"></a>/new opens the new-session dialog. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-808) |
| F-809 | [x] <a id="f-809"></a>/model patches the model ID. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-809) |
| F-810 | [x] <a id="f-810"></a>/effort sets a family effort or explains an unavailable effort. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-810) |
| F-811 | [x] <a id="f-811"></a>/mode patches the permission mode. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-811) |
| F-812 | [x] <a id="f-812"></a>/rename patches the session title. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-812) |
| F-813 | [x] <a id="f-813"></a>/quota probes all accounts. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-813) |
| F-814 | [x] <a id="f-814"></a>/usage opens Usage settings. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-814) |
| F-815 | [x] <a id="f-815"></a>/accounts opens Accounts settings. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-815) |
| F-816 | [x] <a id="f-816"></a>/login opens Add Google account. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-816) |
| F-817 | [x] <a id="f-817"></a>/settings opens Settings. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-817) |
| F-818 | [x] <a id="f-818"></a>/stack is a legacy stack-drawer entry point requiring repair because its renderer is absent. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-818) |
| F-819 | [x] <a id="f-819"></a>/permissions opens Permissions settings. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-819) |
| F-820 | [x] <a id="f-820"></a>/delete confirms and deletes the selected session. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-820) |
| F-821 | [x] <a id="f-821"></a>/skills opens Skills despite being absent from legacy command suggestions. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-821) |
| F-822 | [x] <a id="f-822"></a>/hooks opens Hooks despite being absent from legacy command suggestions. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-822) |
| F-823 | [x] <a id="f-823"></a>/mcp opens MCP servers despite being absent from legacy command suggestions. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-823) |
### Autocomplete and shortcuts

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-824 | [x] <a id="f-824"></a>A slash prefix filters suggestions without case sensitivity. | `antiquota/ui/app.js:1197` | None identified | [Passed](TEST_REPORT.md#f-824) |
| F-825 | [x] <a id="f-825"></a>An @ prefix before the caret searches files and inserts the selected path. | `antiquota/ui/app.js:1197` | None identified | [Passed](TEST_REPORT.md#f-825) |
| F-826 | [x] <a id="f-826"></a>Stale mention results cannot replace a newer query. | `antiquota/ui/app.js:1197` | None identified | [Passed](TEST_REPORT.md#f-826) |
| F-827 | [x] <a id="f-827"></a>ArrowUp and ArrowDown cycle suggestion items. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-827) |
| F-828 | [x] <a id="f-828"></a>Tab or Enter applies the selected suggestion before sending. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-828) |
| F-829 | [x] <a id="f-829"></a>Escape dismisses suggestions. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-829) |
| F-830 | [x] <a id="f-830"></a>Composer blur dismisses suggestions after 100 ms. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-830) |
| F-831 | [x] <a id="f-831"></a>Enter sends except during IME composition. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-831) |
| F-832 | [x] <a id="f-832"></a>Shift+Enter inserts a newline. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-832) |
| F-833 | [x] <a id="f-833"></a>ArrowUp recalls earlier prompts when empty or at the start without Shift. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-833) |
| F-834 | [x] <a id="f-834"></a>ArrowDown recalls newer prompts and the saved draft. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-834) |
| F-835 | [x] <a id="f-835"></a>Escape stops the selected working session when no suggestion menu is open. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-835) |
| F-836 | [x] <a id="f-836"></a>Escape closes the context menu. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-836) |
| F-837 | [x] <a id="f-837"></a>ArrowUp and ArrowDown wrap focus among context-menu items. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-837) |
| F-838 | [x] <a id="f-838"></a>Cmd/Ctrl+N opens New session. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-838) |
| F-839 | [x] <a id="f-839"></a>Cmd/Ctrl+. stops the turn. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-839) |
| F-840 | [x] <a id="f-840"></a>Cmd/Ctrl+, opens Settings. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-840) |
| F-841 | [x] <a id="f-841"></a>Cmd/Ctrl+/ opens the shortcut sheet. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-841) |
| F-842 | [x] <a id="f-842"></a>Cmd/Ctrl+Shift+M toggles dictation. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-842) |
| F-843 | [ ] <a id="f-843"></a>Desktop Cmd/Ctrl+= or Cmd/Ctrl++ increases zoom ten percentage points. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-843) |
| F-844 | [ ] <a id="f-844"></a>Desktop Cmd/Ctrl+- or Cmd/Ctrl+_ decreases zoom ten percentage points. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-844) |
| F-845 | [ ] <a id="f-845"></a>Desktop Cmd/Ctrl+0 resets zoom. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-845) |
| F-846 | [x] <a id="f-846"></a>Cmd/Ctrl+K focuses and selects search. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-846) |
| F-847 | [x] <a id="f-847"></a>Cmd/Ctrl+J focuses the composer. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-847) |
| F-848 | [x] <a id="f-848"></a>Cmd/Ctrl+B toggles the side panel without Shift. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-848) |
| F-849 | [x] <a id="f-849"></a>Cmd/Ctrl+Backslash toggles split view outside embedded sessions. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-849) |
| F-850 | [x] <a id="f-850"></a>Cmd/Ctrl+Shift+[ or { selects the previous visible session. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-850) |
| F-851 | [x] <a id="f-851"></a>Cmd/Ctrl+Shift+] or } selects the next visible session. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-851) |
| F-852 | [x] <a id="f-852"></a>Escape closes Settings when no modal dialog is open. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-852) |
| F-853 | [x] <a id="f-853"></a>Alt-clicking a session opens it on the right. | `antiquota/ui/app.js:224` | None identified | [Passed](TEST_REPORT.md#f-853) |
| F-854 | [x] <a id="f-854"></a>Non-Mac reference labels use Ctrl and Shift rather than Cmd glyphs. | `antiquota/ui/app.js:1527` | None identified | [Passed](TEST_REPORT.md#f-854) |
### Layout, themes and accessibility

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-855 | [x] <a id="f-855"></a>The default dark theme uses the dense warm palette and centralized CSS variables. | `antiquota/ui/app.css:5` | None identified | [Passed](TEST_REPORT.md#f-855) |
| F-856 | [x] <a id="f-856"></a>The light palette follows prefers-color-scheme and overrides centralized color variables. | `antiquota/ui/app.css:36` | None identified | [Passed](TEST_REPORT.md#f-856) |
| F-857 | [x] <a id="f-857"></a>The composer stays docked below an independently scrolling transcript. | `antiquota/ui/app.css:194` | None identified | [Passed](TEST_REPORT.md#f-857) |
| F-858 | [x] <a id="f-858"></a>Below 860 pixels the sidebar becomes a slide-in overlay with Open and Close controls. | `antiquota/ui/app.css:544` | None identified | [Passed](TEST_REPORT.md#f-858) |
| F-859 | [x] <a id="f-859"></a>Below 860 pixels the side panel becomes an overlay. | `antiquota/ui/app.css:544` | None identified | [Passed](TEST_REPORT.md#f-859) |
| F-860 | [x] <a id="f-860"></a>Narrow headers omit working-folder and token totals. | `antiquota/ui/app.css:558` | None identified | [Passed](TEST_REPORT.md#f-860) |
| F-861 | [x] <a id="f-861"></a>Below 860 pixels Settings uses wrapped navigation above the body. | `antiquota/ui/app.css:627` | None identified | [Passed](TEST_REPORT.md#f-861) |
| F-862 | [x] <a id="f-862"></a>Below 680 pixels the shortcut reference uses one column. | `antiquota/ui/app.css:942` | None identified | [Passed](TEST_REPORT.md#f-862) |
| F-863 | [x] <a id="f-863"></a>Below 720 pixels usage statistics use two columns. | `antiquota/ui/app.css:993` | None identified | [Passed](TEST_REPORT.md#f-863) |
| F-864 | [x] <a id="f-864"></a>Token meter widths below 620 pixels hide session totals and context label. | `antiquota/ui/app.css:659` | None identified | [Passed](TEST_REPORT.md#f-864) |
| F-865 | [x] <a id="f-865"></a>Token meter widths below 440 pixels hide available-account count. | `antiquota/ui/app.css:660` | None identified | [Passed](TEST_REPORT.md#f-865) |
| F-866 | [x] <a id="f-866"></a>Reduced-motion preference disables animation and transitions. | `antiquota/ui/app.css:564` | None identified | [Passed](TEST_REPORT.md#f-866) |
| F-867 | [ ] <a id="f-867"></a>The macOS sidebar and narrow header reserve space for traffic lights independently of native zoom. | `antiquota/ui/app.css:798` | None identified | [Pending](TEST_REPORT.md#f-867) |
| F-868 | [x] <a id="f-868"></a>Split view keeps two content columns below the sidebar-collapse breakpoint. | `antiquota/ui/app.css:1034` | None identified | [Passed](TEST_REPORT.md#f-868) |
| F-869 | [x] <a id="f-869"></a>Terminal occupies its pane and gives the drawer a wider terminal layout. | `antiquota/ui/app.css:1019` | None identified | [Passed](TEST_REPORT.md#f-869) |
| F-870 | [x] <a id="f-870"></a>Keyboard focus receives a visible accent outline. | `antiquota/ui/app.css:82` | None identified | [Passed](TEST_REPORT.md#f-870) |
| F-871 | [x] <a id="f-871"></a>Hidden controls do not occupy layout space. | `antiquota/ui/app.css:83` | None identified | [Passed](TEST_REPORT.md#f-871) |
### Native scale behavior

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-872 | [x] <a id="f-872"></a>Desktop scale uses native webview zoom rather than CSS zoom. | `antiquota/ui/app.js:81` | tests/ui.test.cjs:30 | [Passed](TEST_REPORT.md#f-872) |
| F-873 | [x] <a id="f-873"></a>Desktop scale persists per device and restores when the root webview opens. | `antiquota/ui/app.js:81` | tests/ui.test.cjs:30 | [Passed](TEST_REPORT.md#f-873) |
### Embedded scale behavior

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-874 | [ ] <a id="f-874"></a>Embedded scale controls delegate to the root without applying another scale. | `antiquota/ui/app.js:81` | tests/ui.test.cjs:47 | [Pending](TEST_REPORT.md#f-874) |
### Browser scale behavior

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-875 | [x] <a id="f-875"></a>Browser scale stays under the browser's own zoom controls. | `antiquota/ui/app.js:81` | tests/ui.test.cjs:57 | [Passed](TEST_REPORT.md#f-875) |
### Scale validation

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-876 | [x] <a id="f-876"></a>Invalid scale resets to one and extreme scale clamps to 70–160 percent. | `antiquota/ui/app.js:81` | tests/ui.test.cjs:65 | [Passed](TEST_REPORT.md#f-876) |
### Every native application menu action

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-877 | [x] <a id="f-877"></a>Antiquota > About Antiquota shows native application metadata. | `app/src-tauri/src/menu.rs:11` | None identified | [Passed](TEST_REPORT.md#f-877) |
| F-878 | [x] <a id="f-878"></a>Antiquota > Settings… opens General; Cmd/Ctrl+Comma. | `app/src-tauri/src/menu.rs:11` | None identified | [Passed](TEST_REPORT.md#f-878) |
| F-879 | [x] <a id="f-879"></a>Antiquota > Accounts and Usage opens Usage. | `app/src-tauri/src/menu.rs:11` | None identified | [Passed](TEST_REPORT.md#f-879) |
| F-880 | [x] <a id="f-880"></a>Antiquota > Services exposes platform services. | `app/src-tauri/src/menu.rs:11` | None identified | [Passed](TEST_REPORT.md#f-880) |
| F-881 | [ ] <a id="f-881"></a>Antiquota > Hide hides the application. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-881) |
| F-882 | [ ] <a id="f-882"></a>Antiquota > Hide Others hides other applications. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-882) |
| F-883 | [ ] <a id="f-883"></a>Antiquota > Show All restores hidden applications. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-883) |
| F-884 | [ ] <a id="f-884"></a>Antiquota > Quit Antiquota uses work-aware quit; Cmd/Ctrl+Q. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-884) |
| F-885 | [x] <a id="f-885"></a>File > New Session opens session creation; Cmd/Ctrl+N. | `app/src-tauri/src/menu.rs:11` | None identified | [Passed](TEST_REPORT.md#f-885) |
| F-886 | [ ] <a id="f-886"></a>File > Export Transcript… exports the transcript; Cmd/Ctrl+Shift+E. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-886) |
| F-887 | [ ] <a id="f-887"></a>File > Close Window hides the main window into the tray. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-887) |
| F-888 | [ ] <a id="f-888"></a>Edit > Undo uses native undo. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-888) |
| F-889 | [ ] <a id="f-889"></a>Edit > Redo uses native redo. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-889) |
| F-890 | [ ] <a id="f-890"></a>Edit > Cut cuts focused content to the clipboard. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-890) |
| F-891 | [ ] <a id="f-891"></a>Edit > Copy copies focused content to the clipboard. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-891) |
| F-892 | [ ] <a id="f-892"></a>Edit > Paste pastes clipboard content. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-892) |
| F-893 | [ ] <a id="f-893"></a>Edit > Select All selects focused editable content. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-893) |
| F-894 | [x] <a id="f-894"></a>Edit > Find Session or Message focuses and selects search; Cmd/Ctrl+K. | `app/src-tauri/src/menu.rs:11` | None identified | [Passed](TEST_REPORT.md#f-894) |
| F-895 | [ ] <a id="f-895"></a>Edit > Dictate toggles dictation; Cmd/Ctrl+Shift+M. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-895) |
| F-896 | [x] <a id="f-896"></a>View > Zoom In increases native zoom; Cmd/Ctrl+Equal. | `app/src-tauri/src/menu.rs:11` | None identified | [Passed](TEST_REPORT.md#f-896) |
| F-897 | [x] <a id="f-897"></a>View > Zoom Out decreases native zoom; Cmd/Ctrl+Minus. | `app/src-tauri/src/menu.rs:11` | None identified | [Passed](TEST_REPORT.md#f-897) |
| F-898 | [x] <a id="f-898"></a>View > Actual Size resets native zoom; Cmd/Ctrl+0. | `app/src-tauri/src/menu.rs:11` | None identified | [Passed](TEST_REPORT.md#f-898) |
| F-899 | [ ] <a id="f-899"></a>View > Side Panel toggles the drawer; Cmd/Ctrl+B. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-899) |
| F-900 | [x] <a id="f-900"></a>View > Side by Side toggles split; Cmd/Ctrl+Backslash. | `app/src-tauri/src/menu.rs:11` | None identified | [Passed](TEST_REPORT.md#f-900) |
| F-901 | [ ] <a id="f-901"></a>View > Expand All Steps toggles transcript expansion; Cmd/Ctrl+Shift+O. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-901) |
| F-902 | [ ] <a id="f-902"></a>View > Reload reloads the page; Cmd/Ctrl+R. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-902) |
| F-903 | [ ] <a id="f-903"></a>View > Fullscreen uses the platform fullscreen action. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-903) |
| F-904 | [ ] <a id="f-904"></a>Window > Minimize uses the platform minimize action. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-904) |
| F-905 | [ ] <a id="f-905"></a>Window > Maximize uses the platform maximize action. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-905) |
| F-906 | [ ] <a id="f-906"></a>Window > Previous Session opens the previous session; Cmd/Ctrl+Shift+BracketLeft. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-906) |
| F-907 | [ ] <a id="f-907"></a>Window > Next Session opens the next session; Cmd/Ctrl+Shift+BracketRight. | `app/src-tauri/src/menu.rs:11` | None identified | [Pending](TEST_REPORT.md#f-907) |
| F-908 | [x] <a id="f-908"></a>Help > Keyboard Shortcuts opens shortcuts; Cmd/Ctrl+Slash. | `app/src-tauri/src/menu.rs:11` | None identified | [Passed](TEST_REPORT.md#f-908) |
| F-909 | [x] <a id="f-909"></a>Help > Engine Log opens the engine log viewer. | `app/src-tauri/src/menu.rs:11` | None identified | [Passed](TEST_REPORT.md#f-909) |
### Desktop lifecycle and window state

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-910 | [ ] <a id="f-910"></a>Showing the main window unminimizes, reveals and focuses it. | `app/src-tauri/src/lib.rs:24` | None identified | [Pending](TEST_REPORT.md#f-910) |
| F-911 | [ ] <a id="f-911"></a>A second instance focuses the existing main window rather than opening another engine. | `app/src-tauri/src/lib.rs:137` | None identified | [Pending](TEST_REPORT.md#f-911) |
| F-912 | [ ] <a id="f-912"></a>The shell persists window size, position and maximized state without visibility. | `app/src-tauri/src/lib.rs:140` | None identified | [Pending](TEST_REPORT.md#f-912) |
| F-913 | [ ] <a id="f-913"></a>The default or unavailable-monitor window position is centered. | `app/src-tauri/src/lib.rs:163` | None identified | [Pending](TEST_REPORT.md#f-913) |
| F-914 | [ ] <a id="f-914"></a>Default window dimensions are 1280×820 with a 720×480 minimum. | `app/src-tauri/src/lib.rs:161` | None identified | [Pending](TEST_REPORT.md#f-914) |
| F-915 | [ ] <a id="f-915"></a>--hidden starts without showing the main window. | `app/src-tauri/src/lib.rs:167` | None identified | [Pending](TEST_REPORT.md#f-915) |
| F-916 | [ ] <a id="f-916"></a>Autostart uses a macOS LaunchAgent with --hidden. | `app/src-tauri/src/lib.rs:148` | None identified | [Pending](TEST_REPORT.md#f-916) |
| F-917 | [ ] <a id="f-917"></a>The macOS title bar overlays content and traffic lights use position 18,20. | `app/src-tauri/src/lib.rs:171` | None identified | [Pending](TEST_REPORT.md#f-917) |
| F-918 | [ ] <a id="f-918"></a>HTML drag-and-drop reaches the UI for file attachments. | `app/src-tauri/src/lib.rs:166` | None identified | [Pending](TEST_REPORT.md#f-918) |
| F-919 | [ ] <a id="f-919"></a>Closing the main window hides it into the tray while the engine keeps running. | `app/src-tauri/src/lib.rs:182` | None identified | [Pending](TEST_REPORT.md#f-919) |
| F-920 | [ ] <a id="f-920"></a>Reopening from the macOS dock reveals the main window. | `app/src-tauri/src/lib.rs:197` | None identified | [Pending](TEST_REPORT.md#f-920) |
| F-921 | [ ] <a id="f-921"></a>Boot discovers or starts the engine and navigates the main window to it. | `app/src-tauri/src/lib.rs:37` | None identified | [Pending](TEST_REPORT.md#f-921) |
| F-922 | [ ] <a id="f-922"></a>Concurrent boot requests are suppressed. | `app/src-tauri/src/lib.rs:37` | None identified | [Pending](TEST_REPORT.md#f-922) |
| F-923 | [ ] <a id="f-923"></a>Splash retry starts another boot in a worker thread. | `app/src-tauri/src/lib.rs:62` | None identified | [Pending](TEST_REPORT.md#f-923) |
| F-924 | [ ] <a id="f-924"></a>The app-info command returns version and operating system. | `app/src-tauri/src/lib.rs:57` | None identified | [Pending](TEST_REPORT.md#f-924) |
| F-925 | [ ] <a id="f-925"></a>Native save writes contents at the user-selected path or returns cancellation. | `app/src-tauri/src/lib.rs:68` | None identified | [Pending](TEST_REPORT.md#f-925) |
| F-926 | [x] <a id="f-926"></a>Quitting does not shut down an engine started outside the app. | `app/src-tauri/src/lib.rs:83` | None identified | [Passed](TEST_REPORT.md#f-926) |
| F-927 | [ ] <a id="f-927"></a>Quitting an app-owned idle engine requests shutdown. | `app/src-tauri/src/lib.rs:83` | None identified | [Pending](TEST_REPORT.md#f-927) |
| F-928 | [ ] <a id="f-928"></a>Quitting with active sessions or jobs presents the work-aware choice. | `app/src-tauri/src/lib.rs:83` | None identified | [Pending](TEST_REPORT.md#f-928) |
| F-929 | [ ] <a id="f-929"></a>Keep running in background quits the shell and retains the engine. | `app/src-tauri/src/lib.rs:83` | None identified | [Pending](TEST_REPORT.md#f-929) |
| F-930 | [ ] <a id="f-930"></a>Stop and quit stops turns while background jobs continue. | `app/src-tauri/src/lib.rs:83` | None identified | [Pending](TEST_REPORT.md#f-930) |
### Engine discovery and process startup

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-931 | [ ] <a id="f-931"></a>Discovery honors ANTIQUOTA_DIR or uses ~/.antiquota. | `app/src-tauri/src/engine.rs:28` | None identified | [Pending](TEST_REPORT.md#f-931) |
| F-932 | [x] <a id="f-932"></a>Discovery reads state/engine.json and GUI token to attach to the shared engine. | `app/src-tauri/src/engine.rs:64` | None identified | [Passed](TEST_REPORT.md#f-932) |
| F-933 | [x] <a id="f-933"></a>Discovery accepts only a health response identifying antiquota. | `app/src-tauri/src/engine.rs:51` | None identified | [Passed](TEST_REPORT.md#f-933) |
| F-934 | [ ] <a id="f-934"></a>Ownership in engine.json controls shutdown behavior across later app starts. | `app/src-tauri/src/engine.rs:64` | None identified | [Pending](TEST_REPORT.md#f-934) |
| F-935 | [ ] <a id="f-935"></a>Native engine requests have a five-second global timeout. | `app/src-tauri/src/engine.rs:39` | None identified | [Pending](TEST_REPORT.md#f-935) |
| F-936 | [x] <a id="f-936"></a>ANTIQUOTA_ENGINE takes precedence over bundled and source engines. | `app/src-tauri/src/engine.rs:98` | None identified | [Passed](TEST_REPORT.md#f-936) |
| F-937 | [ ] <a id="f-937"></a>A release discovers its platform engine in bundled resources. | `app/src-tauri/src/engine.rs:98` | None identified | [Pending](TEST_REPORT.md#f-937) |
| F-938 | [ ] <a id="f-938"></a>Legacy source discovery honors ANTIQUOTA_DEV_ROOT or repository location. | `app/src-tauri/src/engine.rs:98` | None identified | [Pending](TEST_REPORT.md#f-938) |
| F-939 | [x] <a id="f-939"></a>Legacy discovery searches for Python. Authorized replacement: discover the single Rust antiquota sidecar or an explicit Rust-engine executable while retaining shared engine.json discovery and actionable missing-engine errors; no Python interpreter is required. | `app/src-tauri/src/engine.rs:128` | None identified | [Passed](TEST_REPORT.md#f-939) |
| F-940 | [x] <a id="f-940"></a>Spawned engine output appends to private state/engine.log. | `app/src-tauri/src/engine.rs:150` | None identified | [Passed](TEST_REPORT.md#f-940) |
| F-941 | [x] <a id="f-941"></a>App engine launch supplies serve --port 8765 and app ownership/path markers. | `app/src-tauri/src/engine.rs:150` | None identified | [Passed](TEST_REPORT.md#f-941) |
| F-942 | [x] <a id="f-942"></a>POSIX engines use an independent process group and survive app exit. | `app/src-tauri/src/engine.rs:174` | None identified | [Passed](TEST_REPORT.md#f-942) |
| F-943 | [ ] <a id="f-943"></a>Windows engines use a new process group without a console window. | `app/src-tauri/src/engine.rs:180` | None identified | [Pending](TEST_REPORT.md#f-943) |
| F-944 | [ ] <a id="f-944"></a>Startup polls engine discovery every 250 ms up to thirty seconds. | `app/src-tauri/src/engine.rs:188` | None identified | [Pending](TEST_REPORT.md#f-944) |
| F-945 | [ ] <a id="f-945"></a>Startup failure reports timeout with up to twelve engine-log tail lines. | `app/src-tauri/src/engine.rs:188` | None identified | [Pending](TEST_REPORT.md#f-945) |
### Every tray item and tray state

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-946 | [ ] <a id="f-946"></a>The tray uses a template icon and opens its menu on left click. | `app/src-tauri/src/tray.rs:13` | None identified | [Pending](TEST_REPORT.md#f-946) |
| F-947 | [ ] <a id="f-947"></a>The disabled tray heading shows starting, idle, active sessions and jobs. | `app/src-tauri/src/tray.rs:107` | None identified | [Pending](TEST_REPORT.md#f-947) |
| F-948 | [ ] <a id="f-948"></a>Up to six working sessions appear with truncated title and status. | `app/src-tauri/src/tray.rs:107` | None identified | [Pending](TEST_REPORT.md#f-948) |
| F-949 | [ ] <a id="f-949"></a>Selecting a tray session reveals the main window and opens that session. | `app/src-tauri/src/tray.rs:26` | None identified | [Pending](TEST_REPORT.md#f-949) |
| F-950 | [ ] <a id="f-950"></a>Up to ten accounts appear as disabled quota/status rows. | `app/src-tauri/src/tray.rs:107` | None identified | [Pending](TEST_REPORT.md#f-950) |
| F-951 | [x] <a id="f-951"></a>Account rows distinguish Gemini quota, wall, cooldown, signed-out and unprobed state. | `app/src-tauri/src/tray.rs:80` | None identified | [Passed](TEST_REPORT.md#f-951) |
| F-952 | [x] <a id="f-952"></a>Rewrite correction: tray verification-needed state accepts the actual account API verification URL string, as well as compatible legacy object/boolean representations. The URL-string mismatch is confirmed by source, not claimed as observed during the Gemini run. | `app/src-tauri/src/tray.rs:80` | None identified | [Passed](TEST_REPORT.md#f-952) |
| F-953 | [ ] <a id="f-953"></a>Tray Open antiquota reveals the main window. | `app/src-tauri/src/tray.rs:147` | None identified | [Pending](TEST_REPORT.md#f-953) |
| F-954 | [ ] <a id="f-954"></a>Tray New session… reveals the window and opens session creation. | `app/src-tauri/src/tray.rs:148` | None identified | [Pending](TEST_REPORT.md#f-954) |
| F-955 | [ ] <a id="f-955"></a>Tray Stop all running is enabled only while sessions work and stops every active session. | `app/src-tauri/src/tray.rs:149` | None identified | [Pending](TEST_REPORT.md#f-955) |
| F-956 | [ ] <a id="f-956"></a>Tray Quit antiquota uses work-aware quit and Cmd/Ctrl+Q. | `app/src-tauri/src/tray.rs:151` | None identified | [Pending](TEST_REPORT.md#f-956) |
| F-957 | [ ] <a id="f-957"></a>Tray state refreshes every five seconds. | `app/src-tauri/src/tray.rs:156` | None identified | [Pending](TEST_REPORT.md#f-957) |
| F-958 | [ ] <a id="f-958"></a>Three consecutive state-request failures trigger engine discovery or respawn. | `app/src-tauri/src/tray.rs:156` | None identified | [Pending](TEST_REPORT.md#f-958) |
| F-959 | [ ] <a id="f-959"></a>On macOS, tray title, tooltip and dock/window badge indicate active-session count. | `app/src-tauri/src/tray.rs:156` | None identified | [Pending](TEST_REPORT.md#f-959) |
### Splash and native permissions

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-960 | [x] <a id="f-960"></a>The splash displays startup progress from engine-status events. | `app/splash/index.html:38` | None identified | [Passed](TEST_REPORT.md#f-960) |
| F-961 | [x] <a id="f-961"></a>The splash displays error text with Try again and Quit only in error state. | `app/splash/index.html:51` | None identified | [Passed](TEST_REPORT.md#f-961) |
| F-962 | [x] <a id="f-962"></a>Try again clears the error state and invokes retry_engine. | `app/splash/index.html:54` | None identified | [Passed](TEST_REPORT.md#f-962) |
| F-963 | [ ] <a id="f-963"></a>Splash Quit invokes work-aware quit_app. | `app/splash/index.html:59` | None identified | [Pending](TEST_REPORT.md#f-963) |
| F-964 | [x] <a id="f-964"></a>The splash respects reduced-motion preference. | `app/splash/index.html:34` | None identified | [Passed](TEST_REPORT.md#f-964) |
### Native permissions scope

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-965 | [ ] <a id="f-965"></a>The named main window grants engine-page APIs to localhost and 127.0.0.1 origins. | `app/src-tauri/capabilities/engine-ui.json:6` | None identified | [Pending](TEST_REPORT.md#f-965) |
| F-966 | [ ] <a id="f-966"></a>Engine-page permissions expose zoom, dragging, attention, badges, clipboard, notification, autostart and save actions. | `app/src-tauri/capabilities/engine-ui.json:11` | None identified | [Pending](TEST_REPORT.md#f-966) |
| F-967 | [ ] <a id="f-967"></a>Engine-page openPath capability is limited to the user's home directory. | `app/src-tauri/capabilities/engine-ui.json:21` | None identified | [Pending](TEST_REPORT.md#f-967) |
### Legacy packaging and release outputs

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-968 | [ ] <a id="f-968"></a>Legacy engine resources use an engine/antiquotad tree. Authorized replacement: bundle the single Rust antiquota sidecar through the platform Tauri configuration and discover it while preserving shared engine.json sessions; retain no PyInstaller dependency. | `app/src-tauri/tauri.bundle.json:3` | None identified | [Pending](TEST_REPORT.md#f-968) |
### Legacy bundled command entry point

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-969 | [x] <a id="f-969"></a>The bundled engine forwards serve and current CLI commands to the engine entry point. | `app/engine/antiquotad.py:4` | None identified | [Passed](TEST_REPORT.md#f-969) |
### Platform release packages

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-970 | [x] <a id="f-970"></a>macOS release builds an app and compressed DMG with an Applications shortcut. | `app/scripts/build-app.sh:7` | None identified | [Passed](TEST_REPORT.md#f-970) |
| F-971 | [x] <a id="f-971"></a>Linux release builds deb, rpm and AppImage outputs. | `app/scripts/build-app.sh:21` | None identified | [Passed](TEST_REPORT.md#f-971) |
| F-972 | [ ] <a id="f-972"></a>Other release platforms use the configured Tauri bundle targets. | `app/scripts/build-app.sh:24` | None identified | [Pending](TEST_REPORT.md#f-972) |
### Legacy CI matrix

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-973 | [ ] <a id="f-973"></a>CI independently builds desktop artifacts on macOS, Windows and Linux. | `.github/workflows/app.yml:12` | None identified | [Pending](TEST_REPORT.md#f-973) |
| F-974 | [ ] <a id="f-974"></a>Authorized CI replacement: run cargo tests and clippy, pnpm check, Vitest, Playwright, stale-type checks and the Rust-sidecar app build on macOS, Windows and Linux; legacy Python test coverage remains reference evidence only. | `.github/workflows/app.yml:33` | None identified | [Pending](TEST_REPORT.md#f-974) |
| F-975 | [ ] <a id="f-975"></a>Authorized Windows replacement: bundle the Rust sidecar with portable-pty ConPTY support and verify paths, terminal operation and installer packaging; no pywinpty runtime dependency is required. | `.github/workflows/app.yml:42` | None identified | [Pending](TEST_REPORT.md#f-975) |
| F-976 | [x] <a id="f-976"></a>Authorized POSIX replacement: bundle the self-contained Rust antiquota sidecar, verify its shared-engine discovery and build native installers without Python or PyInstaller. | `.github/workflows/app.yml:45` | None identified | [Passed](TEST_REPORT.md#f-976) |
| F-977 | [ ] <a id="f-977"></a>CI uploads platform app bundles and installers. | `.github/workflows/app.yml:51` | None identified | [Pending](TEST_REPORT.md#f-977) |
### Dialog defaults and remaining interaction edges

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-978 | [x] <a id="f-978"></a>New-session defaults use the current model/mode and project folder, or configured defaults without a session. | `antiquota/ui/app.js:1948` | None identified | [Passed](TEST_REPORT.md#f-978) |
| F-979 | [x] <a id="f-979"></a>New sessions from a worktree default to the parent repository folder. | `antiquota/ui/app.js:1948` | None identified | [Passed](TEST_REPORT.md#f-979) |
| F-980 | [x] <a id="f-980"></a>The worktree checkbox follows the configured default unless an explicit project-menu choice overrides it. | `antiquota/ui/app.js:1948` | None identified | [Passed](TEST_REPORT.md#f-980) |
| F-981 | [x] <a id="f-981"></a>Working-folder browse shows up to four recent unique session folders before a path is typed. | `antiquota/ui/app.js:1934` | None identified | [Passed](TEST_REPORT.md#f-981) |
| F-982 | [x] <a id="f-982"></a>Folder browser entries navigate into a child folder or up to its parent. | `antiquota/ui/app.js:1934` | None identified | [Passed](TEST_REPORT.md#f-982) |
| F-983 | [x] <a id="f-983"></a>Selecting a browser folder updates the working-folder input. | `antiquota/ui/app.js:1934` | None identified | [Passed](TEST_REPORT.md#f-983) |
| F-984 | [x] <a id="f-984"></a>Typing a working folder debounces filesystem browsing by 200 ms. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-984) |
| F-985 | [x] <a id="f-985"></a>Create session posts folder, concrete effort/model, mode and worktree choice. | `antiquota/ui/app.js:1959` | None identified | [Passed](TEST_REPORT.md#f-985) |
| F-986 | [x] <a id="f-986"></a>Successful creation inserts the session into navigation and opens it. | `antiquota/ui/app.js:1959` | None identified | [Passed](TEST_REPORT.md#f-986) |
| F-987 | [x] <a id="f-987"></a>A creation error shows the API error without creating a fake local session. | `antiquota/ui/app.js:1959` | None identified | [Passed](TEST_REPORT.md#f-987) |
| F-988 | [x] <a id="f-988"></a>Canceling New session closes its dialog without sending a create request. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-988) |
| F-989 | [x] <a id="f-989"></a>Canceling Goal closes its dialog without sending a goal request. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-989) |
| F-990 | [x] <a id="f-990"></a>Goal verifier defaults to enabled unless the existing goal says otherwise. | `antiquota/ui/app.js:1970` | None identified | [Passed](TEST_REPORT.md#f-990) |
| F-991 | [x] <a id="f-991"></a>Goal rounds default to twenty unless the existing goal has a limit. | `antiquota/ui/app.js:1970` | None identified | [Passed](TEST_REPORT.md#f-991) |
| F-992 | [x] <a id="f-992"></a>Goal objective and working folder use required input validation before accepting their forms. | `antiquota/ui/index.html:148` | None identified | [Passed](TEST_REPORT.md#f-992) |
| F-993 | [x] <a id="f-993"></a>The sign-in code submits on Enter and automatically after paste. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-993) |
| F-994 | [x] <a id="f-994"></a>A raw text paste without files remains ordinary composer input. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-994) |
| F-995 | [x] <a id="f-995"></a>Live turn completion triggers turn notification except for quota endings. | `antiquota/ui/app.js:871` | None identified | [Passed](TEST_REPORT.md#f-995) |
| F-996 | [x] <a id="f-996"></a>Live job completion triggers the job-finished notification. | `antiquota/ui/app.js:871` | None identified | [Passed](TEST_REPORT.md#f-996) |
| F-997 | [x] <a id="f-997"></a>A live met-goal notice triggers the goal-met notification. | `antiquota/ui/app.js:871` | None identified | [Passed](TEST_REPORT.md#f-997) |
| F-998 | [ ] <a id="f-998"></a>A live exhausted-quota notice triggers the quota notification. | `antiquota/ui/app.js:871` | None identified | [Pending](TEST_REPORT.md#f-998) |
| F-999 | [ ] <a id="f-999"></a>Desktop header and brand regions permit native window dragging. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-999) |
| F-1000 | [ ] <a id="f-1000"></a>Desktop external-link handling preserves in-app file links and downloadable files. | `antiquota/ui/app.js:2526` | None identified | [Pending](TEST_REPORT.md#f-1000) |
| F-1001 | [x] <a id="f-1001"></a>Session relative-time labels refresh every minute. | `antiquota/ui/app.js:2526` | None identified | [Passed](TEST_REPORT.md#f-1001) |
| F-1002 | [x] <a id="f-1002"></a>An unknown slash command shows its unknown-command message. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-1002) |
| F-1003 | [x] <a id="f-1003"></a>Commands requiring a session report Open a session first when none is open. | `antiquota/ui/app.js:1430` | None identified | [Passed](TEST_REPORT.md#f-1003) |
### Requested rewrite scale acceptance

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-1004 | [x] <a id="f-1004"></a>Both split panes remain fully visible and equally scaled at 80, 100, 125 and 150 percent native zoom. | `antiquota/ui/app.js:81` | None identified | [Passed](TEST_REPORT.md#f-1004) |
| F-1005 | [ ] <a id="f-1005"></a>Context menus and usage tooltips track their intended position at every required zoom. | `antiquota/ui/app.js:81` | None identified | [Pending](TEST_REPORT.md#f-1005) |
| F-1006 | [ ] <a id="f-1006"></a>Terminal fitting remains correct at every required zoom. | `antiquota/ui/app.js:81` | None identified | [Pending](TEST_REPORT.md#f-1006) |
| F-1007 | [x] <a id="f-1007"></a>Session sidebar content stays clear of macOS traffic lights at every required zoom. | `antiquota/ui/app.js:81` | None identified | [Passed](TEST_REPORT.md#f-1007) |
### Requested rewrite desktop persistence acceptance

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-1008 | [ ] <a id="f-1008"></a>Window size, position and maximized state survive a real quit and relaunch. | `app/src-tauri/src/lib.rs:137` | None identified | [Pending](TEST_REPORT.md#f-1008) |
| F-1009 | [ ] <a id="f-1009"></a>A saved position on a disconnected monitor falls back to a visible centered window. | `app/src-tauri/src/lib.rs:163` | None identified | [Pending](TEST_REPORT.md#f-1009) |

## Explicit rewrite requirements

The following acceptance items come from the authorized rewrite task, not from an observation of legacy code. They supplement the baseline rather than replacing its feature coverage.

### Domain boundaries and ports

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-1010 | [x] <a id="f-1010"></a>Build the Rust engine as a Cargo workspace with core, features, app, adapters, server and cli crates. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1010) |
| F-1011 | [x] <a id="f-1011"></a>Keep crates/core independent of tokio, filesystem access and process spawning; domain tests run without operating-system adapters. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1011) |
| F-1012 | [x] <a id="f-1012"></a>Define Session, Turn, Event, Command, Account, Slot, Quota, Goal, Approval, Rule and Settings as serializable domain types. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1012) |
| F-1013 | [x] <a id="f-1013"></a>Use one Event enum for all session event variants; UI, CLI and approval consumers decode that same typed event stream. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1013) |
| F-1014 | [ ] <a id="f-1014"></a>Keep state machines, account routing, rotation and permission policy in the pure domain and test their transitions independently. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1014) |
| F-1015 | [x] <a id="f-1015"></a>Expose AgentRunner as a core trait that can be replaced with the scripted test runner. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1015) |
| F-1016 | [x] <a id="f-1016"></a>Expose AccountStore as a core trait that can be replaced without reading real account files. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1016) |
| F-1017 | [x] <a id="f-1017"></a>Expose QuotaProbe as a core trait that can return deterministic account quota and failure responses. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1017) |
| F-1018 | [x] <a id="f-1018"></a>Expose SessionStore as a core trait that can save and replay sessions through an in-memory test implementation. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1018) |
| F-1019 | [x] <a id="f-1019"></a>Expose Vcs as a core trait that can substitute version-control operations in application tests. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1019) |
| F-1020 | [x] <a id="f-1020"></a>Expose Pty as a core trait that can substitute terminal input, output, resize and restart operations. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1020) |
| F-1021 | [ ] <a id="f-1021"></a>Expose Transcriber as a core trait that can return text or NO_SPEECH without live model calls. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1021) |
| F-1022 | [x] <a id="f-1022"></a>Expose Clock as a core trait that supports deterministic advancement of cooldowns, loop waits and schedules. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1022) |
| F-1023 | [ ] <a id="f-1023"></a>Expose Notifier as a core trait that records notification requests in tests. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1023) |

### Feature and application ownership

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-1024 | [ ] <a id="f-1024"></a>Implement the Feature trait with before_turn, on_event and after_turn hooks; test hook order across a complete turn. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1024) |
| F-1025 | [ ] <a id="f-1025"></a>Keep goal execution and the verifier in the goal feature module, with passing goal-round and verification scenarios. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1025) |
| F-1026 | [ ] <a id="f-1026"></a>Keep plan mode in its feature module and test that the instructed plan flow completes without agy headless plan-mode hangs. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1026) |
| F-1027 | [ ] <a id="f-1027"></a>Keep loop and schedule behavior in feature modules driven by the Clock port. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1027) |
| F-1028 | [ ] <a id="f-1028"></a>Keep background-job detection and lifecycle in its feature module. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1028) |
| F-1029 | [ ] <a id="f-1029"></a>Keep account handoff in its feature module and preserve the same agy conversation during rotation. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1029) |
| F-1030 | [ ] <a id="f-1030"></a>Keep mid-turn steering in its feature module and deliver steering while a turn is active. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1030) |
| F-1031 | [ ] <a id="f-1031"></a>Keep fork behavior in its feature module and preserve the selected conversation state in the child session. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1031) |
| F-1032 | [ ] <a id="f-1032"></a>Keep keep-awake behavior in its feature module and release its operating-system assertion after work stops. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1032) |
| F-1033 | [ ] <a id="f-1033"></a>Add a feature by adding a focused module and registration rather than editing a central object that owns every feature. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1033) |
| F-1034 | [ ] <a id="f-1034"></a>Accept typed Commands in crates/app and produce typed Events through the shared application services. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1034) |
| F-1035 | [x] <a id="f-1035"></a>Keep one append-only event log per session; replay reconstructs the same state after restart. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1035) |
| F-1036 | [x] <a id="f-1036"></a>Recover lagging event consumers through replay or explicit resynchronization without silently dropping required events. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1036) |

### Operating-system adapters, server and CLI

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-1037 | [x] <a id="f-1037"></a>The agy adapter launches agy with stream-json output and parses every scripted success and failure scenario. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1037) |
| F-1038 | [ ] <a id="f-1038"></a>The accounts adapter isolates file-token access using a per-account HOME and SSH_CONNECTION on every agy invocation. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1038) |
| F-1039 | [ ] <a id="f-1039"></a>The store adapter reads and writes JSON compatible with every captured legacy data specimen without losing unknown preserved data. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1039) |
| F-1040 | [x] <a id="f-1040"></a>The git adapter uses the git CLI to create, inspect and remove session worktrees through the Vcs port. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1040) |
| F-1041 | [ ] <a id="f-1041"></a>The pty adapter uses portable-pty and passes terminal interaction tests using ConPTY on Windows. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1041) |
| F-1042 | [ ] <a id="f-1042"></a>The transcribe adapter uses agy with gemini-3.8-flash-high and returns no inserted text for NO_SPEECH. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1042) |
| F-1043 | [ ] <a id="f-1043"></a>The axum server keeps the captured HTTP method, path, authentication, body, error and SSE contracts except documented intentional corrections. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1043) |
| F-1044 | [x] <a id="f-1044"></a>antiquota serve hosts the built UI and the HTTP/SSE API for browser mode. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1044) |
| F-1045 | [x] <a id="f-1045"></a>Build one clap-based antiquota executable with serve, every existing CLI command and the new hook subcommand. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1045) |
| F-1046 | [x] <a id="f-1046"></a>The hook subcommand starts in milliseconds and applies the permission decision before the protected tool executes; record startup timings. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1046) |

### Typed UI architecture

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-1047 | [x] <a id="f-1047"></a>Build ui/ with Svelte 5, TypeScript strict mode and Vite using pnpm. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1047) |
| F-1048 | [x] <a id="f-1048"></a>Generate API types from Rust core and server types with ts-rs into ui/src/gen/. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1048) |
| F-1049 | [x] <a id="f-1049"></a>Fail the stale-type check when committed generated API types differ from freshly generated output. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1049) |
| F-1050 | [x] <a id="f-1050"></a>Use generated API types rather than hand-written duplicate API request, response or event interfaces. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1050) |
| F-1051 | [x] <a id="f-1051"></a>Use one typed API client at ui/src/lib/api.ts; UI components make no direct fetch calls. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1051) |
| F-1052 | [x] <a id="f-1052"></a>Use one SSE event store; test event reduction, reconnect recovery and session selection with the same generated Event types. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1052) |
| F-1053 | [x] <a id="f-1053"></a>Use xterm.js and its fit addon directly for terminal interaction. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1053) |
| F-1054 | [ ] <a id="f-1054"></a>Use the chart library directly for usage plots and tooltips. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1054) |
| F-1055 | [x] <a id="f-1055"></a>Use MediaRecorder directly and upload its supported container with the correct audio type while accepting recorded WAV test input. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1055) |
| F-1056 | [x] <a id="f-1056"></a>Define design tokens as CSS variables in one file and render both light and dark themes. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1056) |
| F-1057 | [x] <a id="f-1057"></a>Preserve the dense, warm, dark tool appearance; keep the light palette and narrow-window states legible. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1057) |
| F-1058 | [ ] <a id="f-1058"></a>Run the same UI build in the Tauri webview and the browser served by antiquota serve. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1058) |
| F-1059 | [x] <a id="f-1059"></a>Keep UI state in stores and use a focused component for each view or widget. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1059) |

### Thin desktop shell

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-1060 | [x] <a id="f-1060"></a>Keep app/src-tauri a thin shell that bundles the single Rust antiquota executable as a sidecar without Python or PyInstaller. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1060) |
| F-1061 | [ ] <a id="f-1061"></a>Preserve engine.json discovery so CLI, browser and desktop clients share sessions with the same running engine. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1061) |
| F-1062 | [ ] <a id="f-1062"></a>Keep native tray, application menus, notifications and autostart owned by the desktop shell. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1062) |
| F-1063 | [x] <a id="f-1063"></a>Implement desktop zoom through native webview setZoom; never apply CSS zoom to the desktop root or embedded split pane. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1063) |
| F-1064 | [ ] <a id="f-1064"></a>Use tauri-plugin-window-state to restore size, position and maximized state while excluding visibility. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1064) |
| F-1065 | [ ] <a id="f-1065"></a>A restored off-screen window falls back to a centered visible window, and --hidden remains hidden after saved state is restored. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1065) |

### Tooling, code rules and documentation

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-1066 | [x] <a id="f-1066"></a>just dev runs both development toolchains with a working UI and engine. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1066) |
| F-1067 | [x] <a id="f-1067"></a>just test runs the Rust and TypeScript test suites. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1067) |
| F-1068 | [x] <a id="f-1068"></a>just check runs the required Rust, TypeScript, formatting and generated-type checks. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1068) |
| F-1069 | [x] <a id="f-1069"></a>just build builds the engine, UI and sidecar desktop bundle. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1069) |
| F-1070 | [x] <a id="f-1070"></a>just gen-types regenerates the TypeScript API types from Rust. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1070) |
| F-1071 | [x] <a id="f-1071"></a>Use thiserror in library crates and reserve anyhow for executable binaries. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1071) |
| F-1072 | [x] <a id="f-1072"></a>Use tracing for Rust logs and serde for persisted and wire types. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1072) |
| F-1073 | [x] <a id="f-1073"></a>cargo clippy -- -D warnings passes without ignored warnings. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1073) |
| F-1074 | [x] <a id="f-1074"></a>Rust source passes rustfmt and dependency policy passes cargo-deny. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1074) |
| F-1075 | [x] <a id="f-1075"></a>Unsafe Rust is confined to the pty adapter. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1075) |
| F-1076 | [x] <a id="f-1076"></a>pnpm check passes svelte-check and strict TypeScript checking. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1076) |
| F-1077 | [x] <a id="f-1077"></a>TypeScript and Svelte source passes eslint and prettier and contains no any types. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1077) |
| F-1078 | [x] <a id="f-1078"></a>Keep modules and components near or below 400 lines, each with a doc comment that states its ownership. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1078) |
| F-1079 | [x] <a id="f-1079"></a>docs/ARCHITECTURE.md explains crates, ports, adapters, UI and shell with a diagram; short ADRs record key implementation decisions. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1079) |
| F-1080 | [ ] <a id="f-1080"></a>Each vertical slice works through engine, API and UI before the next slice; record passing checks with the slice commit. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1080) |

### Deterministic engine and UI proof

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-1081 | [x] <a id="f-1081"></a>Every crate has meaningful unit tests for the behavior it owns. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1081) |
| F-1082 | [x] <a id="f-1082"></a>Use proptest for permission policy, including shell-aware command splitting and the RISKY flag cases. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1082) |
| F-1083 | [x] <a id="f-1083"></a>Use proptest for account routing and rotation invariants. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1083) |
| F-1084 | [x] <a id="f-1084"></a>Use proptest for event-log replay invariants, including persistence and replay after interruption. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1084) |
| F-1085 | [x] <a id="f-1085"></a>Use Vitest for UI stores, including event reducers and reconnect behavior. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1085) |
| F-1086 | [x] <a id="f-1086"></a>Use Vitest for the typed API client and request/error handling. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1086) |
| F-1087 | [x] <a id="f-1087"></a>Use Vitest for formatting functions and boundary values. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1087) |
| F-1088 | [x] <a id="f-1088"></a>Provide a fake-agy test executable that plays a successful stream-json turn. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1088) |
| F-1089 | [x] <a id="f-1089"></a>The fake-agy executable plays a quota wall in the middle of a turn. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1089) |
| F-1090 | [x] <a id="f-1090"></a>The fake-agy executable plays a rate-limit response. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1090) |
| F-1091 | [x] <a id="f-1091"></a>The fake-agy executable plays an authentication failure. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1091) |
| F-1092 | [x] <a id="f-1092"></a>The fake-agy executable plays tool calls requiring approval. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1092) |
| F-1093 | [x] <a id="f-1093"></a>The fake-agy executable invokes the PreToolUse hook. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1093) |
| F-1094 | [x] <a id="f-1094"></a>The fake-agy executable emits a RUN_IN_BACKGROUND: line. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1094) |
| F-1095 | [x] <a id="f-1095"></a>The fake-agy executable emits GOAL_COMPLETE. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1095) |
| F-1096 | [x] <a id="f-1096"></a>The fake-agy executable emits huge output without losing required state or events. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1096) |
| F-1097 | [x] <a id="f-1097"></a>The fake-agy executable crashes and the engine reports and recovers the failure. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1097) |
| F-1098 | [x] <a id="f-1098"></a>Whole-engine tests with fake-agy prove handoff to the next account and cooldown enforcement. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1098) |
| F-1099 | [x] <a id="f-1099"></a>Whole-engine tests with fake-agy prove goal rounds, completion checks and the verifier. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1099) |
| F-1100 | [ ] <a id="f-1100"></a>Whole-engine tests prove approval timeout behavior using a fake Clock. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1100) |
| F-1101 | [x] <a id="f-1101"></a>Whole-engine tests prove loop and schedule timing using a fake Clock. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1101) |
| F-1102 | [x] <a id="f-1102"></a>Whole-engine tests prove mid-turn steering, plan mode and fork behavior. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1102) |
| F-1103 | [ ] <a id="f-1103"></a>Contract tests replay every captured API fixture against the Rust server and document each intentional difference. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1103) |

### Browser end-to-end proof

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-1104 | [ ] <a id="f-1104"></a>Playwright in Chromium drives antiquota serve backed by fake-agy through every UI view and settings section. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1104) |
| F-1105 | [ ] <a id="f-1105"></a>Playwright exercises every shortcut and context menu action in the feature inventory. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1105) |
| F-1106 | [ ] <a id="f-1106"></a>Playwright exercises approval cards and their allow, deny, expiry and resolved states. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1106) |
| F-1107 | [x] <a id="f-1107"></a>Playwright verifies grouped transcript steps in expanded and collapsed states. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1107) |
| F-1108 | [ ] <a id="f-1108"></a>Playwright verifies both split panes at 80, 100, 125 and 150 percent zoom. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1108) |
| F-1109 | [x] <a id="f-1109"></a>Playwright types into the terminal, resizes it and restarts its process. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1109) |
| F-1110 | [x] <a id="f-1110"></a>Playwright uses recorded WAV audio and silence to verify dictation text and the silence guard. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1110) |
| F-1111 | [x] <a id="f-1111"></a>Playwright verifies the usage chart and its tooltips. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1111) |
| F-1112 | [x] <a id="f-1112"></a>Playwright exercises project and session menus. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1112) |
| F-1113 | [x] <a id="f-1113"></a>Playwright captures light and dark themes and narrow-window states. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1113) |

### Native desktop proof

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-1114 | [ ] <a id="f-1114"></a>Computer-use tests against the built app verify window size and position after a real quit and relaunch. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1114) |
| F-1115 | [ ] <a id="f-1115"></a>Computer-use tests exercise every application menu item, the tray and zoom shortcuts. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1115) |
| F-1116 | [ ] <a id="f-1116"></a>Computer-use tests verify notifications, the autostart toggle and --hidden startup. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1116) |
| F-1117 | [ ] <a id="f-1117"></a>Computer-use tests kill the engine and verify the shell respawns it. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1117) |
| F-1118 | [ ] <a id="f-1118"></a>Computer-use tests install the app from the rebuilt DMG. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1118) |
| F-1119 | [x] <a id="f-1119"></a>Desktop evidence uses background-mode, window-targeted screenshots and clicks; full-screen control requires the user's permission. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1119) |

### Live-account and browser authorization proof

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-1120 | [ ] <a id="f-1120"></a>Run one short live agy smoke scenario per feature only inside ~/Desktop/test and record each scenario in the report. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1120) |
| F-1121 | [ ] <a id="f-1121"></a>Check real account quotas before live calls and use gemini-3.8-flash models with short prompts. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1121) |
| F-1122 | [ ] <a id="f-1122"></a>Live tests prove a real account handoff across the five existing signed-in accounts. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1122) |
| F-1123 | [x] <a id="f-1123"></a>Live tests prove a real goal and verifier run. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1123) |
| F-1124 | [x] <a id="f-1124"></a>Obtain the user's approval before any paid live rewrite scenario, then record its token cost in TEST_REPORT.md; the owner approved live rewrite tests on 2026-10-11. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1124) |
| F-1125 | [x] <a id="f-1125"></a>Use Chrome with the user's signed-in Google accounts for agy OAuth only for the five existing account slots. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1125) |
| F-1126 | [x] <a id="f-1126"></a>Use Chrome to check account identity, quota pages and the served web GUI. | User rewrite task | Not applicable | [Passed](TEST_REPORT.md#f-1126) |
| F-1127 | [ ] <a id="f-1127"></a>Never enter passwords or 2FA codes, change Google account settings or grant another application or scope; stop and ask if required. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1127) |

### Compatibility, safety and evidence gates

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-1128 | [ ] <a id="f-1128"></a>Before the Rust engine first touches real data, create and verify a local ~/.antiquota.bak-<date> backup; never commit its tokens. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1128) |
| F-1129 | [ ] <a id="f-1129"></a>Read the user's existing ~/.antiquota with no migration loss and preserve all five signed-in account tokens. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1129) |
| F-1130 | [ ] <a id="f-1130"></a>Preserve the conversation store link and the captured sessions, slots, settings, rules and state formats. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1130) |
| F-1131 | [ ] <a id="f-1131"></a>Never modify the installed agy CLI or the Antigravity IDE configuration in ~/.gemini. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1131) |
| F-1132 | [ ] <a id="f-1132"></a>Keep every artifact in normal English and pass the repository content guard required by the handoff hard rules. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1132) |
| F-1133 | [ ] <a id="f-1133"></a>Keep verifier and default models set to gemini-3.8-flash-high. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1133) |
| F-1134 | [ ] <a id="f-1134"></a>Reject allow rules for commands that can execute arbitrary programs, including xargs, awk, sh -c, env and curl. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1134) |
| F-1135 | [ ] <a id="f-1135"></a>Keep multi-agent workflows as use cases run with the product; add no watcher or fixer/auditor roles to the product. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1135) |
| F-1136 | [ ] <a id="f-1136"></a>Every canonical feature ID links to at least one passing test and recorded evidence before parity is declared. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1136) |
| F-1137 | [ ] <a id="f-1137"></a>TEST_REPORT.md records feature ID, tests, status and evidence path; evidence remains in ignored .handoff/evidence/. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1137) |
| F-1138 | [ ] <a id="f-1138"></a>Intentional API or data-format changes and any feature removal require the user's decision; retain every legacy acceptance item meanwhile. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1138) |

### Cross-platform CI

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-1139 | [ ] <a id="f-1139"></a>GitHub Actions runs cargo tests and clippy on macOS, Windows and Linux. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1139) |
| F-1140 | [ ] <a id="f-1140"></a>GitHub Actions runs pnpm check and Vitest on macOS, Windows and Linux. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1140) |
| F-1141 | [ ] <a id="f-1141"></a>GitHub Actions runs Playwright and the stale-generated-types check on macOS, Windows and Linux. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1141) |
| F-1142 | [ ] <a id="f-1142"></a>GitHub Actions builds the Rust sidecar app on macOS, Windows and Linux, including platform paths and bundling checks. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1142) |
| F-1143 | [ ] <a id="f-1143"></a>Windows CI verifies ConPTY terminal behavior and the sidecar paths in the packaged app. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1143) |
| F-1144 | [ ] <a id="f-1144"></a>Update .github/workflows/app.yml to run the new toolchains and upload the native bundles and installers. | User rewrite task | Not applicable | [Pending](TEST_REPORT.md#f-1144) |

### Owner requests after Slice 13

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-1145 | [x] <a id="f-1145"></a>Every user prompt and every completed agent reply has a visible Copy button that copies its exact text. | Owner request 2026-10-09 | Not applicable | [Passed](TEST_REPORT.md#f-1145) |
| F-1146 | [ ] <a id="f-1146"></a>Usage shows input, cached input, output and thinking tokens and the cache hit rate (cached input over all input) for the period and today, for each account, model and day; agy's `cache_read_tokens` reach the ledger as `cache`. | Owner request 2026-10-09 | Not applicable | [Pending](TEST_REPORT.md#f-1146) |
| F-1147 | [x] <a id="f-1147"></a>Token counts are labelled input and output (with thinking and cached input when present), never as arrows that read as sent and received. | Owner request 2026-10-09 | Not applicable | [Passed](TEST_REPORT.md#f-1147) |
| F-1148 | [ ] <a id="f-1148"></a>Several sessions can run on one account at once: turns, the verifier, dictation and raw runs share the account lease; probes, checks and removal need it alone, and the account reads busy while any call runs. | Owner request 2026-10-09 | Not applicable | [Pending](TEST_REPORT.md#f-1148) |
| F-1149 | [x] <a id="f-1149"></a>The Usage view refreshes itself shortly after a turn ends and every minute while visible, keeping the shown period; a failed background read keeps the last data without an error, and it never cancels a period change. | Owner request 2026-10-09 | Not applicable | [Passed](TEST_REPORT.md#f-1149) |
| F-1150 | [x] <a id="f-1150"></a>A placement setting chooses where new sessions go: fill first (the default) keeps them on the first ranked account until it is walled or below the reserve; spread first prefers accounts without a running call, then the rotation order. A session keeps its own account in both. The default stays out of the settings wire format, and other values are rejected. | Owner request 2026-10-09 | Not applicable | [Passed](TEST_REPORT.md#f-1150) |

### Owner requests of 2026-10-11

| Feature ID | Acceptance behavior | Source or requirement | Related legacy test | Rewrite proof |
| --- | --- | --- | --- | --- |
| F-1151 | [x] <a id="f-1151"></a>One Antigravity and account-rotation mark supplies the sidebar, splash, browser favicon, platform app icons and monochrome tray assets. The sidebar remains legible in both existing themes; generated assets share the canonical geometry. | Owner request 2026-10-11 | Not applicable | [Passed](TEST_REPORT.md#f-1151) |
| F-1152 | [x] <a id="f-1152"></a>The installed application and native application identity are named Antiquota. The GUI wordmark, splash, page and window titles and CLI remain lowercase antiquota. Remove the test display label Native Proof, while preserving internal identifiers, commands and storage compatibility. | Owner request 2026-10-11 | Not applicable | [Passed](TEST_REPORT.md#f-1152) |

## Source risks and completion boundaries

The legacy light theme follows the operating-system preference; it has no manual selector. Test both palettes. Typed `/skills`, `/hooks` and `/mcp` exist but are absent from suggestions, and `/stack` selects a drawer state with no renderer. Repair the inconsistency without dropping the commands.

Source review identifies an empty-model-family dereference, stale boolean switch callbacks, a settings-heading comma expression and a tray/API verification-type mismatch. The event-queue overflow gap is also source-confirmed. These are not claimed as observed Gemini-run failures without runtime evidence. See the subsidiary inventories for source citations and the bug ledger for observed defects.

The all-green gate requires actual passing tests for every feature ID, including browser, desktop and live-account evidence where required. When that gate is reached, present TEST_REPORT.md to the user. Merge, removal of the old stack, README cutover and push retain the user's separate authorization gates.
