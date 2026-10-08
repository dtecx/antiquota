/** Owns sidebar menu construction and project collapse actions. */
import { get } from 'svelte/store';
import type { Api } from './api';
import type { Session } from '../gen/Session';
import { asWorktree, type Workbench } from './workbench';
import { createSessionActions } from './session-actions';
import { openMenu, type MenuItem } from './context-menu';
import { preferences } from './preferences';
import { projectName } from './format';

export function createSidebarActions(api: Api, workbench: Workbench, select = workbench.select) {
  const actions = createSessionActions(api, workbench, select);
  function sessionMenu(session: Session, event: MouseEvent) {
    const disabled = get(actions.busy);
    openMenu(
      event,
      [
        { label: 'Rename', action: () => actions.rename(session.id), disabled },
        {
          label: session.pinned ? 'Unpin' : 'Pin to top',
          action: () => actions.pin(session),
          disabled,
        },
        {
          label: 'Fork: new session from its state',
          action: () => actions.fork(session.id),
          disabled,
        },
        {
          label: session.archived ? 'Unarchive' : 'Archive',
          action: () => actions.archive([session], !session.archived),
          disabled,
        },
        {
          label: 'Delete…',
          action: () => actions.remove([session], `Delete "${session.title || 'New session'}"?`),
          danger: true,
          disabled,
        },
      ],
      'Session actions',
    );
  }
  function toggle(cwd: string) {
    const { collapsed } = get(preferences.state);
    preferences.change({
      collapsed: collapsed.includes(cwd)
        ? collapsed.filter((path) => path !== cwd)
        : [...collapsed, cwd],
    });
  }
  function projectMenu(cwd: string, listed: Session[], event: MouseEvent) {
    const disabled = get(actions.busy),
      archives = listed.filter((session) => session.archived);
    const items: MenuItem[] = [
      {
        label: 'New session here',
        action: () => workbench.openDialog('create', { cwd, worktree: false }),
      },
      {
        label: 'New session in a git worktree',
        action: () => workbench.openDialog('create', { cwd, worktree: true }),
      },
      {
        label: 'Collapse every other project',
        action: () =>
          preferences.change({
            collapsed: [
              ...new Set(
                (get(workbench.state).snapshot?.sessions ?? []).map(
                  (session) => asWorktree(session.worktree)?.repo ?? session.cwd,
                ),
              ),
            ].filter((path) => path !== cwd),
          }),
      },
      { label: 'Expand every project', action: () => preferences.change({ collapsed: [] }) },
      { label: 'Copy path', action: () => actions.copy(cwd), disabled },
      {
        label: `Archive its ${listed.length} sessions`,
        action: () => actions.archive(listed),
        disabled,
      },
    ];
    if (archives.length)
      items.push({
        label: `Delete ${archives.length} archived sessions…`,
        action: () =>
          actions.remove(archives, `Delete the archived sessions in ${projectName(cwd)}?`),
        danger: true,
        disabled,
      });
    items.push({
      label: `Delete all ${listed.length} sessions…`,
      action: () =>
        actions.remove(listed, `Delete every session in ${projectName(cwd)}?`, 'Delete all'),
      danger: true,
      disabled,
    });
    openMenu(event, items, 'Project actions');
  }
  return { sessionMenu, projectMenu, toggle };
}
