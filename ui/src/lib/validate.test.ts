/** Proves account SSE fields are checked before they reach reactive views. */
import { describe, expect, it } from 'vitest';
import { fixtureAccount } from './fixtures.test-support';
import { isAccount } from './validate';
import { parseStreamEvent } from './transcript';
describe('account stream validation', () => {
  it('accepts unprobed accounts, disabled windows and missing today counters', () => {
    const account = {
      ...fixtureAccount(),
      probed: null,
      today: {},
      quota: { gemini: { weekly: [0, null] } },
    };
    expect(isAccount(account)).toBe(true);
    expect(
      parseStreamEvent(JSON.stringify({ t: 'accounts', ts: 1, sid: null, accounts: [account] })),
    ).toMatchObject({ accounts: [account] });
  });
  it('rejects malformed known account fields instead of crashing a view', () => {
    for (const patch of [
      { slot: 1 },
      { paused: 'false' },
      { quota: { gemini: { '5h': ['90', 100] } } },
      { quota: { gemini: { '5h': [0.9] } } },
      { walls: { gemini: 'until' } },
      { why: { gemini: 1 } },
      { today: { turns: 'one' } },
      { verify: { url: 'https://example.invalid' } },
    ]) {
      expect(
        parseStreamEvent(
          JSON.stringify({
            t: 'accounts',
            ts: 1,
            sid: null,
            accounts: [{ ...fixtureAccount(), ...patch }],
          }),
        ),
      ).toBeNull();
    }
  });
});
