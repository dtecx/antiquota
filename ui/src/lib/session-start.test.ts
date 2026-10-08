/** Proves new-session defaults and the bounded same-folder handoff choices. */
import { get } from 'svelte/store';
import { expect, it, vi } from 'vitest';
import { createApi } from './api';
import { createWorkbench } from './workbench';
import { fixtureSession, fixtureState } from './fixtures.test-support';
import { dialogDefaults, earlierSessions } from './session-start';
function bench() {
  return get(createWorkbench(createApi('test')).state);
}
it('uses current model/mode and the parent repository of a worktree', () => {
  const selected = {
    ...fixtureSession(),
    model: 'concrete-low',
    mode: 'plan' as const,
    cwd: '/test/tree',
    worktree: {
      repo: '/test/parent',
      path: '/test/tree',
      base: 'main',
      branch: 'test',
      base_sha: '1',
    },
  };
  expect(
    dialogDefaults({ ...bench(), selected, snapshot: fixtureState() }, 'create', {}),
  ).toMatchObject({ cwd: '/test/parent', model: 'concrete-low', mode: 'plan', worktree: false });
});
it('uses configured defaults without a selection and honors explicit project-menu worktree choices', () => {
  const snapshot = fixtureState([]);
  Object.assign(snapshot.defaults, {
    default_cwd: '/test/configured',
    model: 'concrete-high',
    mode: 'accept-edits',
    worktree_default: 'on',
  });
  expect(dialogDefaults({ ...bench(), snapshot }, 'create', {})).toMatchObject({
    cwd: '/test/configured',
    model: 'concrete-high',
    mode: 'accept-edits',
    worktree: true,
  });
  expect(
    dialogDefaults({ ...bench(), snapshot }, 'create', { cwd: '/test/menu', worktree: false }),
  ).toMatchObject({ cwd: '/test/menu', worktree: false });
});
it('posts a concrete model, mode, trimmed folder and worktree choice and retains failures without a fake session', async () => {
  const api = {
    ...createApi('test'),
    state: vi.fn(async () => fixtureState([])),
    create: vi.fn(async () => {
      throw new Error('creation refused');
    }),
  };
  const workbench = createWorkbench(api);
  await workbench.refresh();
  workbench.openDialog('create', {
    cwd: ' /test/new ',
    model: 'concrete-low',
    mode: 'plan',
    worktree: true,
  });
  await workbench.saveDialog();
  expect(api.create).toHaveBeenCalledWith({
    cwd: '/test/new',
    title: '',
    model: 'concrete-low',
    mode: 'plan',
    worktree: true,
  });
  expect(get(workbench.state)).toMatchObject({
    selected: null,
    snapshot: { sessions: [] },
    error: 'creation refused',
    dialog: { cwd: ' /test/new ' },
  });
  workbench.change({ dialog: null });
  expect(api.create).toHaveBeenCalledOnce();
});
it('offers at most three completed earlier sessions from exactly the same folder, newest first', () => {
  const selected = fixtureSession('current');
  const rows = [1, 4, 2, 3].map((updated) => ({
    ...fixtureSession(String(updated)),
    updated,
    turns: 1,
  }));
  expect(
    earlierSessions(selected, [
      selected,
      ...rows,
      { ...fixtureSession('empty'), updated: 99 },
      { ...fixtureSession('other'), cwd: '/test/elsewhere', updated: 98, turns: 1 },
    ]).map((row) => row.id),
  ).toEqual(['4', '3', '2']);
  expect(earlierSessions(null, rows)).toEqual([]);
});
