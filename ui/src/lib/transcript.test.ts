/** Proves replay and live updates produce stable tool/reply groups. */
import { describe, expect, it } from 'vitest';
import type { EventRecord } from '../gen/EventRecord';
import {
  groupTranscript,
  nearBottom,
  parseStreamEvent,
  projectHistory,
  reduceTranscript,
  transcriptItems,
  userPresentation,
} from './transcript';

function ended(turn: number, status = 'success'): Extract<EventRecord, { t: 'turn_end' }> {
  return {
    t: 'turn_end',
    ts: 10,
    turn,
    slot: 'slot',
    status,
    secs: 1,
    usage: { input_tokens: 1, output_tokens: 2, thinking_tokens: 0, total_tokens: 3, context: 3 },
  };
}
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
  it.each(['success', 'error', 'stopped'])(
    'finalizes every partial step on %s without moving it',
    (status) => {
      const events: EventRecord[] = [
        { t: 'delta', ts: 1, turn: 1, step: 1, text: 'First partial' },
        { t: 'notice', ts: 2, kind: 'info', text: 'Between replies', data: null },
        { t: 'delta', ts: 3, turn: 1, step: 2, text: 'Second partial' },
        { t: 'delta', ts: 4, turn: 2, step: 1, text: 'Other turn' },
      ];
      const end = ended(1, status);
      const expected: EventRecord[] = [
        { ...events[0], t: 'assistant' } as EventRecord,
        events[1]!,
        { ...events[2], t: 'assistant' } as EventRecord,
        events[3]!,
        end,
      ];
      const live = reduceTranscript(events, end);
      expect(live).toEqual(expected);
      expect(projectHistory([...events, end])).toEqual(expected);
      expect(events[0]?.t).toBe('delta');
      expect(
        reduceTranscript(live, { t: 'delta', ts: 11, turn: 1, step: 99, text: 'Late' }),
      ).toEqual(live);
      const authoritative: EventRecord = {
        t: 'assistant',
        ts: 9,
        turn: 1,
        step: 1,
        text: 'Final answer',
      };
      expect(reduceTranscript(live, authoritative)).toEqual([authoritative, ...expected.slice(1)]);
    },
  );
  it('keeps identical durable turn ends and notices distinct while finalizing partial replies', () => {
    const end = ended(1);
    const notice: EventRecord = { t: 'notice', ts: 10, kind: 'info', text: 'Repeated', data: null };
    const events = projectHistory([
      { t: 'delta', ts: 1, turn: 1, step: 1, text: 'Partial' },
      end,
      { ...end },
      notice,
      { ...notice },
      { t: 'delta', ts: 12, turn: 1, step: 2, text: 'Late' },
    ]);
    expect(events.map((event) => event.t)).toEqual([
      'assistant',
      'turn_end',
      'turn_end',
      'notice',
      'notice',
    ]);
    const items = transcriptItems(events);
    expect(new Set(items.map((item) => item.key)).size).toBe(5);
  });
});

describe('chronological transcript presentation', () => {
  const tool: Extract<EventRecord, { t: 'tool' }> = {
    t: 'tool',
    ts: 1,
    turn: 1,
    step: 1,
    name: 'view_file',
    params: {},
    state: 'running',
  };
  it('groups only adjacent tools and leaves thinking, replies, notices and turn boundaries in place', () => {
    const events: EventRecord[] = [
      tool,
      { ...tool, step: 2 },
      { t: 'thinking', ts: 2, turn: 1, step: 3, text: 'Think', tokens: 1 },
      { ...tool, step: 4 },
      { t: 'assistant', ts: 3, turn: 1, step: 5, text: 'Reply' },
      { ...tool, step: 6 },
      { t: 'notice', ts: 4, kind: 'info', text: 'Notice', data: null },
      { ...tool, step: 7 },
      ended(1),
    ];
    const items = transcriptItems(events);
    expect(
      items.map((item) =>
        'events' in item ? item.events.map((event) => event.step) : item.event.t,
      ),
    ).toEqual([[1, 2], 'thinking', [4], 'assistant', [6], 'notice', [7], 'turn_end']);
    expect(items.flatMap((item) => ('events' in item ? item.events : [item.event]))).toEqual(
      events,
    );
  });
  it('keeps tool group and reply keys stable across completion and authoritative replacements', () => {
    const delta: EventRecord = { t: 'delta', ts: 2, turn: 1, step: 2, text: 'Part' };
    const first = transcriptItems([tool, delta]);
    const completed = reduceTranscript([tool, delta], { ...tool, ts: 3, state: 'done' });
    const final = reduceTranscript(completed, {
      t: 'assistant',
      ts: 4,
      turn: 1,
      step: 2,
      text: 'Final',
    });
    expect(first.map((item) => item.key)).toEqual(['tools-1-1', 'reply-1-2']);
    expect(transcriptItems(final).map((item) => item.key)).toEqual(first.map((item) => item.key));
    expect(transcriptItems([tool, { ...tool, step: 3 }])[0]?.key).toBe('tools-1-1');
    expect(transcriptItems([{ ...tool, turn: 2 }])[0]?.key).toBe('tools-2-1');
  });
  it.each([
    ['user', 'YOU'],
    ['message', 'YOU'],
    ['goal', 'GOAL'],
    ['nudge', 'GOAL PUSH'],
    ['job', 'JOB'],
    ['loop', 'LOOP'],
    ['future', 'YOU'],
    ['__proto__', 'YOU'],
  ])('labels %s prompts as %s', (kind, label) => {
    expect(
      userPresentation({ t: 'user', ts: 1, kind, text: 'Prompt', steered: true }),
    ).toMatchObject({ label, text: 'Prompt' });
  });
  it('shows the goal objective while retaining standing instructions in its tooltip', () => {
    const text = 'Objective: Ship the change\nwith tests\n\nWork autonomously until finished.';
    expect(userPresentation({ t: 'user', ts: 1, kind: 'goal', text })).toEqual({
      label: 'GOAL',
      text: 'Ship the change\nwith tests',
      tooltip: text,
    });
    expect(
      userPresentation({
        t: 'user',
        ts: 1,
        kind: 'goal',
        text: 'Objective: Ship it\r\n\r\nInstructions',
      }).text,
    ).toBe('Ship it');
    expect(userPresentation({ t: 'user', ts: 1, kind: 'message', text })).toEqual({
      label: 'YOU',
      text,
    });
  });
  it('follows the bottom at the inclusive 120 pixel boundary', () => {
    expect(nearBottom(119)).toBe(true);
    expect(nearBottom(120)).toBe(true);
    expect(nearBottom(121)).toBe(false);
  });
});
