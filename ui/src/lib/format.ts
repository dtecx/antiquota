/** Owns display formatting for paths, time, and token counters. */
export function formatTokens(value: number): string {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}k`;
  return String(value);
}

/** All prompt tokens: fresh input plus the cached input agy reused. */
export function inputTokens(usage: { input: number; cache?: number }): number {
  return usage.input + (usage.cache ?? 0);
}

export function relativeTime(timestamp: number, now = Date.now() / 1_000): string {
  const difference = Math.max(0, now - timestamp);
  if (difference < 60) return 'now';
  if (difference < 3_600) return `${Math.floor(difference / 60)}m`;
  if (difference < 86_400) return `${Math.floor(difference / 3_600)}h`;
  return new Date(timestamp * 1_000).toLocaleDateString(undefined, {
    month: 'short',
    day: 'numeric',
  });
}

export function projectName(path: string): string {
  return (
    path
      .replace(/[\\/]+$/, '')
      .split(/[\\/]/)
      .at(-1) ||
    path ||
    'No folder'
  );
}

export function duration(seconds: number): string {
  if (seconds < 1) return '<1s';
  if (seconds < 60) return `${seconds.toFixed(seconds < 10 ? 1 : 0)}s`;
  return `${Math.floor(seconds / 60)}m ${Math.round(seconds % 60)}s`;
}

const STATUS: Record<string, string> = {
  running: 'working',
  approval: 'needs your approval',
  waiting: 'waiting for an account',
  checking: 'running check',
  verifying: 'verifier reviewing',
  sleeping: 'sleeping',
};
/** How a session status reads in the header and status line. */
export function statusLabel(status: string): string {
  return STATUS[status] ?? status;
}
/** Statuses during which a turn is still in progress. */
export function isBusy(status: string | undefined): boolean {
  return ['running', 'approval', 'waiting', 'checking', 'verifying'].includes(status ?? '');
}
