# ADR 0038: Terminal files, export and account controls

The interactive CLI from ADR 0037 lacked three things the browser already had: file attachments, a way to save a whole session, and per-account controls. Add them as slash commands over the existing authenticated engine routes. No new route, engine state or file format is introduced.

`/attach PATH` uploads the file through the raw attach route, with the filename percent-encoded in `X-Filename`, and stages the returned attachment in memory. The next send passes the staged paths in the existing `attachments` field, so the transcript records them as the browser does. Staged files block `/new`, `/fork` and `/resume`, and a failed send keeps both the draft and the staged files. `/detach` only removes a file from the next message; the uploaded copy stays with the session, as in the browser.

`/export PATH` writes the saved session JSON from the existing session read. It opens the file with create-new and mode 0600, so it never overwrites a file and is private to the user. A failed write removes the partial file.

`/accounts check|pause|resume SLOT` call the existing check and pause routes after validating the slot name. `/routing [N]` reads the existing routing log, bounded to the engine's 200-entry read limit.

A real PTY test drives each command against an isolated engine with the fake agent and checks the saved transcript, the export file mode and no-overwrite behavior, and the account paused state through HTTP.
