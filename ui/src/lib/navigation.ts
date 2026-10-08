/** Owns stable project grouping, local filtering, and pinned session ordering. */
import type { Session } from '../gen/Session';
import { asWorktree } from './workbench';
export const PINNED = '\u0000pinned';
export function groupSessions(
  sessions: Session[],
  filter: string,
  { showArchived = false, current = '' }: { showArchived?: boolean; current?: string } = {},
) {
  const folders = new Map<string, Session[]>();
  const terms = filter.toLowerCase().split(/\s+/);
  for (const session of [...sessions].sort((a, b) => b.updated - a.updated)) {
    if (
      (session.archived && !showArchived && session.id !== current) ||
      !terms.every((term) => `${session.title} ${session.cwd}`.toLowerCase().includes(term))
    )
      continue;
    // A worktree session belongs to the repository it was branched from.
    const folder = session.pinned ? PINNED : (asWorktree(session.worktree)?.repo ?? session.cwd);
    const group = folders.get(folder) ?? [];
    group.push(session);
    folders.set(folder, group);
  }
  return [...folders]
    .sort(([a], [b]) => Number(b === PINNED) - Number(a === PINNED))
    .map(([cwd, group]) => ({
      cwd,
      sessions: group,
    }));
}

export function splitChoices(sessions: Session[], current: string): Session[] {
  const seen = new Set<string>();
  return [...sessions]
    .sort((a, b) => b.updated - a.updated)
    .filter((session) => {
      if (session.archived || session.id === current || seen.has(session.id)) return false;
      seen.add(session.id);
      return true;
    })
    .slice(0, 14);
}

export function stepSession(
  groups: ReturnType<typeof groupSessions>,
  current: string,
  delta: number,
) {
  const sessions = groups.flatMap((group) => group.sessions);
  if (!sessions.length) return undefined;
  const index = sessions.findIndex((session) => session.id === current);
  if (index < 0) return sessions[delta < 0 ? sessions.length - 1 : 0]?.id;
  return sessions[(((index + delta) % sessions.length) + sessions.length) % sessions.length]?.id;
}
