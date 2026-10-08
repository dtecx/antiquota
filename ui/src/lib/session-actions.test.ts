/** Proves asynchronous menu operations preserve targets and confirmation boundaries. */
import { get } from 'svelte/store';
import { expect, it, vi } from 'vitest';
import { createApi, type Api } from './api';
import { createWorkbench } from './workbench';
import { createSessionActions } from './session-actions';
import { fixtureSession, fixtureState } from './fixtures.test-support';
import type { SessionResponse } from '../gen/SessionResponse';
import { question } from './confirm';
function setup() {
  const api: Api = {
    ...createApi('', vi.fn()),
    state: vi.fn(async () => fixtureState(['a', 'b'].map(fixtureSession))),
    session: vi.fn(async (id) => ({ meta: fixtureSession(id), events: [] })),
    deleteSession: vi.fn(async () => ({ ok: true })),
  };
  const workbench = createWorkbench(api);
  return { api, workbench, actions: createSessionActions(api, workbench) };
}
it('abandons a stale rename rather than opening another session title editor', async () => {
  const { api, workbench, actions } = setup();
  await workbench.refresh('a');
  let finish: (value: SessionResponse) => void = () => {};
  api.session = vi
    .fn<Api['session']>()
    .mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    )
    .mockResolvedValueOnce({ meta: fixtureSession('b'), events: [] });
  const pending = actions.rename('a');
  await workbench.select('b');
  finish({ meta: fixtureSession('a'), events: [] });
  await pending;
  expect(get(workbench.state).selected?.id).toBe('b');
  expect(get(workbench.state).dialog).toBeNull();
});
it('cancel deletes nothing and confirmation deletes the captured list despite a selection change', async () => {
  const { api, workbench, actions } = setup();
  await workbench.refresh('a');
  const cancelled = actions.remove([fixtureSession('a')], 'Delete A?');
  get(question)?.answer(false);
  await cancelled;
  expect(api.deleteSession).not.toHaveBeenCalled();
  const confirmed = actions.remove([fixtureSession('a')], 'Delete A?');
  await workbench.select('b');
  get(question)?.answer(true);
  await confirmed;
  expect(api.deleteSession).toHaveBeenCalledExactlyOnceWith('a');
  expect(get(workbench.state).selected?.id).toBe('b');
});
