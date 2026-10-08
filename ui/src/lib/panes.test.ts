/** Proves split panes share state/drafts without sharing selection or extra state requests. */
import { get } from 'svelte/store';
import { expect, it, vi } from 'vitest';
import { createApi, type Api } from './api';
import { fixtureSession, fixtureState } from './fixtures.test-support';
import { createWorkbench } from './workbench';
import { createPanes } from './panes';
import { preferences } from './preferences';
import type { SessionResponse } from '../gen/SessionResponse';
function setup() {
  preferences.change({ split: null });
  const sessions = ['a', 'b', 'c'].map(fixtureSession);
  const api: Api = {
    ...createApi('', vi.fn()),
    state: vi.fn(async () => fixtureState(sessions)),
    session: vi.fn(async (id) => ({ meta: fixtureSession(id), events: [] })),
  };
  const left = createWorkbench(api),
    panes = createPanes(api, left);
  return { api, left, panes, sessions };
}
it('retains drafts across swapping, reopening and independent refresh with one snapshot', async () => {
  const { api, left, panes } = setup();
  await left.refresh('a');
  left.draft('left draft');
  await panes.open('b');
  panes.right.draft('right draft');
  await panes.right.refresh();
  expect(api.state).toHaveBeenCalledTimes(1);
  expect(get(left.state).selected?.id).toBe('a');
  expect(get(panes.right.state).selected?.id).toBe('b');
  await panes.swap();
  expect(get(left.state).selected?.id).toBe('b');
  expect(get(panes.right.state).selected?.id).toBe('a');
  expect(get(left.state).drafts.b).toBe('right draft');
  expect(get(panes.right.state).drafts.a).toBe('left draft');
  panes.close();
  await panes.open('a');
  expect(get(panes.right.state).drafts.a).toBe('left draft');
  await panes.open('b');
  expect(get(panes.state).sid).toBe('a');
  panes.dispose();
});
it('rejects stale right selections after replacement or close and restores only valid persisted IDs', async () => {
  const { api, left, panes } = setup();
  await left.refresh('a');
  const pending = new Map<string, (value: SessionResponse) => void>();
  api.session = (id) => new Promise((resolve) => pending.set(id, resolve));
  const old = panes.open('b'),
    next = panes.open('c');
  pending.get('c')?.({ meta: fixtureSession('c'), events: [] });
  await next;
  pending.get('b')?.({ meta: fixtureSession('b'), events: [] });
  await old;
  expect(get(panes.right.state).selected?.id).toBe('c');
  const closing = panes.open('b');
  panes.close();
  pending.get('b')?.({ meta: fixtureSession('b'), events: [] });
  await closing;
  expect(get(panes.right.state).selected).toBeNull();
  expect(get(panes.state).sid).toBeNull();
  preferences.change({ split: 'missing' });
  await panes.restore();
  expect(get(preferences.state).split).toBeNull();
  preferences.change({ split: 'a' });
  await panes.restore();
  expect(get(panes.state).sid).toBeNull();
  panes.dispose();
});
it('reconciles right metadata and closes a deleted right session', async () => {
  const { left, panes, sessions } = setup();
  await left.refresh('a');
  await panes.open('b');
  left.acceptSnapshot(
    fixtureState(
      sessions.map((s) => (s.id === 'b' ? { ...s, title: 'Renamed', status: 'running' } : s)),
    ),
  );
  panes.reconcile();
  expect(get(panes.right.state).selected).toMatchObject({ title: 'Renamed', status: 'running' });
  left.acceptSnapshot(fixtureState([fixtureSession('a')]));
  panes.reconcile();
  expect(get(panes.state)).toEqual({ sid: null, active: 'left' });
  panes.dispose();
});
it('ignores a second swap while the first selection pair is pending', async () => {
  const { api, left, panes } = setup();
  await left.refresh('a');
  await panes.open('b');
  const pending = new Map<string, (value: SessionResponse) => void>();
  api.session = (id) => new Promise((resolve) => pending.set(id, resolve));
  const first = panes.swap();
  await panes.swap();
  pending.get('a')?.({ meta: fixtureSession('a'), events: [] });
  pending.get('b')?.({ meta: fixtureSession('b'), events: [] });
  await first;
  expect(get(left.state).selected?.id).toBe('b');
  expect(get(panes.right.state).selected?.id).toBe('a');
  expect(get(panes.state).sid).toBe('a');
  panes.dispose();
});
