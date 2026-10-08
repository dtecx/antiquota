/** Loads local ledger periods and derives chart statistics without formatting away totals. */
import { writable } from 'svelte/store';
import type { Api } from './api';
import type { UsageResponse } from '../gen/UsageResponse';
export function usageSummary(data: UsageResponse) {
  const total = data.days.reduce((n, d) => n + d.total, 0),
    turns = data.days.reduce((n, d) => n + d.turns, 0);
  return {
    total,
    turns,
    today: data.days.at(-1) ?? { total: 0, turns: 0 },
    peak: Math.max(0, ...data.days.map((d) => d.total)),
    mean: turns ? total / turns : 0,
    accounts: Object.entries(data.accounts).sort((a, b) => b[1].tokens - a[1].tokens),
    models: Object.entries(data.models).sort((a, b) => b[1] - a[1]),
  };
}
export function dayAccounts(day: UsageResponse['days'][number]) {
  return Object.entries(day.by).sort((a, b) => b[1] - a[1]);
}
export function tooltipLeft(index: number, count: number, width: number, tooltipWidth: number) {
  return Math.max(
    0,
    Math.min(width - tooltipWidth, (width * (index + 0.5)) / Math.max(1, count) - tooltipWidth / 2),
  );
}
export function createUsage(api: Api) {
  const state = writable<{
    days: number;
    data: UsageResponse | null;
    loading: boolean;
    error: string;
  }>({ days: 14, data: null, loading: false, error: '' });
  let generation = 0;
  async function load(days = 14) {
    const own = ++generation;
    state.update((v) => ({ ...v, days, loading: true, error: '' }));
    try {
      const data = await api.usage(days);
      if (own === generation) state.update((v) => ({ ...v, data }));
    } catch (error) {
      if (own === generation)
        state.update((v) => ({
          ...v,
          error: error instanceof Error ? error.message : 'Usage is unavailable.',
        }));
    } finally {
      if (own === generation) state.update((v) => ({ ...v, loading: false }));
    }
  }
  return { state, load, dispose: () => generation++ };
}
