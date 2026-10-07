# ADR 0011: Goals, verifier and plan mode

Status: Accepted for Slice 4.

Goal decisions are pure functions in `antiquota_features::goal`: what follows a turn (wait for jobs, nudge, evaluate, retry with 30/60/90-second waits, abandon) and whether a check result and verdict make the goal met. They take the turn's outcome (success, final reply, denied commands) rather than a `Turn`, because the decision needs the reply text; the application runs the check and the verifier between the two decisions. The `Feature` trait stays for decisions that fit a single hook.

Two ports are added. `ShellRunner` runs a user-configured command line with the platform shell in the project folder; the new `antiquota-shell` adapter owns bounded process trees (process group or job object, output tails, timeout, cancellation) and is reused by the agy adapter. `AgentRunner::ask` is a separate read-only agy call in a fresh conversation: `--output-format json`, agy's own permission mode and no session variables, so antiquota's hook leaves it to agy and the verifier can only view and list files. `Clock::local_hhmm` formats wake times for messages.

The run loop decides the next prompt after every turn: a goal follow-up, a retry after a failed goal turn, or a bounded follow-up after denied commands outside goals, before any queued message — under the session lock, so a message sent at that moment is never stranded. When every account is walled and the goal is active, the session sleeps until the earliest reset plus 30 seconds (at least 5) and continues; without a goal the turn ends with the reset time.

Plan mode is instruction-based: the agy adapter prepends the plan note when the turn is hook-gated and the session is in plan mode; native headless plan mode is never passed. The UI's plan bar executes the plan in accept-edits or default mode, or asks for a revision.

`antiquota goal` is a client of the shared engine: it reuses a running engine, or starts one in its own process for the length of the run and stops it afterwards. The legacy command created a second in-process engine even when the app's engine was running, so two writers could touch the same data; the rewrite never does.
