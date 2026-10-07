# ADR 0013: Git worktrees and project files

Status: Accepted for Slice 6.

The `Vcs` port covers what a session needs from git: the repository root, a worktree for a new session, status with counts, diffs (uncommitted, or committed against the worktree's base), commit, discard, a guarded merge, removal and branch renaming. The new `antiquota-git` adapter runs the `git` command with credential prompts and optional locks off and never pushes. Errors report git's last line. Merge refuses unless the main checkout is on the base branch and both checkouts are clean, so the user's files are never touched.

A worktree session gets a branch named after its title and session id (`antiquota/<slug>-<id prefix>`) and a folder under the data root's `worktrees/`, keeping the working subfolder. An untitled session's branch takes the first request's title. Removing the worktree moves the session to the main checkout with a fresh conversation, archives it and leaves a note that agy reads once with its next request; the same `pending_note` path will carry rewind, compact and shell notes.

The `Workspace` port reads a session's project for the Files tab and the file viewer: one folder at a time (real-path containment, folders first, hidden entries last, at most 400 entries) and text files up to 2 MB inside the project or the session's attachments. Linked folders and files cannot leave those roots. Query parameters are percent-decoded, so paths with spaces and Unicode work.

In the UI, a side drawer (⌘B) holds Changes and Files. Changes loads diffs only when a file is expanded, confirms discards and worktree removal in an in-app dialog, and lists the agent's recorded edits by file. Worktree sessions show their branch in the header and group under their repository in the sidebar.
