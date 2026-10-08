/** Proves notification choices and permission errors cannot interrupt app work. */
import { expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import { createNotifications } from './notifications';
import { createPreferences } from './preferences';
import type { EventRecord } from '../gen/EventRecord';
it('suppresses focused, disabled and ungranted notices and tolerates browser errors', async () => {
  const browser = {
    visible: () => true,
    focused: () => true,
    permission: vi.fn<() => NotificationPermission>(() => 'granted'),
    request: vi.fn(async () => 'granted' as const),
    send: vi.fn(),
  };
  const notices = createNotifications(browser),
    prefs = get(createPreferences().state);
  notices.send('done', '', 'turn', prefs);
  expect(browser.send).not.toHaveBeenCalled();
  notices.send('done', '', 'turn', { ...prefs, focused: true });
  expect(browser.send).toHaveBeenCalledOnce();
  notices.send('done', '', 'turn', {
    ...prefs,
    focused: true,
    notify: { ...prefs.notify, turn: false },
  });
  expect(browser.send).toHaveBeenCalledOnce();
  browser.permission.mockReturnValue('denied');
  notices.send('done', '', 'turn', { ...prefs, focused: true });
  expect(browser.send).toHaveBeenCalledOnce();
  await notices.request();
  expect(browser.request).not.toHaveBeenCalled();
  browser.permission.mockReturnValue('default');
  await notices.request();
  expect(browser.request).toHaveBeenCalledOnce();
  browser.request.mockRejectedValueOnce(new Error('blocked'));
  await expect(notices.request()).resolves.toBeUndefined();
  browser.permission.mockReturnValue('granted');
  browser.send.mockImplementationOnce(() => {
    throw new Error('unavailable');
  });
  expect(() => notices.send('done', '', 'turn', { ...prefs, focused: true })).not.toThrow();
});
it('routes each live notice to its independent preference and skips resolved approvals', () => {
  const send = vi.fn(),
    notices = createNotifications({
      visible: () => false,
      focused: () => false,
      permission: () => 'granted',
      request: async () => 'granted',
      send,
    }),
    prefs = get(createPreferences().state);
  const events: EventRecord[] = [
    {
      t: 'turn_end',
      ts: 1,
      turn: 1,
      slot: 'slot',
      status: 'SUCCESS',
      secs: 1,
      usage: { input_tokens: 1, output_tokens: 1, thinking_tokens: 0, total_tokens: 2, context: 0 },
    },
    {
      t: 'approval',
      ts: 1,
      id: 'a',
      session: 's',
      tool: 'run_command',
      args: {},
      rule: '',
      state: 'pending',
      why: '',
      deadline: 2,
    },
    { t: 'job', ts: 1, phase: 'ended', job: {} },
    { t: 'notice', ts: 1, kind: 'goal', text: '', data: { met: true } },
    { t: 'notice', ts: 1, kind: 'wall', text: 'spent', data: {} },
  ];
  for (const event of events) notices.event(event, 'Session', prefs);
  expect(send.mock.calls.map(([title]) => title)).toEqual([
    'Session: turn 1 SUCCESS',
    'Approval needed',
    'Job finished',
    'Goal met',
    'Out of quota',
  ]);
  const disabled = {
    ...prefs,
    notify: { turn: false, approval: false, job: false, goal: false, quota: false },
  };
  for (const event of events) notices.event(event, 'Session', disabled);
  notices.event(
    {
      ...events[1]!,
      t: 'approval',
      id: 'a',
      session: 's',
      tool: 'run_command',
      args: {},
      rule: '',
      state: 'allowed',
      why: '',
      deadline: 2,
    },
    'Session',
    prefs,
  );
  expect(send).toHaveBeenCalledTimes(5);
});
it('suppresses quota turn endings and false or malformed goal verdicts while allowing exhausted-quota notices', () => {
  const send = vi.fn(),
    notices = createNotifications({
      visible: () => false,
      focused: () => false,
      permission: () => 'granted',
      request: async () => 'granted',
      send,
    }),
    prefs = get(createPreferences().state);
  notices.event(
    {
      t: 'turn_end',
      ts: 1,
      turn: 1,
      slot: 'test',
      status: 'quota',
      secs: 1,
      usage: { input_tokens: 0, output_tokens: 0, thinking_tokens: 0, total_tokens: 0, context: 0 },
    },
    'Test',
    prefs,
  );
  for (const met of [false, 'false', 1, null])
    notices.event({ t: 'notice', ts: 1, kind: 'goal', text: '', data: { met } }, 'Test', prefs);
  expect(send).not.toHaveBeenCalled();
  notices.event(
    { t: 'notice', ts: 1, kind: 'wall', text: 'All accounts exhausted.', data: {} },
    'Test',
    prefs,
  );
  expect(send).toHaveBeenCalledWith('Out of quota', 'All accounts exhausted.');
});
