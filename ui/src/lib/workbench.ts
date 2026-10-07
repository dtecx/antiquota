/** Owns session selection, drafts, mutations, and snapshot/stream reconciliation. */
import { get, writable } from 'svelte/store';
import type { EventRecord } from '../gen/EventRecord';
import type { PatchSessionRequest } from '../gen/PatchSessionRequest';
import type { Session } from '../gen/Session';
import type { StateResponse } from '../gen/StateResponse';
import type { StreamEvent } from '../gen/StreamEvent';
import type { Api } from './api';
import { projectHistory, reduceTranscript } from './transcript';

export interface DialogState {
  kind: 'create' | 'rename';
  title: string;
  cwd: string;
}
export interface WorkbenchState {
  snapshot: StateResponse | null;
  selected: Session | null;
  events: EventRecord[];
  drafts: Record<string, string>;
  filter: string;
  sidebarOpen: boolean;
  dialog: DialogState | null;
  loading: boolean;
  mutating: boolean;
  error: string;
  feedback: string;
}
const initial: WorkbenchState = {
  snapshot: null,
  selected: null,
  events: [],
  drafts: {},
  filter: '',
  sidebarOpen: false,
  dialog: null,
  loading: true,
  mutating: false,
  error: '',
  feedback: '',
};
export function createWorkbench(api: Api, remember: (id: string) => void = () => {}) {
  const state = writable<WorkbenchState>({ ...initial, drafts: {} });
  let wanted = '';
  let generation = 0;
  let syncing = false;
  let buffered: StreamEvent[] = [];
  let syncAgain = false;
  let nextPreferred: string | undefined;
  const selecting = new Map<number, StreamEvent[]>();
  function fail(error: unknown) {
    state.update((value) => ({
      ...value,
      error: error instanceof Error ? error.message : 'An unexpected error occurred.',
    }));
  }
  function replaceSession(session: Session) {
    state.update((value) => ({
      ...value,
      selected: wanted === session.id ? session : value.selected,
      snapshot: value.snapshot
        ? {
            ...value.snapshot,
            sessions: [
              session,
              ...value.snapshot.sessions.filter((item) => item.id !== session.id),
            ],
          }
        : null,
    }));
  }
  function apply(event: StreamEvent) {
    if (event.t === 'meta') {
      replaceSession(event.meta);
      return;
    }
    if (event.t === 'deleted') {
      void refresh();
      return;
    }
    if (event.t === 'notice' && event.kind === 'resync') {
      void refresh();
      return;
    }
    if (event.sid !== wanted) return;
    const { sid: _sid, ...record } = event;
    void _sid;
    state.update((value) => ({ ...value, events: reduceTranscript(value.events, record) }));
    if (event.t === 'turn_end') void refresh();
  }
  function onEvent(event: StreamEvent) {
    if (syncing) buffered.push(event);
    else if (selecting.has(generation)) selecting.get(generation)?.push(event);
    else apply(event);
  }
  async function select(id: string) {
    wanted = id;
    const own = ++generation;
    selecting.set(own, []);
    remember(id);
    state.update((value) => ({
      ...value,
      loading: true,
      sidebarOpen: false,
      error: '',
      feedback: '',
      selected: value.snapshot?.sessions.find((item) => item.id === id) ?? null,
      events: [],
    }));
    try {
      const response = await api.session(id);
      if (own === generation)
        state.update((value) => ({
          ...value,
          selected: response.meta,
          events: projectHistory(response.events),
          loading: false,
        }));
    } catch (error) {
      if (own === generation) {
        fail(error);
        state.update((value) => ({ ...value, loading: false }));
      }
    } finally {
      const pending = selecting.get(own) ?? [];
      selecting.delete(own);
      if (own === generation) for (const event of pending) apply(event);
    }
  }
  async function refresh(preferred?: string) {
    if (syncing) {
      syncAgain = true;
      if (preferred) nextPreferred = preferred;
      return;
    }
    syncing = true;
    const own = generation;
    try {
      const snapshot = await api.state();
      state.update((value) => ({ ...value, snapshot, error: '' }));
      const id =
        preferred || wanted || snapshot.sessions.find((session) => !session.archived)?.id || '';
      if (id && snapshot.sessions.some((session) => session.id === id)) {
        if (id !== wanted) await select(id);
        else {
          const response = await api.session(id);
          if (own === generation)
            state.update((value) => ({
              ...value,
              selected: response.meta,
              events: projectHistory(response.events),
              loading: false,
            }));
        }
      } else if (own === generation) {
        wanted = '';
        state.update((value) => ({ ...value, selected: null, events: [], loading: false }));
      }
    } catch (error) {
      fail(error);
      state.update((value) => ({ ...value, loading: false }));
    } finally {
      syncing = false;
      const pending = buffered;
      buffered = [];
      for (const event of pending) apply(event);
      if (syncAgain) {
        syncAgain = false;
        const preferred = nextPreferred;
        nextPreferred = undefined;
        await refresh(preferred);
      }
    }
  }
  async function mutate(action: () => Promise<void>) {
    if (get(state).mutating) return;
    state.update((value) => ({ ...value, mutating: true, error: '', feedback: '' }));
    try {
      await action();
    } catch (error) {
      fail(error);
    } finally {
      state.update((value) => ({ ...value, mutating: false }));
    }
  }
  function openDialog(kind: DialogState['kind']) {
    const value = get(state);
    state.update((current) => ({
      ...current,
      error: '',
      dialog: {
        kind,
        title: kind === 'rename' ? (value.selected?.title ?? '') : '',
        cwd:
          kind === 'rename'
            ? (value.selected?.cwd ?? '')
            : value.selected?.cwd ||
              value.snapshot?.defaults.default_cwd ||
              value.snapshot?.defaults.cwd ||
              '',
      },
    }));
  }
  return {
    state,
    onEvent,
    select,
    refresh,
    change: (patch: Partial<Pick<WorkbenchState, 'filter' | 'sidebarOpen' | 'dialog' | 'error'>>) =>
      state.update((value) => ({ ...value, ...patch })),
    draft: (text: string) =>
      state.update((value) => ({ ...value, drafts: { ...value.drafts, [wanted]: text } })),
    openDialog,
    dialogField: (field: 'title' | 'cwd', text: string) =>
      state.update((value) => ({
        ...value,
        dialog: value.dialog ? { ...value.dialog, [field]: text } : null,
      })),
    saveDialog: () =>
      mutate(async () => {
        const value = get(state);
        if (!value.dialog) return;
        const { kind, title, cwd } = value.dialog;
        if (kind === 'rename' && value.selected)
          replaceSession(await api.update(value.selected.id, { title }));
        else {
          const session = await api.create({
            cwd,
            title,
            model: value.snapshot?.defaults.model || 'gemini-3.8-flash-high',
          });
          await refresh(session.id);
        }
        state.update((current) => ({ ...current, dialog: null }));
      }),
    update: (patch: PatchSessionRequest) =>
      mutate(async () => {
        if (wanted) replaceSession(await api.update(wanted, patch));
      }),
    send: () =>
      mutate(async () => {
        const id = wanted;
        const text = get(state).drafts[id] ?? '';
        if (!id || !text.trim()) return;
        const response = await api.send(id, { text });
        state.update((value) => ({
          ...value,
          drafts: {
            ...value.drafts,
            [id]: value.drafts[id] === text ? '' : (value.drafts[id] ?? ''),
          },
          feedback: response.result === 'queued' ? 'Message queued for the next turn.' : '',
        }));
      }),
    stop: () =>
      mutate(async () => {
        if (wanted) {
          await api.stop(wanted);
          await refresh();
        }
      }),
  };
}
export type Workbench = ReturnType<typeof createWorkbench>;
