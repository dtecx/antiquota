/** Proves ledger statistics and stale-period isolation with exact token totals. */
import { get } from 'svelte/store';
import { describe, expect, it, vi } from 'vitest';
import { createApi } from './api';
import { createUsage, usageSummary, dayAccounts, tooltipLeft } from './usage';
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
