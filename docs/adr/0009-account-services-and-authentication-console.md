# ADR 0009: Account services and the authentication console

Status: Accepted for Slice 2. Desktop terminal functionality remains pending until Slice 7.

Account operations live in a small application service that depends on account, quota, settings-storage and clock ports. The session service shares this account service and its typed event sender. Standalone CLI account commands construct only the account service: they never load sessions, replay events or reconcile the usage ledger. This keeps offline account maintenance safe while a discovered engine owns active sessions.

Routing is pure domain policy. Reserve precedes affinity for all strategies; reset selection uses the limiting quota window and the existing 120-second padding. The handoff feature decides continuation after quota, authentication, transient and lost-conversation outcomes. The application owns exclusive leases, cancellation and persistence. A clock port supplies both instants and cancellable waits, allowing full retries and affinity to be tested without elapsed policy delays.

Every account retains its opaque token in its private HOME. Shared configuration is cloned into the owned stack; link farms exclude credentials and per-process state. Metadata uses the verified agy commands and bounded process groups or Windows jobs. Successful current steps outrank a historical error retained in a conversation result, including raw CLI calls. Raw argv and output are forwarded unchanged.

Authentication requires terminal input before the general terminal slice. The portable PTY adapter therefore provides only a bounded console for agy's own authorization-code conversation in Slice 2. It uses the legacy 8000-column width so OAuth URLs and JSON are not wrapped, and it cleans up the whole process group or Windows job. Terminal routes, widgets and resize commands remain pending. Windows and Linux runtime behavior still requires CI and platform proof; local evidence proves macOS only.

The CLI restores the existing default `accounts` command, raw `run`, login, probe, doctor, stack and unwall behavior. A proposed shared-session command is kept as an ignored reviewable draft; no additional public command is shipped without the user's choice. The existing goal command will be implemented in its scheduled slice.

Intentional corrections preserve data and successful work: verification URLs stop at JSON string delimiters, raw success does not create a quota wall from a stale result, unreadable settings use defaults without rewriting the source, and reseeding retains the owned denying hook. Unknown JSON remains in persisted settings and quota caches. Invalid future quota-window shapes do not hide valid known windows in the public projection.

Primary implementation references: [portable-pty 0.9.0](https://docs.rs/portable-pty/0.9.0/portable_pty/), [process-wrap 10.0.1](https://docs.rs/process-wrap/10.0.1/process_wrap/tokio/), [Windows job ownership](https://docs.rs/win32job/2.0.3/win32job/struct.Job.html), [junction deletion](https://docs.rs/junction/2.1.0/junction/fn.delete.html), [Chrono local dates](https://docs.rs/chrono/0.4.45/chrono/struct.Local.html) and [cached literal regular expressions](https://docs.rs/regex/1.13.1/regex/macro.regex.html).
