# ADR 0014: Terminal

Status: Accepted for Slice 7.

The `Terminals` port starts one shell per session for the Terminal panel; `Terminal` writes input, resizes, kills and hands out subscriptions. The PTY adapter runs the user's login shell (`$SHELL -l`, or `/bin/zsh`) in a pseudo-terminal, and `COMSPEC` or PowerShell through ConPTY and a job object on Windows. It keeps the last 256 KiB of output for late or reconnecting subscribers and gives each subscriber a 4000-chunk queue. A full queue drops output for that subscriber only, a closed one is removed, and the exit notice always arrives, so a lagging client still learns that the shell ended (F-278). Kill sends SIGHUP to the shell's process group; restart, session deletion and engine shutdown kill too.

The stream is server-sent events in the legacy shape: a first frame with base64 recent output and `shell`/`cwd`/`alive`, then base64 chunks with bursts of up to 64 combined, a ping every 15 seconds while quiet, and `{"exit": true}`. A blocking thread reads the subscription and feeds the async response; when the client disconnects the thread ends and the subscription closes. EventSource cannot send headers, so the stream takes the token from the query, as `/api/events` does.

The UI loads xterm.js 6 and its fit addon lazily as a separate chunk and keeps one xterm instance per session across tab and session switches. The shell object lives outside Svelte's state proxy (`$state.raw`), because xterm and the connection change it directly.
