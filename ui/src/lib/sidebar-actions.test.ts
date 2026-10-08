/** Proves collapse-other includes pinned-only and hidden archived projects without depending on the filter. */
import { get } from 'svelte/store';
import { expect, it, vi } from 'vitest';
import { createApi } from './api';
import { createWorkbench } from './workbench';
import { createSidebarActions } from './sidebar-actions';
import { contextMenu, closeMenu } from './context-menu';
import { preferences } from './preferences';
import { fixtureSession, fixtureState } from './fixtures.test-support';
it('collapses every other parent folder including pinned-only and archived projects', async () => {
  vi.stubGlobal('HTMLElement', class {});
  try {
    const current = fixtureSession('current'),
      sessions = [
        current,
        { ...fixtureSession('pin'), pinned: true, cwd: '/Desktop/test/pinned-only' },
        { ...fixtureSession('archive'), archived: true, cwd: '/Desktop/test/hidden' },
        {
          ...fixtureSession('wt'),
          cwd: '/Desktop/test/tree',
          worktree: {
            repo: '/Desktop/test/parent',
            path: '/Desktop/test/tree',
            branch: 'branch',
            base: 'main',
            base_sha: 'synthetic',
          },
        },
      ];
    const api = {
      ...createApi('', vi.fn()),
      state: async () => fixtureState(sessions),
      session: async () => ({ meta: current, events: [] }),
    };
    const workbench = createWorkbench(api);
    await workbench.refresh();
    workbench.change({ filter: 'current' });
    const actions = createSidebarActions(api, workbench);
    actions.projectMenu(current.cwd, [current], {
      preventDefault: () => {},
      type: 'contextmenu',
      clientX: 10,
      clientY: 10,
      currentTarget: null,
    } as unknown as MouseEvent);
    await get(contextMenu)
      ?.items.find((item) => item.label === 'Collapse every other project')
      ?.action();
    expect(get(preferences.state).collapsed).toEqual([
      '/Desktop/test/pinned-only',
      '/Desktop/test/hidden',
      '/Desktop/test/parent',
    ]);
    await get(contextMenu)
      ?.items.find((item) => item.label === 'Expand every project')
      ?.action();
    expect(get(preferences.state).collapsed).toEqual([]);
    closeMenu();
  } finally {
    vi.unstubAllGlobals();
  }
});
