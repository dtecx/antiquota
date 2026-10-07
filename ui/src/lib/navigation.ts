/** Owns stable project grouping, local filtering, and pinned session ordering. */
import type { Session } from '../gen/Session';
import { asWorktree } from './workbench';
export function groupSessions(sessions: Session[], filter: string) {
  const folders = new Map<string, Session[]>();
  const terms = filter.toLowerCase().split(/\s+/);
  for (const session of sessions) {
    if (
      session.archived ||
      !terms.every((term) => `${session.title} ${session.cwd}`.toLowerCase().includes(term))
    )
      continue;
    // A worktree session belongs to the repository it was branched from.
    const folder = asWorktree(session.worktree)?.repo ?? session.cwd;
    const group = folders.get(folder) ?? [];
    group.push(session);
    folders.set(folder, group);
  }
  return [...folders].map(([cwd, group]) => ({
    cwd,
    sessions: group.sort((a, b) => Number(b.pinned) - Number(a.pinned) || b.updated - a.updated),
  }));
}
