# ADR 0018: Sidebar and shared split view

Status: Accepted for Slice 9c.

Two focused session panes share one engine snapshot, one event connection and device preferences. They have independent selections, transcripts and input controls. Session-keyed drafts, attachment trays, caret positions and scroll positions follow the session when it moves between panes. Split choices and project collapse state keep the existing device keys. Invalid restored choices close the second pane. Selecting the right session from the sidebar swaps the panes; concurrent swaps are ignored until the first pair completes. No iframe, embedded application root or CSS zoom is used.

Pure navigation helpers own recent ordering, pinned groups, worktree parents, archive visibility and visible stepping. The sidebar composes those helpers with debounced message search and application-owned session actions. Search generations discard stale responses after replacing or clearing a query. Menu views own keyboard wrapping, focus restoration, viewport bounds and outside dismissal. Destructive actions capture their targets before confirmation and retain project files. A rename or preferred refresh cannot override a newer selection.

Composer focus depends on selection, readiness and active pane, rather than every snapshot or stream update. Activating a header control retains its focus. Scroll positions are saved before selection removes the old transcript. Dictation belongs to the session where it starts. Selection changes and pane disposal cancel recognition, pending microphone acquisition and transcription delivery. Late streams stop their tracks; canceled speech and transcription cannot change another draft. An already submitted transcription request may finish, but its canceled result is discarded.

Native project reveal remains in Slice 10. Prompt history and command dispatch remain in Slice 9d. Browser tests verify session/project actions, bounded keyboard menus, shared events, independent input and attachments, scroll/caret restoration, split persistence and cancellation. Captured HTTP routes and domain tests continue to run in the whole gate.
