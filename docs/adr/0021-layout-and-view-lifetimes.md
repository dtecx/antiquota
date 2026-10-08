# ADR 0021: Layout and view lifetimes

Date: 2026-10-08

Status: Accepted

Session creation derives its concrete model, mode and project folder from the current session, or configured defaults when no session is selected. A worktree session contributes its repository folder. Recent folders are distinct, ordered by last activity and bounded to four. Folder browsing waits 200 milliseconds after input changes and invalidates old results immediately.

The workbench owns one shared error and feedback toast. A new message replaces its 2.6-second timer. Initial empty models receive one cancellable retry after four seconds. Sidebar times share a minute clock whose interval ends when its last subscriber leaves. Confirmation replacement resolves the superseded request as cancellation.

Git actions capture their session before confirmation. Status and diff reads belong to the current mounted session; history diffs are fetched on first expansion. Drawer file elements have session-specific identities. Job logs admit only the latest request and cannot reopen after cancellation. File links preserve valid inclusive line ranges and center their first highlighted line.

Responsive layout keeps settings navigation, shortcuts, drawers and terminal content usable within their existing breakpoints. Compact context meters retain the context percentage while hiding less useful labels first. Hidden elements, focus indicators, reduced motion and theme colors use the shared stylesheet. Browser layout never applies root CSS zoom or introduces another root for split view. Native zoom, window restoration, menus, tray and WebKit recording remain the desktop adapter's responsibility.

Account acceptance is folded only where Rust and captured HTTP tests prove every clause. Partial account rows retain their remaining conditions, including live authorization and platform-specific behavior. Subsidiary inventory status links follow the canonical report; historical source and legacy-test references are unchanged.
