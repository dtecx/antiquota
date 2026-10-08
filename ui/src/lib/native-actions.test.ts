/** Proves native menu dispatch, captured session ownership and preserved composer drafts. */
import { get } from 'svelte/store';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import type { EventRecord } from '../gen/EventRecord';
import type { SessionResponse } from '../gen/SessionResponse';
import { createApi, type Api } from './api';
import { shortcutsOpen } from './commands';
import { downloadSession } from './export';
import { fixtureSession, fixtureState } from './fixtures.test-support';
import { drawer, viewer } from './git';
import { dispatchNative } from './native-actions';
import { createPanes } from './panes';
import { preferences } from './preferences';
import { createWorkbench } from './workbench';

vi.mock('./export', () => ({ downloadSession: vi.fn(async () => {}) }));
const dispose: (() => void)[] = [];
const reload = vi.fn();
const getElementById = vi.fn();
beforeEach(() => {
  getElementById.mockReset();
  preferences.change({ zoom: 1, split: null });
  shortcutsOpen.set(false);
  drawer.set({ open: false, tab: 'files' });
  viewer.set(null);
  vi.stubGlobal('document', { getElementById });
  vi.stubGlobal('location', { reload });
});
afterEach(() => {
  for (const stop of dispose.splice(0)) stop();
  viewer.set(null);
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});
async function setup() {
  const sessions = [fixtureSession('a'), { ...fixtureSession('b'), mode: 'plan' as const }];
  const api: Api = {
    ...createApi('synthetic', vi.fn<typeof fetch>()),
    state: vi.fn(async () => fixtureState(sessions)),
    session: vi.fn(async (id) => ({ meta: sessions.find((s) => s.id === id)!, events: [] })),
    engineLog: vi.fn(async () => ({ text: 'Synthetic engine log\n' })),
  };
  const left = createWorkbench(api),
    panes = createPanes(api, left);
  dispose.push(panes.dispose);
  await left.refresh('a');
  await panes.open('b');
  left.draft('Unsubmitted left draft');
  panes.right.draft('Unsubmitted right draft');
  const host = {
    newSession: vi.fn(() => {
      panes.focus('left');
      left.openDialog('create');
    }),
    settings: vi.fn(),
    find: vi.fn(),
    split: vi.fn(),
    step: vi.fn(),
    select: vi.fn(async () => {}),
  };
  vi.mocked(api.session).mockClear();
  return { api, left, panes, host };
}

it.each([
  ['settings', 'settings', 'general'],
  ['usage', 'settings', 'usage'],
  ['find', 'find', undefined],
  ['split', 'split', undefined],
  ['prev_session', 'step', -1],
  ['next_session', 'step', 1],
] as const)(
  '%s dispatches its host action without altering either draft',
  async (id, method, arg) => {
    const { api, left, panes, host } = await setup();
    const drafts = get(left.state).drafts;
    await dispatchNative(id, api, panes.right, host);
    if (arg === undefined) expect(host[method]).toHaveBeenCalledExactlyOnceWith();
    else expect(host[method]).toHaveBeenCalledExactlyOnceWith(arg);
    expect(get(left.state).drafts).toBe(drafts);
    expect(get(panes.right.state).drafts).toBe(drafts);
    expect(api.session).not.toHaveBeenCalled();
  },
);

it('delegates New session to the primary owner even while the right pane is active', async () => {
  const { api, left, panes, host } = await setup();
  expect(get(panes.state).active).toBe('right');
  await dispatchNative('new', api, panes.right, host);
  expect(host.newSession).toHaveBeenCalledExactlyOnceWith();
  expect(get(panes.state).active).toBe('left');
  expect(get(left.state).dialog).toMatchObject({ kind: 'create', mode: 'default' });
  expect(get(panes.right.state).dialog).toBeNull();
  expect(get(left.state).drafts).toEqual({
    a: 'Unsubmitted left draft',
    b: 'Unsubmitted right draft',
  });
});

it.each([
  ['zoom_in', 1.1],
  ['zoom_out', 0.9],
  ['zoom_reset', 1],
] as const)('%s changes only the normalized device zoom', async (id, expected) => {
  const { api, left, host } = await setup();
  if (id === 'zoom_reset') preferences.change({ zoom: 1.4 });
  const drafts = get(left.state).drafts;
  await dispatchNative(id, api, left, host);
  expect(get(preferences.state).zoom).toBe(expected);
  expect(get(left.state).drafts).toBe(drafts);
  expect(api.session).not.toHaveBeenCalled();
});

it('toggles the side panel while retaining its chosen tab and opens the shortcut reference', async () => {
  const { api, left, host } = await setup();
  await dispatchNative('panel', api, left, host);
  expect(get(drawer)).toEqual({ open: true, tab: 'files' });
  await dispatchNative('panel', api, left, host);
  expect(get(drawer)).toEqual({ open: false, tab: 'files' });
  await dispatchNative('shortcuts', api, left, host);
  expect(get(shortcutsOpen)).toBe(true);
});

it('reloads through the browser location without submitting the current draft', async () => {
  const { api, left, host } = await setup();
  const drafts = get(left.state).drafts;
  await dispatchNative('reload', api, left, host);
  expect(reload).toHaveBeenCalledExactlyOnceWith();
  expect(get(left.state).drafts).toBe(drafts);
  expect(api.session).not.toHaveBeenCalled();
});

