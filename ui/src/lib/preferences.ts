/** Keeps device preferences compatible and tolerates blocked or malformed storage. */
import { get, writable } from 'svelte/store';
export const notificationLabels = {
  turn: 'A turn finishes',
  goal: 'A goal is met',
  approval: 'agy needs your approval',
  job: 'A background job finishes',
  quota: 'Every account is out of quota',
};
export type NotificationKind = keyof typeof notificationLabels;
export interface Preferences {
  collapsed: string[];
  split: string | null;
  zoom: number;
  view: 'grouped' | 'steps';
  stamps: boolean;
  focused: boolean;
  detail: 'detailed' | 'compact';
  notify: Record<NotificationKind, boolean>;
}
export function createPreferences(storage?: Pick<Storage, 'getItem' | 'setItem'>) {
  const defaults: Preferences = {
    collapsed: [],
    split: null,
    zoom: 1,
    view: 'grouped',
    stamps: true,
    focused: false,
    detail: 'detailed',
    notify: { turn: true, goal: true, approval: true, job: true, quota: true },
  };
  let initial = defaults;
  try {
    const value: unknown = JSON.parse(storage?.getItem('antiquota.prefs') ?? '{}');
    if (value && typeof value === 'object') {
      const row = value as Partial<Preferences>;
      initial = {
        ...defaults,
        collapsed: Array.isArray(row.collapsed)
          ? row.collapsed.filter((s): s is string => typeof s === 'string')
          : [],
        split: typeof row.split === 'string' ? row.split : null,
        zoom:
          typeof row.zoom === 'number' && Number.isFinite(row.zoom)
            ? Math.min(1.6, Math.max(0.7, row.zoom))
            : 1,
        view: row.view === 'steps' ? 'steps' : 'grouped',
        stamps: row.stamps !== false,
        focused: row.focused === true,
        notify: Object.fromEntries(
          Object.keys(notificationLabels).map((key) => [
            key,
            row.notify?.[key as NotificationKind] !== false,
          ]),
        ) as Preferences['notify'],
      };
    }
    initial.detail = storage?.getItem('antiquota.detail') === 'compact' ? 'compact' : 'detailed';
  } catch {
    /* Device storage can be disabled or from an older version. */
  }
  const state = writable(initial);
  function change(patch: Partial<Preferences>) {
    state.update((value) => ({ ...value, ...patch }));
    try {
      storage?.setItem('antiquota.prefs', JSON.stringify(get(state)));
      storage?.setItem('antiquota.detail', get(state).detail);
    } catch {
      /* Preferences still work for this window. */
    }
  }
  return {
    state,
    change,
    notify: (kind: NotificationKind, on: boolean) =>
      change({ notify: { ...get(state).notify, [kind]: on } }),
  };
}
let storage: Storage | undefined;
try {
  storage = typeof localStorage === 'undefined' ? undefined : localStorage;
} catch {
  /* A webview can deny localStorage access. */
}
export const preferences = createPreferences(storage);
