# ADR 0039: Compact activity, live usage and the native folder picker

The owner asked on 2026-10-12 for a Claude Code style transcript, visible skill loads, a system folder picker, no model choice when creating a session, and token counters that move during a turn.

agy closes every tool step with an empty `agent_response`. The transcript showed each one as an empty reply with a Copy button, which split every tool into its own one-step activity row. The UI now drops replies whose text is blank before grouping. Adjacent tools fold into one row such as "Ran 3 commands, read 2 files" between the model's real text. Durable history is unchanged; only presentation skips the empty records. Grouped view stays the default.

A tool that reads `<name>/SKILL.md`, through `view_file` or a plain `cat`, `head`, `tail`, `less`, `bat` or `sed -n` command, is shown as "Load skill <name>" and leads the activity summary as "Loaded skill <name>". Piped or compound commands stay ordinary commands.

Only the turn's final reply shows Copy all the time. Earlier replies show it on hover or keyboard focus.

agy reports usage on each completed step, but the engine only added it to the session at the turn end. Long goal turns showed zero for many minutes. `TurnSink` gains a default `usage` hook. The agy runner calls it when the running total changes, and the application publishes a session `meta` event with the stored usage plus the running total. That event is never stored, so the durable turn end still adds the final usage exactly once. Account day totals still update at the turn end.

The desktop shell adds a `pick_folder` command on tauri-plugin-dialog. It opens Finder on macOS, the Explorer folder dialog on Windows and the desktop portal or GTK dialog on Linux. In the native app the new-session dialog shows "Choose folder…" and hides the in-app folder listing; the browser keeps the listing. The new-session dialog no longer offers model or effort. It sends the saved default, and the composer changes the model afterwards.
