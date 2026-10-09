/** Loads local ledger periods and derives chart statistics without formatting away totals. */
import { get, writable } from 'svelte/store';
import type { Api } from './api';
import type { UsageResponse } from '../gen/UsageResponse';
import type { UsageParts } from '../gen/UsageParts';
import { formatTokens } from './format';
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
    detail: data.detail ?? null,
  };
}
/** The share of prompt tokens agy read from its cache: cached / (input + cached). */
export function cacheRate(parts: UsageParts | null | undefined): number | null {
  if (!parts) return null;
  const prompt = parts.input + parts.cache;
  return prompt ? parts.cache / prompt : null;
}
export function percent(rate: number | null): string {
  return rate === null ? '–' : `${Math.round(rate * 100)}%`;
}
/** One line with every part of a token count, as the usage tooltip shows it. */
export function partsLine(parts: UsageParts): string {
  return [
    `input ${formatTokens(parts.input)}`,
    `cached input ${formatTokens(parts.cache)}`,
    `output ${formatTokens(parts.output)}`,
    `thinking ${formatTokens(parts.thinking)}`,
  ].join(' · ');
}
/** The breakdown rows: each part for the period and for today, with what it counts. */
export function breakdown(detail: NonNullable<UsageResponse['detail']>) {
  const { period, today } = detail;
  return [
    {
      label: 'Input',
      period: formatTokens(period.input),
      today: formatTokens(today.input),
      hint: 'Prompt tokens sent to the model, apart from the cached part',
    },
    {
      label: 'Cached input',
      period: formatTokens(period.cache),
      today: formatTokens(today.cache),
      hint: 'Prompt tokens agy reused from its cache',
    },
    {
      label: 'Output',
      period: formatTokens(period.output),
      today: formatTokens(today.output),
      hint: 'Tokens the model wrote',
    },
    {
      label: 'Thinking',
      period: formatTokens(period.thinking),
      today: formatTokens(today.thinking),
      hint: 'Tokens the model spent reasoning',
    },
    {
      label: 'Cache hit rate',
      period: percent(cacheRate(period)),
      today: percent(cacheRate(today)),
      hint: 'Cached input as a share of all input',
    },
  ];
}
/** An open day tooltip after new data: the same day stays open with its new totals. */
export function keptDay<T extends { date: string }>(
  open: { index: number; left: number; day: T } | null,
  days: readonly T[],
): { index: number; left: number; day: T } | null {
  const day = open ? days[open.index] : undefined;
  return open && day && day.date === open.day.date ? { ...open, day } : null;
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
/** Background refreshes keep the shown period and stay quiet; a period change shows progress. */
export function createUsage(api: Api, every = 60_000) {
  const state = writable<{
    days: number;
    data: UsageResponse | null;
    loading: boolean;
    error: string;
  }>({ days: 14, data: null, loading: false, error: '' });
  let generation = 0;
  let refreshes = 0;
  let days = 14;
  let timer: ReturnType<typeof setInterval> | undefined;
  let pending: ReturnType<typeof setTimeout> | undefined;
  async function load(next = 14) {
    const own = ++generation;
    days = next;
    state.update((v) => ({ ...v, days: next, loading: true, error: '' }));
    try {
      const data = await api.usage(next, true);
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
  async function refresh() {
    // A period change in progress already reads fresh data.
    if (get(state).loading) return;
    const own = generation,
      mine = ++refreshes;
    try {
      const data = await api.usage(days, true);
      if (own === generation && mine === refreshes)
        state.update((v) => ({ ...v, data, error: '' }));
    } catch {
      // The last reading stays; the next refresh or a period change tries again.
    }
  }
  return {
    state,
    load,
    refresh,
    /** Reads again shortly after usage changed, for example when a turn ended. */
    changed: (delay = 500) => {
      clearTimeout(pending);
      pending = setTimeout(() => void refresh(), delay);
    },
    start: () => {
      clearInterval(timer);
      timer = setInterval(() => {
        if (typeof document === 'undefined' || document.visibilityState !== 'hidden')
          void refresh();
      }, every);
    },
    dispose: () => {
      generation++;
      clearInterval(timer);
      clearTimeout(pending);
    },
  };
}
