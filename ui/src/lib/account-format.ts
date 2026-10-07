/** Owns quota and account display calculations; it never makes routing decisions. */
import type { Account } from '../gen/Account';
import type { Quota } from '../gen/Quota';

export const pools = [
  { key: 'gemini', label: 'Gemini' },
  { key: 'cg', label: 'Claude / GPT' },
] as const;
export function modelPool(model: string): 'gemini' | 'cg' {
  return /^(claude|gpt)/.test(model) ? 'cg' : 'gemini';
}
export function minimumQuota(windows: Quota[string] | undefined): number | null {
  const values = Object.values(windows ?? {}).map(([fraction]) => fraction);
  return values.length ? Math.min(...values) : null;
}
export function quotaPercent(fraction: number | null): string {
  return fraction === null ? '–' : `${Math.round(fraction * 100)}%`;
}
export function quotaWidth(fraction: number | null): number {
  return fraction === null ? 0 : Math.min(100, Math.max(2, fraction * 100));
}
export function shortAccount(slot: string | null | undefined): string {
  return (slot ?? '').split('@')[0] ?? '';
}
export function wallReason(reason: string | undefined): string {
  if (!reason) return 'walled';
  if (/^rate limited|errors/.test(reason)) return 'resting';
  if (/^signed out/.test(reason)) return 'signed out';
  if (/verify/.test(reason)) return 'verify';
  return 'walled';
}
export function clockTime(timestamp: number): string {
  if (!timestamp) return '';
  return new Date(timestamp * 1000).toLocaleTimeString([], {
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  });
}
export function resetDescription(reset: number | null): string {
  return reset === null
    ? 'Disabled until the other window resets'
    : `Resets ${new Date(reset * 1000).toLocaleString()}`;
}
export function accountStrip(account: Account, pool: string) {
  const fraction = minimumQuota(account.quota[pool]);
  return {
    state:
      account.verify || account.paused
        ? 'off'
        : account.walls[pool]
          ? 'wall'
          : fraction !== null && fraction < 0.2
            ? 'low'
            : 'ok',
    fraction:
      account.walls[pool] || account.verify
        ? 0.12
        : fraction === null
          ? 0.5
          : Math.max(0.12, fraction),
  };
}
export function accountSummary(accounts: Account[], servingSlot: string | null, pool: string) {
  const available = accounts.filter(
    (account) => !account.walls[pool] && !account.verify && !account.paused,
  ).length;
  const attention = accounts.filter(
    (account) => account.verify || /^signed out/.test(account.why[pool] ?? ''),
  ).length;
  const serving = accounts.find((account) => account.slot === servingSlot);
  return {
    available,
    attention,
    main: accounts.length
      ? `${available} of ${accounts.length} accounts free`
      : 'Add a Google account',
    detail: !accounts.length
      ? 'Signs in separately from your agy CLI'
      : serving
        ? `${shortAccount(serving.slot)} · ${pool === 'cg' ? 'Claude/GPT' : 'Gemini'} ${quotaPercent(minimumQuota(serving.quota[pool]))}`
        : attention
          ? `${attention} ${attention === 1 ? 'needs' : 'need'} attention`
          : 'Usage and accounts',
  };
}
