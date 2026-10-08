/** Owns new-session defaults and compatible handoff choices. */
import type { Session } from '../gen/Session';
import type { DialogState, WorkbenchState } from './workbench-state';

export function dialogDefaults(
  view: WorkbenchState,
  kind: DialogState['kind'],
  options: Partial<DialogState>,
): DialogState {
  const selected = view.selected,
    defaults = view.snapshot?.defaults;
  const worktree = selected?.worktree;
  const repo =
    worktree &&
    typeof worktree === 'object' &&
    !Array.isArray(worktree) &&
    typeof worktree.repo === 'string'
      ? worktree.repo
      : '';
  const mode = defaults?.mode ?? '';
  return {
    id: kind === 'rename' ? selected?.id : undefined,
    kind,
    title: kind === 'rename' ? (selected?.title ?? '') : '',
    cwd:
      kind === 'rename'
        ? (selected?.cwd ?? '')
        : repo || selected?.cwd || defaults?.default_cwd || defaults?.cwd || '',
    model: selected?.model || defaults?.model || 'gemini-3.8-flash-high',
    mode:
      selected?.mode ||
      (['default', 'accept-edits', 'plan', 'bypass'].includes(mode)
        ? (mode as Session['mode'])
        : 'default'),
    worktree: kind === 'create' && defaults?.worktree_default === 'on',
    ...options,
  };
}
export function earlierSessions(session: Session | null, sessions: Session[]) {
  return session
    ? sessions
        .filter((row) => row.id !== session.id && row.cwd === session.cwd && row.turns > 0)
        .sort((a, b) => b.updated - a.updated)
        .slice(0, 3)
    : [];
}
