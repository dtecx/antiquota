/** Proves quota display and summaries for disabled, missing, wall and attention states. */
import { describe, expect, it } from 'vitest';
import {
  accountStrip,
  accountSummary,
  clockTime,
  minimumQuota,
  modelPool,
  quotaPercent,
  quotaWidth,
  resetDescription,
  wallReason,
} from './account-format';
import { fixtureAccount, fixtureSession } from './fixtures.test-support';
import { routingLine } from './routing-format';
describe('account presentation', () => {
  it('uses the minimum window and preserves missing versus disabled readings', () => {
    expect(minimumQuota(undefined)).toBeNull();
    expect(minimumQuota({})).toBeNull();
    expect(minimumQuota({ short: [0.8, 100], disabled: [0, null] })).toBe(0);
    expect(quotaPercent(null)).toBe('–');
    expect(quotaPercent(0.126)).toBe('13%');
    expect(quotaWidth(0)).toBe(2);
    expect(quotaWidth(2)).toBe(100);
    expect(resetDescription(null)).toContain('Disabled');
    expect(resetDescription(100)).toContain('Resets');
    expect(clockTime(0)).toBe('');
    expect(clockTime(100)).toMatch(/^\d{2}:\d{2}$/);
  });
  it('shows paused, wall, low, attention and serving-account states per pool', () => {
    const one = fixtureAccount(),
      paused = { ...fixtureAccount('two@example.com'), paused: true },
      walled = {
        ...fixtureAccount('three@example.com'),
        walls: { gemini: 1000 },
        why: { gemini: 'signed out' },
      };
    expect(accountStrip(paused, 'gemini').state).toBe('off');
    expect(accountStrip(walled, 'gemini').state).toBe('wall');
    expect(accountStrip({ ...one, quota: { gemini: { '5h': [0.1, null] } } }, 'gemini').state).toBe(
      'low',
    );
    expect(accountSummary([one, paused, walled], one.slot, 'gemini')).toMatchObject({
      available: 1,
      attention: 1,
      detail: 'one · Gemini 80%',
    });
    expect(accountSummary([], null, 'gemini').main).toBe('Add a Google account');
    expect(modelPool('claude-sonnet')).toBe('cg');
    expect(modelPool('gemini-3.8-flash-high')).toBe('gemini');
  });
  it('formats every captured routing kind and the prior account of a pick', () => {
    expect(
      routingLine(
        {
          ts: 1,
          event: 'pick',
          slot: 'two@example.com',
          session: 's',
          previous: 'one@example.com',
        },
        [fixtureSession('s')],
      ).text,
    ).toBe('serves Fixture session (was one)');
    for (const [event, text] of [
      ['wall', 'walled until'],
      ['cooldown', 'rests 1 min'],
      ['retry', 'retry in 5s'],
      ['signed-out', 'sign in again'],
      ['paused', 'paused by you'],
      ['resumed', 'resumed by you'],
    ] as const)
      expect(
        routingLine(
          { ts: 1, event, slot: 'one@example.com', why: 'reason', until: 100, secs: 60, wait: 5 },
          [],
        ).text,
      ).toContain(text);
    expect(wallReason('rate limited, quota left')).toBe('resting');
    expect(wallReason('signed out')).toBe('signed out');
    expect(wallReason('verify')).toBe('verify');
  });
});