it.each([
  ['dictate', 'button.mic:not(:disabled)'],
  ['expand', '.presentation button:last-child'],
] as const)('%s activates only the button in the initiating session pane', async (id, selector) => {
  const { api, left, panes, host } = await setup();
  const leftClick = vi.fn(),
    rightClick = vi.fn();
  const leftQuery = vi.fn(() => ({ click: leftClick }));
  const rightQuery = vi.fn(() => ({ click: rightClick }));
  getElementById.mockImplementation((id: string) => ({
    closest: () => ({ querySelector: id === 'prompt-b' ? rightQuery : leftQuery }),
  }));
  await dispatchNative(id, api, panes.right, host);
  expect(getElementById).toHaveBeenLastCalledWith('prompt-b');
  expect(rightQuery).toHaveBeenCalledExactlyOnceWith(selector);
  expect(rightClick).toHaveBeenCalledOnce();
  expect(leftQuery).not.toHaveBeenCalled();
  await dispatchNative(id, api, left, host);
  expect(getElementById).toHaveBeenLastCalledWith('prompt-a');
  expect(leftQuery).toHaveBeenCalledExactlyOnceWith(selector);
  expect(leftClick).toHaveBeenCalledOnce();
});

it('ignores pane actions when no session or enabled pane button exists', async () => {
  const { api, left, host } = await setup();
  getElementById.mockReturnValue({ closest: () => ({ querySelector: () => null }) });
  await dispatchNative('dictate', api, left, host);
  left.clearSelection();
  getElementById.mockClear();
  for (const id of ['dictate', 'expand', 'export']) await dispatchNative(id, api, left, host);
  expect(getElementById).not.toHaveBeenCalled();
  expect(api.session).not.toHaveBeenCalled();
  expect(downloadSession).not.toHaveBeenCalled();
});

it('exports fresh durable data from the captured active session despite a later pane switch', async () => {
  const { api, left, panes, host } = await setup();
  const events: EventRecord[] = [
    { t: 'user', ts: 1, kind: 'message', text: 'Durable prompt' },
    { t: 'assistant', ts: 2, turn: 1, step: 1, text: 'Durable completed reply' },
  ];
  const meta = { ...fixtureSession('b'), title: 'Authoritative durable title' };
  let finish!: (value: SessionResponse) => void;
  api.session = vi.fn(
    () =>
      new Promise<SessionResponse>((resolve) => {
        finish = resolve;
      }),
  );
  const drafts = get(left.state).drafts;
  const pending = dispatchNative('export', api, panes.right, host);
  expect(api.session).toHaveBeenCalledExactlyOnceWith('b');
  panes.close();
  finish({ meta, events });
  await pending;
  expect(downloadSession).toHaveBeenCalledExactlyOnceWith(meta, events);
  expect(get(left.state).drafts).toBe(drafts);
  expect(get(panes.right.state).drafts).toBe(drafts);
  expect(get(left.state).selected?.id).toBe('a');
});

it('propagates export read and save failures while preserving drafts', async () => {
  const { api, left, host } = await setup();
  const drafts = get(left.state).drafts;
  vi.mocked(api.session).mockRejectedValueOnce(new Error('durable read refused'));
  await expect(dispatchNative('export', api, left, host)).rejects.toThrow('durable read refused');
  expect(downloadSession).not.toHaveBeenCalled();
  vi.mocked(downloadSession).mockRejectedValueOnce(new Error('native save refused'));
  await expect(dispatchNative('export', api, left, host)).rejects.toThrow('native save refused');
  expect(get(left.state).drafts).toBe(drafts);
});

it('opens the exact engine log without changing session or draft', async () => {
  const { api, left, host } = await setup();
  const drafts = get(left.state).drafts;
  await dispatchNative('engine_log', api, left, host);
  expect(api.engineLog).toHaveBeenCalledExactlyOnceWith();
  expect(get(viewer)).toEqual({
    rel: 'engine log',
    path: 'engine.log',
    text: 'Synthetic engine log\n',
  });
  expect(get(left.state).selected?.id).toBe('a');
  expect(get(left.state).drafts).toBe(drafts);
});

it('does not let a late engine log overwrite a newer file viewer', async () => {
  const { api, left, host } = await setup();
  let finish!: (value: { text: string }) => void;
  api.engineLog = vi.fn(
    () =>
      new Promise<{ text: string }>((resolve) => {
        finish = resolve;
      }),
  );
  const pending = dispatchNative('engine_log', api, left, host);
  const newer = { rel: 'newer.txt', path: '/Desktop/test/newer.txt', text: 'Newer viewer' };
  viewer.set(newer);
  finish({ text: 'Old pending log' });
  await pending;
  expect(get(viewer)).toBe(newer);
});

it('forwards a valid tray session target exactly without touching either draft', async () => {
  const { api, left, panes, host } = await setup();
  const drafts = get(left.state).drafts;
  await dispatchNative('session:a', api, panes.right, host);
  expect(host.select).toHaveBeenCalledExactlyOnceWith('a');
  expect(get(left.state).drafts).toBe(drafts);
  expect(get(panes.right.state).drafts).toBe(drafts);
});

it('ignores unknown, shell-owned and empty session action IDs', async () => {
  const { api, left, host } = await setup();
  const initial = get(left.state),
    prefs = get(preferences.state);
  for (const id of ['unknown', 'quit', 'tray:new', 'openNew()', 'session:', 'session:   ', ''])
    await dispatchNative(id, api, left, host);
  for (const action of Object.values(host)) expect(action).not.toHaveBeenCalled();
  expect(get(left.state)).toBe(initial);
  expect(get(preferences.state)).toBe(prefs);
  expect(api.session).not.toHaveBeenCalled();
  expect(downloadSession).not.toHaveBeenCalled();
});
