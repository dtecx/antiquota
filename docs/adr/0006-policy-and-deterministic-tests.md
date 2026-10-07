# ADR 0006: One permission policy and controllable time

Status: Accepted. Policy, hook and approval proof landed in Slice 3; jobs, time and scheduling proof remain pending.

Port every legacy shell-aware permission test and RISKY-flag case into core. Conservatively split shell operators while respecting quoting; substitutions, redirects, malformed input, and executable wrappers need a decision. Every compound segment is checked. Arbitrary-program runners are never newly added to allow lists.

The tool hook and agent-generated jobs use the same policy and approval stream. agy's headless runner pairs skip-permissions with its installed denying hook. Protected real launches require preflight proving that the owned hook is present, enabled, executable, and rejects a known-denied call; missing or unusable hooks refuse launch. A hook the user switched off hands decisions to agy's own modes, never native plan mode (F-136, [ADR 0010](0010-hook-approvals-and-steering.md)). An unavailable engine, an unknown request and a timeout deny. Plan mode remains instruction-based because native headless plan mode hangs.

Routing, cooldowns, deadlines, loops, and schedules receive time through `Clock`. A fake clock changes test time without sleeping. Daily schedules preserve local HH:MM behavior and explicitly test daylight-saving changes. A separate cancellable runtime wait lets stop interrupt deadlines promptly.

Default and verifier models remain `gemini-3.8-flash-high`. Completed step usage decides resumed-turn success and accounting; stale aggregate result errors/tokens never override successful current steps. Dictation retains the NO_SPEECH and client silence guards.

Proof: policy/routing/replay properties, fake-agy error scenarios, unavailable-hook denial, approval timeout/stop, guarded background jobs, steering, handoff, goal/verifier rounds, local-calendar scheduling, and speech/silence fixtures. Real paid scenarios require an agreed capped budget and remain separate from offline checks.
