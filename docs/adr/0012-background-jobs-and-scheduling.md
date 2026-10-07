# ADR 0012: Background jobs and scheduling

Status: Accepted for Slice 5.

A `JobRunner` port starts detached jobs in their own process group (a new process group on Windows), appends their output to a per-session log, waits for them, stops their whole tree and reads log tails. Jobs survive the engine; a new engine adopts unfinished jobs and polls their process ids, recording an unknown exit code, as the legacy engine did. The `antiquota-shell` adapter implements it next to the check runner and the `Awake` port, which holds `caffeinate` on macOS and `systemd-inhibit` on Linux; Windows keep-awake is not implemented yet.

Jobs agy asks for are treated like its tool calls (R-001, R-002). `RUN_IN_BACKGROUND:` lines count only outside fenced code blocks, and `<placeholder>` commands never run. Each requested command passes the permission policy for the session's mode: allowed commands start, denied ones are reported, and the rest park on an approval card (`run_in_background`) that starts the job when allowed. Since no hook waits on these cards, the scheduler declines them when their deadline passes. A session whose turn ends with such a card still open keeps the `approval` status.

The run loop starts requested jobs before the goal decision, so a goal waits for its jobs without spending a round. A watcher sends check-ins while the session is idle and a result message when the job ends, unless the job is silent or was stopped. A scheduler tick every five seconds holds or releases keep-awake (turns and running jobs count as busy work), expires background approvals, and dispatches due interval loops and daily schedules while the session is idle and not stopped. Daily times use the local calendar through the `Clock` port.

Fresh conversations receive the background-job instruction before the request, as in the legacy engine.
