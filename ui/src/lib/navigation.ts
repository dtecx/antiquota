/** Owns stable project grouping, local filtering, and pinned session ordering. */
import type { Session } from '../gen/Session';
export function groupSessions(sessions: Session[], filter: string) {
  const folders = new Map<string, Session[]>();
  const terms = filter.toLowerCase().split(/\s+/);
  for (const session of sessions) {
    if (
      session.archived ||
      !terms.every((term) => `${session.title} ${session.cwd}`.toLowerCase().includes(term))
    )
      continue;
    const group = folders.get(session.cwd) ?? [];
    group.push(session);
    folders.set(session.cwd, group);
  }
  return [...folders].map(([cwd, group]) => ({
    cwd,
    sessions: group.sort((a, b) => Number(b.pinned) - Number(a.pinned) || b.updated - a.updated),
  }));
}
