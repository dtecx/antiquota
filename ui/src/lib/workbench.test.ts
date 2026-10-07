/** Proves drafts, selection races, queue feedback, and reconnect reconciliation. */
import { get } from 'svelte/store';
import { describe, expect, it, vi } from 'vitest';
import type { Api } from './api';
import type { SessionResponse } from '../gen/SessionResponse';
import { fixtureSession, fixtureState } from './fixtures.test-support';
import { createWorkbench } from './workbench';
function fixtureApi(): Api {
  return {
    state: vi.fn(async () => fixtureState()),
    session: vi.fn(async (id) => ({ meta: fixtureSession(id), events: [] })),
    create: vi.fn(async () => fixtureSession('new')),
    update: vi.fn(async (id, patch) => ({ ...fixtureSession(id), ...patch })),
    send: vi.fn(async () => ({ result: 'started' })),
    stop: vi.fn(async () => ({ ok: true })),
    streamUrl: () => '/api/events?token=synthetic',
  };
}
describe('session workbench store', () => {
  it('keeps a failed-send draft and clears it only after successful queue acceptance', async () => {
    const api = fixtureApi();
    const send = vi
      .fn<Api['send']>()
      .mockRejectedValueOnce(new Error('offline'))
      .mockResolvedValueOnce({ result: 'queued' });
    api.send = send;
    const workbench = createWorkbench(api);
    await workbench.refresh();
    workbench.draft('Do this next');
    await workbench.send();
    expect(get(workbench.state).drafts.fixture).toBe('Do this next');
    expect(get(workbench.state).error).toBe('offline');
    await workbench.send();
    expect(get(workbench.state).drafts.fixture).toBe('');
    expect(get(workbench.state).feedback).toBe('Message queued for the next turn.');
  });
  it('preserves a newer draft typed while sending the earlier one', async () => {
    const api = fixtureApi();
    let complete: (value: { result: string }) => void = () => {};
    api.send = () =>
      new Promise((resolve) => {
        complete = resolve;
      });
    const workbench = createWorkbench(api);
    await workbench.refresh();
    workbench.draft('earlier');
    const pending = workbench.send();
    workbench.draft('newer');
    complete({ result: 'started' });
    await pending;
    expect(get(workbench.state).drafts.fixture).toBe('newer');
  });
  it('ignores an old selection response after choosing another session', async () => {
    const api = fixtureApi();
    const completions = new Map<string, (value: SessionResponse) => void>();
    api.session = (id) =>
      new Promise((resolve) => {
        completions.set(id, resolve);
      });
    const workbench = createWorkbench(api);
    const first = workbench.select('first');
    const second = workbench.select('second');
    completions.get('second')?.({ meta: fixtureSession('second'), events: [] });
    await second;
    completions.get('first')?.({ meta: fixtureSession('first'), events: [] });
    await first;
    expect(get(workbench.state).selected?.id).toBe('second');
  });
  it('replays frames arriving while the authoritative snapshot is loading', async () => {
    const api = fixtureApi();
    const workbench = createWorkbench(api);
    await workbench.refresh();
    let complete: (value: SessionResponse) => void = () => {};
    api.session = () =>
      new Promise((resolve) => {
        complete = resolve;
      });
    const pending = workbench.refresh();
    await Promise.resolve();
    workbench.onEvent({ sid: 'fixture', ts: 4, t: 'delta', turn: 1, step: 2, text: 'live reply' });
    complete({ meta: fixtureSession(), events: [] });
    await pending;
    expect(get(workbench.state).events).toEqual([
      { ts: 4, t: 'delta', turn: 1, step: 2, text: 'live reply' },
    ]);
  });
  it('uses the runtime default folder and keeps validation failures in the dialog', async () => {
    const api = fixtureApi();
    api.create = vi.fn(async () => {
      throw new Error('Folder missing');
    });
    const workbench = createWorkbench(api);
    await workbench.refresh();
    workbench.openDialog('create');
    expect(get(workbench.state).dialog?.cwd).toBe('/Desktop/test/project');
    workbench.dialogField('cwd', '/missing');
    await workbench.saveDialog();
    expect(get(workbench.state).dialog?.cwd).toBe('/missing');
    expect(get(workbench.state).error).toBe('Folder missing');
  });
  it('retains a newly created session target when another refresh is already in flight', async () => {
    const api = fixtureApi();
    const workbench = createWorkbench(api);
    await workbench.refresh();
    let complete: (value: ReturnType<typeof fixtureState>) => void = () => {};
    api.state = vi
      .fn<Api['state']>()
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            complete = resolve;
          }),
      )
      .mockResolvedValueOnce(fixtureState([fixtureSession(), fixtureSession('new')]));
    const pending = workbench.refresh();
    workbench.openDialog('create');
    await workbench.saveDialog();
    complete(fixtureState());
    await pending;
    expect(get(workbench.state).selected?.id).toBe('new');
    expect(get(workbench.state).dialog).toBeNull();
  });
  it('keeps live frames received during a selection without reviving a completed delta', async () => {
    const api = fixtureApi();
    let complete: (value: SessionResponse) => void = () => {};
    api.session = () =>
      new Promise((resolve) => {
        complete = resolve;
      });
    const workbench = createWorkbench(api);
    const pending = workbench.select('second');
    workbench.onEvent({ sid: 'second', ts: 1, t: 'delta', turn: 1, step: 2, text: 'old fragment' });
    workbench.onEvent({
      sid: 'second',
      ts: 4,
      t: 'tool',
      turn: 1,
      step: 3,
      name: 'read_file',
      params: {},
      state: 'ACTIVE',
    });
    complete({
      meta: fixtureSession('second'),
      events: [{ t: 'assistant', ts: 3, turn: 1, step: 2, text: 'Final reply' }],
    });
    await pending;
    expect(get(workbench.state).events.map((event) => event.t)).toEqual(['assistant', 'tool']);
    expect(get(workbench.state).events[0]).toMatchObject({ text: 'Final reply' });
  });
  it('prefers an explicit default folder when there is no selected session', async () => {
    const api = fixtureApi();
    api.state = async () => ({
      ...fixtureState([]),
      defaults: { ...fixtureState().defaults, default_cwd: '/Desktop/test/explicit' },
    });
    const workbench = createWorkbench(api);
    await workbench.refresh();
    workbench.openDialog('create');
    expect(get(workbench.state).dialog?.cwd).toBe('/Desktop/test/explicit');
  });
  it('uses stream order rather than wall-clock timestamps for live metadata', async () => {
    const workbench = createWorkbench(fixtureApi());
    await workbench.refresh();
    workbench.onEvent({
      sid: 'fixture',
      ts: 90,
      t: 'meta',
      meta: { ...fixtureSession(), updated: 90, status: 'running' },
    });
    expect(get(workbench.state).selected?.status).toBe('running');
  });
});
