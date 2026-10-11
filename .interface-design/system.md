# antiquota interface

## Direction and feel

A quiet developer workbench for continuous agent work across accounts. Preserve the existing dense layout, warm graphite surfaces, amber action color and light theme. The owner requested a shared logo inspired by Antigravity and account rotation on 2026-10-11.

The installed application is **Antiquota**. The GUI wordmark, page and window titles, splash and CLI use **antiquota**. Native application identity in the OS application menu and About panel uses **Antiquota**.

## Depth and spacing

Use the existing subtle borders and layered surfaces in `ui/src/styles/tokens.css`. Spacing uses a 4px base unit. This branding change does not change layout, component sizing or navigation.

## Hierarchy

Keep IBM Plex Sans for interface text and JetBrains Mono for technical detail. The sidebar wordmark stays 19px, weight 600, letter spacing -0.65px. The new mark is 30px with a 12px gap to the wordmark. The splash mark is 80px.

## Shared brand mark

`ui/src/assets/brand-mark.svg` is the canonical artwork: an ascending open A crossed by a tilted, segmented amber orbit with one account node. The A uses the existing ink token; the orbit and node use the existing amber token. Light mode uses its existing darker ink and amber colors.

`npm --prefix app run icons` derives the splash, favicon, platform icons and monochrome tray images using the installed Tauri CLI. The app icon and favicon add a graphite rounded tile. The sidebar and splash use the mark without a tile. The tray keeps the same silhouette as a template image. Do not redraw these assets independently. The mark is static in both motion preferences.

No raster reference shaped this design. The implemented warm palette and the owner's continuity and rotation request set its direction.

## Transcript and account controls

Fenced code uses the existing well surface and mono font, with a fixed language/Wrap/Copy toolbar above the horizontal scroller. Lines remain unwrapped by default; Wrap is a local, keyboard-accessible pressed control. Syntax colors come from four theme tokens for keywords, strings, numbers and comments. These colors and the toolbar text meet 4.5:1 contrast in both settled themes; pressed Wrap uses the keyword token. Unknown, unnamed or oversized blocks retain the same controls and escaped plain text.

Copy conversation sits with the existing transcript presentation controls and copies source Markdown for prompts and completed replies. It uses the existing clipboard feedback; unsent drafts, thinking and tool details are excluded.

Refresh quotas sits beside Add account in account settings. It disables during the existing serialized account operation and reports current progress, refreshed count, busy/removed skips and individual failures using existing status and alert surfaces. Paused accounts can refresh without resuming, and a later refresh can retry failures.

Explicit user prompts show Reuse beside Copy. Reuse loads source text into that pane's composer, keeps current attachments and never sends automatically. Replacing a different unsent draft uses the existing confirmation dialog. Generated prompts do not offer Reuse.

Find sits first in the transcript presentation controls. Its pane-local bar uses the existing surfaces, typography and spacing, wraps its controls in a narrow pane and labels its count as matching messages. A theme-aware outline identifies the current prompt or reply without changing rendered Markdown. Cmd/Ctrl+F focuses the active pane's search; Enter and Shift+Enter navigate, and Escape closes with focus returned to Find while retaining the result viewport. Session changes reset the finder.

Jump to latest floats at the bottom of the transcript when the reader is more than 120 pixels from its end. It uses the existing subtle surface and focus treatment. New output keeps the older reading position until the reader jumps or scrolls back near the bottom. Jump restores following and programmatically focuses the conversation region.

## Interactive terminal

`antiquota -i` uses normal terminal scrollback with compact amber headings and the lowercase wordmark. NO_COLOR keeps headings plain. The single editable row clips horizontally, marks hidden leading text with ‹ and logical newlines with ↵; pasted multiline prompts require explicit Enter. Current model, permission mode, serving account, folder and session id appear above each idle prompt. Permission prompts name the tool, show its arguments and state the one-use allow and default deny choices. Output and pasted control bytes remain inert. The terminal and browser share one session and engine.
