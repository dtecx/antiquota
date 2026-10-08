# ADR 0019: Composer commands and controls

Date: 2026-10-08

Status: Accepted

The browser needs every legacy composer entry point without duplicating engine decisions. The composer delegates slash commands and local prefixes to an application controller, and sends ordinary text through the existing workbench. Rust exports job and loop request types for the client; HTTP keeps its captured legacy input compatibility. Autocomplete owns query generations and insertion, while prompt history owns recall and the saved draft. Model controls use the existing family helper and concrete server model IDs.

Commands capture their initiating session and draft. A delayed response clears only accepted text, and a first prompt is sent to the session actually created. Refresh selects a remaining session after deletion. Inline rename retains its draft while a mutation is pending. Goal dialog completion and deferred upload, suggestion and dictation focus cannot take ownership from newer work.

The shortcut sheet and Settings share their reference and platform labels. The header shows identity, origin, activity and usage; the composer shows context capacity and the selected quota pool. Export fetches durable history, projects repeated step updates, and downloads Markdown with a bounded sanitized filename. Native save, zoom and other desktop adapters remain in Slice 10.

Two legacy entry points need explicit corrections. `/stack` opens the working Skills settings view because the old stack drawer has no renderer. `/mode bypass` uses the same dangerous-action confirmation as the mode selector. Its warning states that bypass ignores antiquota permission rules; it does not promise that the installed hook retains those rules in bypass mode. Unknown or invalid commands stay local and preserve the draft for correction. The composer requires a positive rewind turn, while the server retains captured turn-zero compatibility.
