/** Owns two independent session selections sharing the primary snapshot and one event connection. */
import { get, writable } from 'svelte/store';
import type { Api } from './api';
import { createWorkbench, type Workbench } from './workbench';
import { preferences } from './preferences';
import { drawer } from './git';
export function createPanes(api: Api, left: Workbench) {
  const state = writable<{ sid: string | null; active: 'left' | 'right' }>({
    sid: null,
    active: 'left',
  });
  const right = createWorkbench(
    api,
    () => {},
    () => get(left.state).snapshot,
  );
  const mirror = (from: Workbench, to: Workbench) =>
    from.state.subscribe((value) => {
      if (get(to.state).drafts !== value.drafts)
        to.state.update((other) => ({ ...other, drafts: value.drafts }));
    });
  const stopLeft = mirror(left, right),
    stopRight = mirror(right, left);
  let generation = 0,
    swapping = false;
  async function open(id: string) {
    const snapshot = get(left.state).snapshot;
    if (
      !snapshot ||
      id === get(left.state).selected?.id ||
      !snapshot.sessions.some((s) => s.id === id)
    )
      return;
    generation++;
    preferences.change({ split: id });
    state.set({ sid: id, active: 'right' });
    right.acceptSnapshot(snapshot);
    drawer.update((d) => ({ ...d, open: false }));
    await right.select(id);
  }
  function close() {
    generation++;
    preferences.change({ split: null });
    state.set({ sid: null, active: 'left' });
    right.clearSelection();
  }
  async function swap(active: 'left' | 'right' = 'left') {
    const l = get(left.state).selected?.id,
      r = get(state).sid;
    if (!l || !r || l === r || swapping) return;
    swapping = true;
    const own = ++generation;
    try {
      await Promise.all([left.select(r), right.select(l)]);
      if (own !== generation) return;
      preferences.change({ split: l });
      state.set({ sid: l, active });
    } finally {
      swapping = false;
    }
  }
  return {
    state,
    right,
    dispose: () => {
      stopLeft();
      stopRight();
      right.clearSelection();
    },
    open,
    close,
    focus: (active: 'left' | 'right') => state.update((v) => ({ ...v, active })),
    restore: async () => {
      const id = get(preferences.state).split;
      if (
        id &&
        id !== get(left.state).selected?.id &&
        get(left.state).snapshot?.sessions.some((s) => s.id === id)
      )
        await open(id);
      else if (id) close();
    },
    reconcile: () => {
      const snapshot = get(left.state).snapshot;
      if (!snapshot) return;
      right.acceptSnapshot(snapshot);
      const id = get(state).sid;
      if (id && !snapshot.sessions.some((s) => s.id === id)) close();
    },
    swap,
    select: async (id: string) => {
      if (id === get(state).sid) await swap();
      else await left.select(id);
    },
  };
}
export type Panes = ReturnType<typeof createPanes>;
