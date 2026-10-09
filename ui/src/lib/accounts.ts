/** Owns account settings state, mutation feedback and snapshot/stream reconciliation. */
import { get, writable } from 'svelte/store';
import type { Account } from '../gen/Account';
import type { DefaultsRequest } from '../gen/DefaultsRequest';
import type { Settings } from '../gen/Settings';
import type { StateResponse } from '../gen/StateResponse';
import type { StreamEvent } from '../gen/StreamEvent';
import type { RoutingEntry } from '../gen/RoutingEntry';
import type { Api } from './api';
import type { SettingsSection, SettingsStore } from './settings';

export interface AccountsView {
  open: boolean;
  section: SettingsSection;
  accounts: Account[];
  defaults: Settings | null;
  fields: { rotation: string; reserve: string; quota_poll: string; placement: string };
  dirty: boolean;
  routing: RoutingEntry[];
  loading: boolean;
  busy: string;
  saving: boolean;
  remove: string | null;
  error: string;
  feedback: string;
}
/** The rotation fields of the saved defaults; a missing placement is the default, fill. */
export function fieldsOf(settings: Settings): AccountsView['fields'] {
  return {
    rotation: settings.rotation,
    reserve: settings.reserve,
    quota_poll: settings.quota_poll,
    placement: settings.placement ?? 'fill',
  };
}
export function createAccounts(
  api: Api,
  settingsStore?: Pick<SettingsStore, 'accept' | 'reading'>,
) {
  const state = writable<AccountsView>({
    open: false,
    section: 'accounts',
    accounts: [],
    defaults: null,
    fields: { rotation: 'perishable', reserve: '3', quota_poll: '10', placement: 'fill' },
    dirty: false,
    routing: [],
    loading: false,
    busy: '',
    saving: false,
    remove: null,
    error: '',
    feedback: '',
  });
  let revision = 0;
  let generation = 0;
  let routingGeneration = 0;
  const pending = new Set<keyof AccountsView['fields']>();
  let draining = false;
  function defaults(settings: Settings) {
    state.update((value) => ({
      ...value,
      defaults: settings,
      fields: value.dirty ? value.fields : fieldsOf(settings),
    }));
  }
  function accept(accounts: Account[]) {
    revision++;
    state.update((value) => ({ ...value, accounts }));
  }
  function fail(error: unknown) {
    state.update((value) => ({
      ...value,
      error: error instanceof Error ? error.message : 'An unexpected error occurred.',
    }));
  }
  async function refresh() {
    const settingsReading = settingsStore?.reading();
    const own = ++generation;
    const before = revision;
    const routingOwn = ++routingGeneration;
    state.update((value) => ({ ...value, loading: true, error: '' }));
    try {
      const [accounts, settings, routing] = await Promise.all([
        api.accounts(),
        api.settings(),
        api.routing(),
      ]);
      if (own !== generation) return;
      if (revision === before) accept(accounts);
      defaults(settings.defaults);
      settingsStore?.accept(settings, settingsReading);
      if (routingOwn === routingGeneration) state.update((value) => ({ ...value, routing }));
    } catch (error) {
      if (own === generation) fail(error);
    } finally {
      if (own === generation) state.update((value) => ({ ...value, loading: false }));
    }
  }
  async function mutate(label: string, action: () => Promise<void>) {
    if (get(state).busy) return;
    state.update((value) => ({ ...value, busy: label, error: '', feedback: '' }));
    try {
      await action();
    } catch (error) {
      fail(error);
    } finally {
      state.update((value) => ({ ...value, busy: '' }));
    }
  }
  function field(key: keyof AccountsView['fields'], text: string) {
    state.update((value) => ({ ...value, fields: { ...value.fields, [key]: text }, dirty: true }));
  }
  function validate(fields: AccountsView['fields'], keys: Array<keyof AccountsView['fields']>) {
    for (const key of keys) {
      const text = fields[key],
        number = Number(text);
      if (key === 'rotation') {
        if (!['perishable', 'sticky', 'round-robin'].includes(text))
          throw new Error('Choose a rotation order.');
      } else if (key === 'placement') {
        if (!['fill', 'spread'].includes(text)) throw new Error('Choose where new sessions go.');
      } else if (
        !text.trim() ||
        !Number.isFinite(number) ||
        number < (key === 'reserve' ? 0 : 2) ||
        number > (key === 'reserve' ? 50 : 240)
      ) {
        throw new Error(
          key === 'reserve'
            ? 'Reserve must be between 0 and 50 percent.'
            : 'Quota check must be between 2 and 240 minutes.',
        );
      }
    }
  }
  async function drain() {
    if (draining) return;
    draining = true;
    state.update((value) => ({ ...value, saving: true, error: '', feedback: '' }));
    try {
      while (pending.size) {
        const keys = [...pending];
        pending.clear();
        try {
          const submitted = { ...get(state).fields };
          validate(submitted, keys);
          const body: DefaultsRequest = {};
          for (const key of keys) body[key] = submitted[key];
          const saved = await api.saveDefaults(body),
            values = fieldsOf(saved);
          state.update((value) => {
            const fields = { ...value.fields };
            for (const key of keys) if (fields[key] === submitted[key]) fields[key] = values[key];
            return {
              ...value,
              defaults: saved,
              fields,
              dirty: Object.keys(fields).some(
                (key) => fields[key as keyof typeof fields] !== values[key as keyof typeof values],
              ),
              feedback: 'Rotation settings saved.',
            };
          });
        } catch (error) {
          fail(error);
        }
      }
    } finally {
      draining = false;
      state.update((value) => ({ ...value, saving: false }));
    }
  }
  function save(keys: Array<keyof AccountsView['fields']> = ['rotation', 'reserve', 'quota_poll']) {
    for (const key of keys) pending.add(key);
    return drain();
  }
  return {
    state,
    refresh,
    acceptSnapshot: (snapshot: StateResponse) => {
      accept(snapshot.accounts);
      defaults(snapshot.defaults);
    },
    onEvent: (event: StreamEvent) => {
      if (event.t === 'accounts') accept(event.accounts);
      if ((event.t === 'turn_end' || event.t === 'accounts') && get(state).open) {
        const own = ++routingGeneration;
        void api
          .routing()
          .then((routing) => {
            if (own === routingGeneration) state.update((value) => ({ ...value, routing }));
          })
          .catch((error) => {
            if (own === routingGeneration) fail(error);
          });
      }
    },
    open: (section: SettingsSection = 'accounts') => {
      state.update((value) => ({ ...value, open: true, section, error: '', feedback: '' }));
      void refresh();
    },
    section: (section: SettingsSection) => state.update((value) => ({ ...value, section })),
    close: () => state.update((value) => ({ ...value, open: false, remove: null, error: '' })),
    field,
    pause: (account: Account) =>
      mutate(account.slot, async () => {
        accept(await api.pauseAccount(account.slot, { on: !account.paused }));
        state.update((value) => ({
          ...value,
          feedback: `${account.slot} ${account.paused ? 'resumed' : 'paused'}.`,
        }));
      }),
    check: (slot: string) =>
      mutate(slot, async () => {
        const accounts = await api.checkAccount(slot);
        accept(accounts);
        state.update((value) => ({
          ...value,
          feedback: accounts.find((account) => account.slot === slot)?.verify
            ? 'Still not verified.'
            : `${slot} is ready.`,
        }));
      }),
    probe: () =>
      mutate('probe', async () => {
        await api.probeAccounts();
        state.update((value) => ({ ...value, feedback: 'Probing accounts…' }));
      }),
    requestRemove: (slot: string | null) =>
      state.update((value) => ({ ...value, remove: slot, error: '' })),
    remove: () =>
      mutate('remove', async () => {
        const slot = get(state).remove;
        if (!slot) return;
        await api.removeAccount(slot);
        accept(get(state).accounts.filter((account) => account.slot !== slot));
        state.update((value) => ({ ...value, remove: null, feedback: `Removed ${slot}.` }));
      }),
    save: () => save(),
    saveField: (key: keyof AccountsView['fields']) => save([key]),
  };
}
export type Accounts = ReturnType<typeof createAccounts>;
