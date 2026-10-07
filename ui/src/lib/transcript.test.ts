/** Proves replay and live updates produce stable tool/reply groups. */
import { describe, expect, it } from 'vitest';
import type { EventRecord } from '../gen/EventRecord';
import { groupTranscript, parseStreamEvent, projectHistory, reduceTranscript } from './transcript';
describe('event projection', () => {
  it('replaces a streamed assistant step with its authoritative final text', () => {
    let events: EventRecord[] = [];
    events = reduceTranscript(events, { t: 'delta', ts: 1, turn: 1, step: 2, text: 'hel' });
    events = reduceTranscript(events, { t: 'delta', ts: 2, turn: 1, step: 2, text: 'lo' });
    expect(events).toEqual([{ t: 'delta', ts: 2, turn: 1, step: 2, text: 'hello' }]);
    events = reduceTranscript(events, { t: 'assistant', ts: 3, turn: 1, step: 2, text: 'Hello.' });
    expect(events).toEqual([{ t: 'assistant', ts: 3, turn: 1, step: 2, text: 'Hello.' }]);
  });
  it('updates tools in place and keeps separate turns and steps', () => {
    const tool: EventRecord = {
      t: 'tool',
      ts: 1,
      turn: 1,
      step: 3,
      name: 'read_file',
      params: { path: 'a' },
      state: 'ACTIVE',
    };
    let events = reduceTranscript([], tool);
    events = reduceTranscript(events, { ...tool, ts: 2, state: 'DONE', output: 'contents' });
    events = reduceTranscript(events, { ...tool, ts: 3, turn: 2 });
    expect(events).toHaveLength(2);
    expect(events[0]).toMatchObject({ state: 'DONE', output: 'contents' });
  });
  it('deduplicates a replayed durable event without losing equal-time distinct events', () => {
    const event: EventRecord = { t: 'notice', ts: 1, kind: 'info', text: 'hello', data: null };
    const events = reduceTranscript(reduceTranscript([], event), event);
    expect(events).toHaveLength(1);
    expect(reduceTranscript(events, { ...event, text: 'different' })).toHaveLength(2);
  });
  it('supports history before a user message and optional steering attachments', () => {
    const groups = groupTranscript([
      { t: 'assistant', ts: 1, turn: 1, step: 1, text: 'old history' },
      { t: 'user', ts: 2, kind: 'steer', text: 'next' },
      { t: 'thinking', ts: 3, turn: 2, step: 1, text: 'considering', tokens: 2 },
    ]);
    expect(groups).toHaveLength(2);
    expect(groups[0]?.user).toBeNull();
    expect(groups[1]?.user?.text).toBe('next');
    expect(groups[1]?.events[0]?.t).toBe('thinking');
  });
  it('accepts Rust discriminants and rejects malformed or unknown envelopes', () => {
    expect(parseStreamEvent('{"sid":"a","ts":1,"t":"deleted"}')?.t).toBe('deleted');
    expect(parseStreamEvent('{"sid":"a","ts":"bad","t":"deleted"}')).toBeNull();
    expect(parseStreamEvent('{"sid":"a","ts":1,"t":"future_event"}')).toBeNull();
    expect(parseStreamEvent('{}')).toBeNull();
    expect(parseStreamEvent('{"sid":"a","ts":1,"t":"meta"}')).toBeNull();
    expect(
      parseStreamEvent(
        '{"sid":"a","ts":1,"t":"tool","turn":1,"step":2,"name":null,"state":"ACTIVE","params":{}}',
      ),
    ).toBeNull();
    expect(
      parseStreamEvent(
        '{"sid":"a","ts":1,"t":"turn_end","turn":1,"slot":"a","status":"success","secs":1,"usage":null}',
      ),
    ).toBeNull();
    expect(
      parseStreamEvent(
        '{"sid":"a","ts":1,"t":"user","kind":"message","text":"hello","attachments":[null]}',
      ),
    ).toBeNull();
  });
  it('drops obsolete buffered deltas after replay and removes matching fragments on final', () => {
    const final: EventRecord = { t: 'assistant', ts: 4, turn: 1, step: 2, text: 'Final' };
    const delta: EventRecord = { t: 'delta', ts: 2, turn: 1, step: 2, text: 'Old' };
    expect(reduceTranscript([final], delta)).toEqual([final]);
    expect(reduceTranscript([delta, { ...delta, ts: 3 }], final)).toEqual([final]);
  });
  it('projects repeated durable tool records once and ignores an older buffered active frame', () => {
    const active: EventRecord = {
      t: 'tool',
      ts: 2,
      turn: 1,
      step: 3,
      name: 'read_file',
      params: {},
      state: 'active',
    };
    const done: EventRecord = { ...active, ts: 3, state: 'done', output: 'done' };
    expect(projectHistory([active, done])).toEqual([done]);
    expect(reduceTranscript([done], active)).toEqual([done]);
  });
  it('accepts a final reply even when the wall clock moved backwards', () => {
    const delta: EventRecord = { t: 'delta', ts: 20, turn: 1, step: 2, text: 'Partial' };
    const final: EventRecord = { t: 'assistant', ts: 10, turn: 1, step: 2, text: 'Final' };
    expect(reduceTranscript([delta], final)).toEqual([final]);
  });
  it('preserves identical durable user occurrences even with the same timestamp', () => {
    const user: EventRecord = { t: 'user', ts: 10, text: 'Repeat this message.', kind: 'user' };
    const projected = projectHistory([user, { ...user }]);
    expect(projected).toHaveLength(2);
    expect(groupTranscript(projected)).toHaveLength(2);
  });
});
