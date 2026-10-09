/** Proves ledger statistics and stale-period isolation with exact token totals. */
import { get } from 'svelte/store';
import { describe, expect, it, vi } from 'vitest';
import { createApi } from './api';
import {
  breakdown,
  cacheRate,
  createUsage,
  dayAccounts,
  partsLine,
  percent,
  tooltipLeft,
  usageSummary,
} from './usage';
import type { UsageResponse } from '../gen/UsageResponse';
const data: UsageResponse = {
  days: [
    { date: '2026-10-07', total: 60, turns: 2, by: { a: 20, b: 40 } },
    { date: '2026-10-08', total: 40, turns: 3, by: { b: 40 } },
  ],
  accounts: {
    a: { tokens: 20, today: 0, turns: 1, input: 15, output: 5 },
    b: { tokens: 80, today: 40, turns: 4, input: 60, output: 20 },
  },
  models: { small: 30, large: 70 },
};
describe('usage chart data', () => {
  it('totals tokens and sorts contributions without rounding the ledger', () => {
    expect(usageSummary(data)).toMatchObject({
      total: 100,
      turns: 5,
      today: { total: 40, turns: 3 },
      peak: 60,
      mean: 20,
    });
    expect(usageSummary(data).accounts.map(([slot]) => slot)).toEqual(['b', 'a']);
    expect(usageSummary(data).models).toEqual([
      ['large', 70],
      ['small', 30],
    ]);
    expect(dayAccounts(data.days[0]!)).toEqual([
      ['b', 40],
      ['a', 20],
    ]);
    expect(usageSummary({ days: [], accounts: {}, models: {} })).toMatchObject({
      total: 0,
      turns: 0,
      peak: 0,
      mean: 0,
      today: { total: 0 },
    });
  });
  it('keeps every tooltip within narrow and wide plots', () => {
    for (const width of [160, 360, 760])
      for (const count of [7, 14, 30])
        for (let index = 0; index < count; index++) {
          const tooltip = Math.min(240, width),
            left = tooltipLeft(index, count, width, tooltip);
          expect(left).toBeGreaterThanOrEqual(0);
          expect(left + tooltip).toBeLessThanOrEqual(width);
        }
  });
  it('ignores stale periods and retains the last reading after a failed refresh', async () => {
    let complete!: (data: UsageResponse) => void;
    const api = {
      ...createApi('fixture', vi.fn<typeof fetch>()),
      usage: vi
        .fn()
        .mockImplementationOnce(
          () =>
            new Promise((resolve) => {
              complete = resolve;
            }),
        )
        .mockResolvedValueOnce(data)
        .mockRejectedValueOnce(new Error('offline')),
    };
    const usage = createUsage(api),
      first = usage.load(7);
    await usage.load(30);
    complete({ days: [], accounts: {}, models: {} });
    await first;
    expect(get(usage.state)).toMatchObject({ days: 30, data, loading: false });
    await usage.load(14);
    expect(get(usage.state)).toMatchObject({ data, error: 'offline', loading: false });
  });
});
describe('usage detail', () => {
  const parts = (input: number, cache: number, output = 0, thinking = 0, turns = 1) => ({
    input,
    output,
    thinking,
    cache,
    turns,
  });
  it('computes the cache hit rate as cached input over all input', () => {
    expect(cacheRate(parts(300, 900))).toBe(0.75);
    expect(cacheRate(parts(0, 0))).toBeNull();
    expect(cacheRate(null)).toBeNull();
    expect(percent(0.754)).toBe('75%');
    expect(percent(null)).toBe('–');
    expect(partsLine(parts(1_500, 20_000, 40, 7))).toBe(
      'input 1.5k · cached input 20.0k · output 40 · thinking 7',
    );
  });
  it('labels each part for the period and today', () => {
    const rows = breakdown({
      period: parts(300, 900, 50, 25, 2),
      today: parts(100, 0, 10, 0, 1),
      days: [],
      accounts: {},
      models: {},
    });
    expect(rows.map((row) => [row.label, row.period, row.today])).toEqual([
      ['Input', '300', '100'],
      ['Cached input', '900', '0'],
      ['Output', '50', '10'],
      ['Thinking', '25', '0'],
      ['Cache hit rate', '75%', '0%'],
    ]);
    expect(rows.every((row) => row.hint)).toBe(true);
  });
  it('refreshes quietly in the background and never cancels a period change', async () => {
    vi.useFakeTimers();
    try {
      let finish!: (data: UsageResponse) => void;
      const fresh = { ...data, days: data.days.slice(1) };
      const api = {
        ...createApi('fixture', vi.fn<typeof fetch>()),
        usage: vi
          .fn()
          .mockResolvedValueOnce(data)
          .mockRejectedValueOnce(new Error('offline'))
          .mockResolvedValueOnce(fresh)
          .mockImplementationOnce(
            () =>
              new Promise((resolve) => {
                finish = resolve;
              }),
          ),
      };
      const usage = createUsage(api, 1_000);
      await usage.load(7);
      expect(api.usage).toHaveBeenLastCalledWith(7, true);
      usage.start();
      // A failed background read keeps the shown data and shows no error.
      await vi.advanceTimersByTimeAsync(1_000);
      expect(get(usage.state)).toMatchObject({ data, error: '', loading: false });
      // A finished turn reads again shortly after.
      usage.changed(200);
      await vi.advanceTimersByTimeAsync(200);
      expect(get(usage.state).data).toBe(fresh);
      expect(api.usage).toHaveBeenLastCalledWith(7, true);
      // While a period change loads, background reads wait for it.
      const change = usage.load(30);
      await vi.advanceTimersByTimeAsync(1_000);
      expect(api.usage).toHaveBeenCalledTimes(4);
      finish(data);
      await change;
      expect(get(usage.state)).toMatchObject({ days: 30, data, loading: false });
      usage.dispose();
      await vi.advanceTimersByTimeAsync(5_000);
      expect(api.usage).toHaveBeenCalledTimes(4);
    } finally {
      vi.useRealTimers();
    }
  });
});
