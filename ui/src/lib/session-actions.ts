/** Owns sidebar mutations and confirmation boundaries while retaining their original targets. */
import { writeClipboard } from './native';
import { get, writable } from 'svelte/store';
import type { Api } from './api';
import type { Session } from '../gen/Session';
import type { Workbench } from './workbench';
import { ask } from './confirm';
export function createSessionActions(api: Api, workbench: Workbench, select = workbench.select) {
  const busy = writable(false);
  let pending = false;
  async function act(action: () => Promise<void>) {
    if (pending) return;
    pending = true;
    busy.set(true);
    try {
      await action();
    } catch (error) {
      workbench.change({
        error: error instanceof Error ? error.message : 'Session action failed.',
      });
    } finally {
      pending = false;
      busy.set(false);
    }
  }
  async function remove(sessions: Session[], title: string, ok = 'Delete') {
    const ids = sessions.map((s) => s.id);
    if (
      !(await ask(title, 'Transcripts and handoff state are deleted. Files in the project stay.', {
        ok,
        danger: true,
      }))
    )
      return;
    await act(async () => {
      for (const id of ids) await api.deleteSession(id);
      await workbench.refresh();
    });
  }
  return {
    busy,
    rename: (id: string) =>
      act(async () => {
        await select(id);
        if (get(workbench.state).selected?.id === id) workbench.openDialog('rename');
      }),
    pin: (session: Session) =>
      act(async () => {
        await api.update(session.id, { pinned: !session.pinned });
        await workbench.refresh();
      }),
    archive: (sessions: Session[], on = true) =>
      act(async () => {
        for (const s of sessions) if (s.archived !== on) await api.update(s.id, { archived: on });
        await workbench.refresh();
      }),
    fork: (id: string) =>
      act(async () => {
        const session = await api.fork(id);
        await workbench.refresh(session.id);
      }),
    remove,
    copy: (path: string) =>
      act(async () => {
        await writeClipboard(path);
      }),
  };
}
