# Interactive antiquota

From the Rust build, start a persistent coding conversation in your project:

```sh
antiquota -i
antiquota -i --cwd /path/to/project --mode plan
antiquota -i --resume SESSION_ID
```

The terminal and app share sessions, account routing, transcripts, usage and permission gates. Startup discovers the app's engine or starts one in the background. Closing the terminal leaves that engine available for the app and other sessions. `antiquota gui` opens its browser interface.

Enter sends. Ctrl+J, Shift+Enter when supported, or backslash followed by Enter adds a newline. Bracketed paste keeps multiline text in the draft until you send. The single input row scrolls horizontally and marks newlines with ↵. Up/Down recalls prompts and restores your draft when you return past the newest history entry. Tab completes a unique slash command. Home/End, arrows, Backspace/Delete and Ctrl+A/E/U/K/W edit the prompt. Ctrl+C/Escape clears an idle draft or stops the current turn. Ctrl+D exits from an empty prompt; during work it stops that session first.

When a tool asks permission, inspect its name and arguments. Type `y` to allow that call once; Enter, `n` or Escape denies. Answers never add permanent rules. If the app resolves the request or it expires, the terminal returns to the turn without waiting for an answer.

| Command | Behavior |
| --- | --- |
| `/help` | Show commands and keys. |
| `/status` | Session id, folder, model/mode, account, turns, context and tokens. |
| `/sessions` | List sessions, most recently updated first. |
| `/resume ID` | Switch to a saved session and restore its prompt history. |
| `/new` | Start a session in the same folder with the current model/mode. |
| `/fork` | Fork through the existing engine API. |
| `/rename TITLE` | Rename the current session. |
| `/model [ID]` | List cached models or select a model. |
| `/mode [MODE]` | Select default, accept-edits or plan. |
| `/accounts` | Cached quota, busy/paused/verification state, token totals and wall counts. |
| `/accounts check SLOT` | Probe one account's quota now. |
| `/accounts pause SLOT` / `/accounts resume SLOT` | Stop or restart automatic routing to one account. |
| `/routing [N]` | Show the last N routing decisions (1 to 200, default 20), oldest first. |
| `/attach PATH` | Upload a file (25 MB limit) and stage it for the next message. Relative paths use the session folder; quote paths with spaces. |
| `/attachments` | List the staged files. |
| `/detach N` / `/detach all` | Remove a staged file from the next message. The uploaded copy stays with the session. |
| `/export PATH` | Write the full saved session as JSON, including tool events. The file is private and never overwrites an existing file. |
| `/usage` | Detailed recorded usage, including cached input and thinking. |
| `/diff` | Show the working-tree diff without applying changes. |
| `/compact` | Prepare a summary for a fresh agent context; keep the saved transcript. |
| `/exit` | Leave the terminal session. |

With files staged, Enter sends them even when the prompt is empty. `/new`, `/fork` and `/resume` refuse to switch while files are staged. If a send fails, the draft and staged files are kept.

This slice verifies the interactive terminal on macOS; Windows and Linux execution still need proof.

Prompt history stays in memory; resume rebuilds it from that session's explicit prompts. There is no separate history file. Cursor movement currently uses Unicode scalars, so compound emoji do not have full grapheme editing. Interactive mode requires a terminal. Set NO_COLOR to disable heading colors.

Bare `antiquota` still lists accounts. `antiquota run` still passes its arguments to agy. `antiquota goal` retains its bounded headless workflow. No `chat` command is added.
