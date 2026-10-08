/** Proves goal bar text, plan readiness and goal drafts. */
import { describe, expect, it } from 'vitest';
import type { EventRecord } from '../gen/EventRecord';
import type { Goal } from '../gen/Goal';
import type { Session } from '../gen/Session';
import { draftFor, goalMeta, goalRequest, planReady } from './goals';

const goal: Goal = {
  objective: 'Ship it',
  check: 'npm test',
  verifier: true,
  verifier_model: '',
  max_rounds: 20,
  round: 3,
  state: 'active',
};
const session = (patch: Partial<Session>) =>
  ({ mode: 'plan', status: 'idle', ...patch }) as Session;
const end = (status: string): EventRecord => ({
  t: 'turn_end',
  ts: 2,
  turn: 1,
  slot: 'a',
  status,
  usage: {
    input_tokens: 0,
    output_tokens: 0,
    thinking_tokens: 0,
    total_tokens: 0,
    context: 0,
  },
  secs: 1,
});
const user: EventRecord = { t: 'user', ts: 1, kind: 'user', text: 'Plan it.' };

describe('goal bar', () => {
  it('shows rounds while active and the state otherwise', () => {
    expect(goalMeta(goal, 'flash')).toBe('round 3/20  ·  check: npm test  ·  verifier: flash');
    expect(goalMeta({ ...goal, state: 'met', check: '', verifier_model: 'pro' }, 'flash')).toBe(
      'met  ·  verifier: pro',
    );
    expect(goalMeta({ ...goal, state: 'paused', check: '', verifier: false }, 'flash')).toBe(
      'paused',
    );
  });
});

describe('plan bar', () => {
  it('appears after a successful plan-mode turn that follows the last message', () => {
    expect(planReady(session({}), [user, end('SUCCESS')])).toBe(true);
    expect(planReady(session({}), [user, end('quota')])).toBe(false);
    expect(planReady(session({}), [end('SUCCESS'), user])).toBe(false);
    expect(planReady(session({ status: 'running' }), [user, end('SUCCESS')])).toBe(false);
    expect(planReady(session({ mode: 'default' }), [user, end('SUCCESS')])).toBe(false);
    expect(planReady(null, [])).toBe(false);
  });
});

describe('goal drafts', () => {
  it('reuse the existing goal or the composer text', () => {
    expect(draftFor(goal, 'typed', 'v').objective).toBe('Ship it');
    expect(draftFor({ ...goal, verifier: false, max_rounds: 37 }, 'typed', 'v')).toMatchObject({
      verifier: false,
      max_rounds: '37',
    });
    const fresh = draftFor(null, '  Write docs  ', 'gemini-3.8-flash-high');
    expect(fresh).toMatchObject({
      objective: 'Write docs',
      verifier: true,
      verifier_model: 'gemini-3.8-flash-high',
      max_rounds: '20',
    });
    expect(goalRequest(fresh)).toEqual({
      objective: 'Write docs',
      check: '',
      verifier: true,
      verifier_model: 'gemini-3.8-flash-high',
      max_rounds: 20,
    });
    expect(goalRequest({ ...fresh, objective: ' ' })).toBe('Describe the objective first.');
    expect(goalRequest({ ...fresh, max_rounds: '0' })).toContain('1 to 500');
    expect(goalRequest({ ...fresh, max_rounds: '2.5' })).toContain('whole number');
  });
});
