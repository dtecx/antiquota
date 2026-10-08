/** Proves model capacities, context thresholds and selected quota-pool counters. */
import { expect, it } from 'vitest';
import { fixtureAccount, fixtureSession } from './fixtures.test-support';
import { contextCapacity, sessionMeter } from './meter';
it('uses exact capacities and warm/hot context thresholds', () => {
  expect(['claude-4', 'gpt-oss-120b', 'gemini-x'].map(contextCapacity)).toEqual([
    200000, 131072, 1048576,
  ]);
  const s = { ...fixtureSession(), model: 'claude-4', context: 100000 };
  expect(sessionMeter(s, [])).toMatchObject({
    fraction: 0.5,
    heat: 'warm',
    context: '50% context',
  });
  expect(sessionMeter({ ...s, context: 160000 }, []).heat).toBe('hot');
  expect(sessionMeter({ ...s, context: 400000 }, []).fraction).toBe(1);
});
it('uses serving account lowest window and excludes paused/walled/verify accounts', () => {
  const a = {
    ...fixtureAccount(),
    quota: { cg: { five: [0.7, 100], week: [0.2, 200] } },
  } as ReturnType<typeof fixtureAccount>;
  const s = {
    ...fixtureSession(),
    model: 'claude-4',
    last_slot: a.slot,
    usage: { input: 1000, output: 500, thinking: 50 },
  };
  expect(
    sessionMeter(s, [
      a,
      { ...a, slot: 'paused', paused: true },
      { ...a, slot: 'wall', walls: { cg: 123 } },
      { ...a, slot: 'verify', verify: 'url' },
    ]),
  ).toMatchObject({
    available: 1,
    account: 'one · Claude/GPT 20%',
    tokens: '1.0k in / 500 out / 50 thinking',
  });
  expect(sessionMeter({ ...s, last_slot: 'missing' }, [a]).account).toBe('one · Claude/GPT 20%');
});
