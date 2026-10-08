/** Owns session selection, drafts, mutations, and snapshot/stream reconciliation. */
import { get, writable } from 'svelte/store';
import type { PatchSessionRequest } from '../gen/PatchSessionRequest';
import type { Session } from '../gen/Session';
import type { StateResponse } from '../gen/StateResponse';
import type { StreamEvent } from '../gen/StreamEvent';
import type { Api } from './api';
import { projectHistory, reduceTranscript } from './transcript';
import type { GoalRequest } from '../gen/GoalRequest';
import { EXECUTE_PLAN } from './goals';
import { acceptDraft, transferDraft } from './session-draft';
import { jobLog } from './jobs';

import type { DialogState, WorkbenchState } from './workbench-state';
export type { DialogState, WorkbenchState } from './workbench-state';
const initial: WorkbenchState = {
  snapshot: null,
  selected: null,
  events: [],
  drafts: {},
  filter: '',
  sidebarOpen: false,
  showArchived: false,
  dialog: null,
  loading: true,
  mutating: false,
  error: '',
  feedback: '',
};
export function createWorkbench(
  api: Api,
  remember: (id: string) => void = () => {},
  shared?: () => StateResponse | null,
) {
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
    if (event.t === 'accounts') {
      state.update((value) => ({
        ...value,
        snapshot: value.snapshot ? { ...value.snapshot, accounts: event.accounts } : null,
      }));
      return;
    }
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
      const snapshot = shared?.() ?? (await api.state());
      state.update((value) => ({ ...value, snapshot, error: '' }));
      const candidate = preferred || wanted;
      const id = snapshot.sessions.some((s) => s.id === candidate)
        ? candidate
        : snapshot.sessions.find((s) => !s.archived)?.id || '';
      if (id && snapshot.sessions.some((session) => session.id === id)) {
        if (id !== wanted && own === generation) await select(id);
        else if (id === wanted) {
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
  function openDialog(kind: DialogState['kind'], options: Partial<DialogState> = {}) {
    const value = get(state);
    state.update((current) => ({
      ...current,
      error: '',
      dialog: {
        id: kind === 'rename' ? value.selected?.id : undefined,
        worktree: kind === 'create' && current.snapshot?.defaults.worktree_default === 'on',
        kind,
        title: kind === 'rename' ? (value.selected?.title ?? '') : '',
        cwd:
          kind === 'rename'
            ? (value.selected?.cwd ?? '')
            : value.selected?.cwd ||
              value.snapshot?.defaults.default_cwd ||
              value.snapshot?.defaults.cwd ||
              '',
        ...options,
      },
    }));
  }
  return {
    state,
    onEvent,
    acceptSession: replaceSession,
    select,
    refresh,
    acceptSnapshot: (snapshot: StateResponse) =>
      state.update((value) => ({
        ...value,
        snapshot,
        selected: snapshot.sessions.find((s) => s.id === wanted) ?? value.selected,
      })),
    clearSelection: () => {
      wanted = '';
      generation++;
      state.update((value) => ({ ...value, selected: null, events: [], loading: false }));
    },
    change: (
      patch: Partial<
        Pick<
          WorkbenchState,
          'filter' | 'sidebarOpen' | 'showArchived' | 'dialog' | 'error' | 'feedback'
        >
      >,
    ) => state.update((value) => ({ ...value, ...patch })),
    draft: (text: string, id = wanted) =>
      state.update((value) => ({ ...value, drafts: { ...value.drafts, [id]: text } })),
    run: mutate,
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
        const dialog = value.dialog;
        const { kind, title, cwd, worktree, id, pendingText } = dialog;
        if (kind === 'rename' && id) replaceSession(await api.update(id, { title }));
        else {
          const session = await api.create({
            cwd,
            title,
            worktree,
            model: value.snapshot?.defaults.model || 'gemini-3.8-flash-high',
          });
          if (pendingText) {
            state.update((current) => ({
              ...current,
              drafts: transferDraft(current.drafts, session.id, pendingText),
            }));
          }
          await refresh(session.id);
          // Creation succeeded; a failed first send must retry in this session.
          state.update((current) => ({
            ...current,
            dialog: current.dialog === dialog ? null : current.dialog,
          }));
          if (pendingText) {
            await api.send(session.id, { text: pendingText });
            state.update((current) => ({
              ...current,
              drafts: acceptDraft(current.drafts, session.id, pendingText),
            }));
          }
        }
        state.update((current) => ({
          ...current,
          dialog: current.dialog === dialog ? null : current.dialog,
        }));
      }),
    update: (patch: PatchSessionRequest, id = wanted) =>
      mutate(async () => {
        if (id) replaceSession(await api.update(id, patch));
      }),
    /** Sends the draft with uploaded attachments; resolves true once the engine took it. */
    send: async (attachments: string[] = []) => {
      let sent = false;
      await mutate(async () => {
        const id = wanted;
        const text = get(state).drafts[id] ?? '';
        if (!id || (!text.trim() && !attachments.length)) return;
        const response = await api.send(id, attachments.length ? { text, attachments } : { text });
        sent = true;
        state.update((value) => ({
          ...value,
          drafts: acceptDraft(value.drafts, id, text),
          feedback:
            response.result !== 'queued'
              ? ''
              : attachments.length
                ? 'Queued after the current turn.'
                : 'Message queued for the next turn. Text arrives at the next step.',
        }));
      });
      return sent;
    },
    setGoal: (body: GoalRequest) =>
      mutate(async () => {
        const id = wanted;
        if (!id) return;
        replaceSession(await api.setGoal(id, body));
        state.update((value) =>
          value.drafts[id]?.trim() === body.objective
            ? { ...value, drafts: { ...value.drafts, [id]: '' } }
            : value,
        );
      }),
    resumeGoal: () =>
      mutate(async () => {
        if (wanted) await api.resumeGoal(wanted);
      }),
    clearGoal: () =>
      mutate(async () => {
        if (wanted) await api.clearGoal(wanted);
      }),
    executePlan: (mode: 'accept-edits' | 'default') =>
      mutate(async () => {
        const id = wanted;
        if (!id) return;
        replaceSession(await api.update(id, { mode }));
        await api.send(id, { text: EXECUTE_PLAN });
        state.update((value) => ({
          ...value,
          feedback:
            mode === 'accept-edits'
              ? 'Executing the plan.'
              : 'Executing: edits and commands ask you first.',
        }));
      }),
    worktreeField: (on: boolean) =>
      state.update((value) => ({
        ...value,
        dialog: value.dialog ? { ...value.dialog, worktree: on } : null,
      })),
    /** Adds an @path mention to the current draft. */
    mention: (path: string) =>
      state.update((value) => {
        const draft = value.drafts[wanted] ?? '';
        const separator = draft && !draft.endsWith(' ') ? ' ' : '';
        return { ...value, drafts: { ...value.drafts, [wanted]: `${draft}${separator}@${path} ` } };
      }),
    removeWorktree: (deleteBranch: boolean) =>
      mutate(async () => {
        if (wanted) replaceSession(await api.gitRemove(wanted, deleteBranch));
      }),
    stopJob: (jid: string) =>
      mutate(async () => {
        if (wanted) await api.stopJob(wanted, jid);
      }),
    openJobLog: async (jid: string) => {
      try {
        if (wanted) jobLog.set(await api.jobLog(wanted, jid));
      } catch (error) {
        fail(error);
      }
    },
    deleteLoop: () =>
      mutate(async () => {
        if (wanted) await api.deleteLoop(wanted);
      }),
    answer: async (id: string, decision: 'allow' | 'deny', always: boolean) => {
      try {
        await api.answerApproval(id, { decision, always });
      } catch (error) {
        fail(
          error instanceof Error && 'status' in error && error.status === 404
            ? new Error('That request expired (the engine restarted).')
            : error,
        );
      }
    },
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

/** A stored worktree record, or null. */
export function asWorktree(value: unknown): import('../gen/Worktree').Worktree | null {
  if (!value || typeof value !== 'object' || !('branch' in value) || !('repo' in value))
    return null;
  return value as import('../gen/Worktree').Worktree;
}
