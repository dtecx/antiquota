/** Owns pure session header labels; formatting is shared with the rest of the UI. */
import type { Session } from '../gen/Session';
import type { Usage } from '../gen/Usage';
import type { Worktree } from '../gen/Worktree';
import { clockTime } from './account-format';
import { formatTokens, statusLabel } from './format';

export function shortHeaderPath(path: string): string {
  return path.replace(/^\/(?:Users|home)\/[^/]+(?=\/|$)/, '~');
}

export function headerPath(cwd: string, worktree: Worktree | null) {
  return worktree
    ? {
        label: worktree.branch,
        tooltip: `${worktree.path} · from ${worktree.base} in ${worktree.repo}`,
      }
    : { label: shortHeaderPath(cwd), tooltip: cwd };
}

export function headerStatus(session: Pick<Session, 'status' | 'status_until'>): string {
  if (session.status === 'idle') return '';
  const until = clockTime(session.status_until ?? 0);
  return session.status === 'sleeping' && until
    ? `sleeping until ${until}`
    : statusLabel(session.status);
}

export function headerUsage(usage: Usage): string {
  return usage.input || usage.output
    ? `${formatTokens(usage.input)} in · ${formatTokens(usage.output)} out`
    : '';
}
