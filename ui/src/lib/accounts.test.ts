/** Proves account mutations, focused drafts and snapshot/stream reconciliation. */
import { get } from 'svelte/store';
import { describe, expect, it, vi } from 'vitest';
import { createApi, type Api } from './api';
import { createAccounts, fieldsOf } from './accounts';
import { fixtureAccount, fixtureSettings, fixtureState } from './fixtures.test-support';
function savedDefaults(body: Parameters<Api['saveDefaults']>[0]) {
  const defaults = { ...fixtureSettings().defaults };
  for (const key of Object.keys(body) as Array<keyof typeof body>) {
    if (body[key] !== undefined) defaults[key] = String(body[key]);
  }
  return defaults;
}
function fixtureApi(): Api {
  return {
    ...createApi('synthetic', vi.fn<typeof fetch>()),
    accounts: vi.fn(async () => [fixtureAccount()]),
    settings: vi.fn(async () => fixtureSettings()),
    routing: vi.fn(async () => []),
    pauseAccount: vi.fn(async (_slot, body) => [{ ...fixtureAccount(), paused: body.on ?? false }]),
    checkAccount: vi.fn(async () => [fixtureAccount()]),
    probeAccounts: vi.fn(async () => ({ ok: true })),
    removeAccount: vi.fn(async () => ({ ok: true })),
    saveDefaults: vi.fn(async (body) => savedDefaults(body)),
  };
}
it('refreshes idle accounts sequentially, skips current busy slots, and reports partial failure', async () => {
  const api = fixtureApi();
  const slots = ['first', 'failed', 'running', 'removed', 'last'].map((name) => ({
    ...fixtureAccount(),
    slot: `${name}@example.com`,
  }));
  api.accounts = vi.fn(async () => slots);
  const accounts = createAccounts(api);
  await accounts.refresh();
  let finish: (value: Awaited<ReturnType<Api['checkAccount']>>) => void = () => {};
  api.checkAccount = vi.fn<Api['checkAccount']>((slot) => {
    if (slot === 'first@example.com')
      return new Promise((resolve) => {
        finish = resolve;
      });
    if (slot === 'failed@example.com')
      return Promise.reject(new Error('Quota service unavailable'));
    const current = get(accounts.state).accounts;
    expect(current.find((account) => account.slot === 'running@example.com')?.busy).toBe(true);
    return Promise.resolve(current);
  });
  const pending = accounts.checkAll();
  await accounts.checkAll();
  await accounts.check('last@example.com');
  expect(api.checkAccount).toHaveBeenCalledTimes(1);
  expect(get(accounts.state).busy).toBe('refresh-all');
  expect(get(accounts.state).feedback).toBe('Refreshing first@example.com…');
  const live = slots
    .filter((account) => account.slot !== 'removed@example.com')
    .map((account) => ({
      ...account,
      busy: account.slot === 'running@example.com',
      probed: 999,
    }));
  accounts.onEvent({ t: 'accounts', ts: 999, sid: null, accounts: live });
  finish(slots);
  await pending;
  expect(vi.mocked(api.checkAccount).mock.calls.map(([slot]) => slot)).toEqual([
    'first@example.com',
    'failed@example.com',
    'last@example.com',
  ]);
  expect(get(accounts.state).feedback).toBe(
    'Refreshed 2 of 5 accounts. Skipped 2 busy or removed. Failed 1.',
  );
  expect(get(accounts.state).error).toBe('failed@example.com: Quota service unavailable');
  expect(get(accounts.state).busy).toBe('');
});
it('refreshes paused accounts without resuming them and handles no eligible accounts', async () => {
  const api = fixtureApi();
  api.accounts = vi.fn(async () => [{ ...fixtureAccount(), paused: true }]);
  api.checkAccount = vi.fn(async () => [{ ...fixtureAccount(), paused: true }]);
  const accounts = createAccounts(api);
  await accounts.refresh();
  await accounts.checkAll();
  expect(get(accounts.state).accounts[0]?.paused).toBe(true);
  expect(api.pauseAccount).not.toHaveBeenCalled();
  accounts.onEvent({
    t: 'accounts',
    ts: 1000,
    sid: null,
    accounts: [{ ...fixtureAccount(), busy: true }],
  });
  vi.mocked(api.checkAccount).mockClear();
  await accounts.checkAll();
  expect(api.checkAccount).not.toHaveBeenCalled();
  expect(get(accounts.state).feedback).toBe(
    'Refreshed 0 of 1 accounts. Skipped 1 busy or removed.',
  );
});
describe('account settings store', () => {
  it('keeps live accounts and focused drafts during a stale refresh', async () => {
    const api = fixtureApi();
    let complete: (value: Awaited<ReturnType<Api['accounts']>>) => void = () => {};
    api.accounts = () =>
      new Promise((resolve) => {
        complete = resolve;
      });
    const accounts = createAccounts(api),
      pending = accounts.refresh();
    accounts.field('reserve', '17');
    accounts.onEvent({
      t: 'accounts',
      ts: 101,
      sid: null,
      accounts: [{ ...fixtureAccount(), paused: true }],
    });
    complete([fixtureAccount()]);
    await pending;
    expect(get(accounts.state).accounts[0]?.paused).toBe(true);
    expect(get(accounts.state).fields.reserve).toBe('17');
    expect(get(accounts.state).dirty).toBe(true);
  });
  it('persists pause/resume and check readiness from returned accounts', async () => {
    const api = fixtureApi(),
      accounts = createAccounts(api);
    await accounts.refresh();
    await accounts.pause(fixtureAccount());
    expect(api.pauseAccount).toHaveBeenCalledWith('one@example.com', { on: true });
    await accounts.pause({ ...fixtureAccount(), paused: true });
    expect(api.pauseAccount).toHaveBeenLastCalledWith('one@example.com', { on: false });
    api.checkAccount = vi.fn(async () => [
      { ...fixtureAccount(), verify: 'https://example.invalid/verify' },
    ]);
    await accounts.check('one@example.com');
    expect(get(accounts.state).feedback).toBe('Still not verified.');
    api.checkAccount = vi.fn(async () => [fixtureAccount()]);
    await accounts.check('one@example.com');
    expect(get(accounts.state).feedback).toBe('Refreshed one@example.com.');
  });
  it('validates ranges before networking and saves string-valued settings', async () => {
    const api = fixtureApi(),
      accounts = createAccounts(api);
    await accounts.refresh();
    for (const [key, bad] of [
      ['reserve', '-1'],
      ['reserve', '51'],
      ['quota_poll', '1'],
      ['quota_poll', '241'],
    ] as const) {
      accounts.field(key, bad);
      await accounts.save();
      expect(get(accounts.state).error).toContain(key === 'reserve' ? 'Reserve' : 'Quota check');
      accounts.field(key, key === 'reserve' ? '3' : '10');
    }
    expect(api.saveDefaults).not.toHaveBeenCalled();
    accounts.field('rotation', 'round-robin');
    accounts.field('reserve', '0');
    accounts.field('quota_poll', '240');
    await accounts.save();
    expect(api.saveDefaults).toHaveBeenCalledWith({
      rotation: 'round-robin',
      reserve: '0',
      quota_poll: '240',
    });
    expect(get(accounts.state).dirty).toBe(false);
  });
  it('preserves a field edited while saving earlier defaults', async () => {
    const api = fixtureApi(),
      accounts = createAccounts(api);
    await accounts.refresh();
    let complete: (value: Awaited<ReturnType<Api['saveDefaults']>>) => void = () => {};
    api.saveDefaults = () =>
      new Promise((resolve) => {
        complete = resolve;
      });
    accounts.field('reserve', '8');
    const pending = accounts.save();
    accounts.field('reserve', '9');
    complete({ ...fixtureSettings().defaults, reserve: '8' });
    await pending;
    expect(get(accounts.state).fields.reserve).toBe('9');
    expect(get(accounts.state).dirty).toBe(true);
  });
  it('confirms removal and keeps the confirmation on error for retry', async () => {
    const api = fixtureApi(),
      accounts = createAccounts(api);
    await accounts.refresh();
    accounts.requestRemove('one@example.com');
    accounts.requestRemove(null);
    expect(api.removeAccount).not.toHaveBeenCalled();
    accounts.requestRemove('one@example.com');
    api.removeAccount = vi
      .fn()
      .mockRejectedValueOnce(new Error('Account is busy'))
      .mockResolvedValueOnce({ ok: true });
    await accounts.remove();
    expect(get(accounts.state).remove).toBe('one@example.com');
    expect(get(accounts.state).error).toBe('Account is busy');
    await accounts.remove();
    expect(get(accounts.state).accounts).toEqual([]);
    expect(get(accounts.state).remove).toBeNull();
  });
  it('retains cached content after refresh errors and releases failed mutation controls', async () => {
    const api = fixtureApi(),
      accounts = createAccounts(api);
    accounts.acceptSnapshot({ ...fixtureState(), accounts: [fixtureAccount()] });
    api.accounts = vi.fn(async () => {
      throw new Error('offline');
    });
    await accounts.refresh();
    expect(get(accounts.state).accounts).toHaveLength(1);
    expect(get(accounts.state).loading).toBe(false);
    api.checkAccount = vi.fn(async () => {
      throw new Error('Refresh failed');
    });
    await accounts.check('one@example.com');
    expect(get(accounts.state).busy).toBe('');
    expect(get(accounts.state).error).toBe('Refresh failed');
  });
  it('coalesces rapid field changes and persists the newest draft after the in-flight save', async () => {
    const api = fixtureApi(),
      accounts = createAccounts(api);
    await accounts.refresh();
    let complete: (value: Awaited<ReturnType<Api['saveDefaults']>>) => void = () => {};
    api.saveDefaults = vi
      .fn<Api['saveDefaults']>()
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            complete = resolve;
          }),
      )
      .mockImplementation(async (body) => savedDefaults(body));
    accounts.field('reserve', '7');
    const first = accounts.saveField('reserve');
    accounts.field('reserve', '8');
    void accounts.saveField('reserve');
    accounts.field('quota_poll', '12');
    void accounts.saveField('quota_poll');
    complete({ ...fixtureSettings().defaults, reserve: '7' });
    await first;
    expect(api.saveDefaults).toHaveBeenCalledTimes(2);
    expect(api.saveDefaults).toHaveBeenLastCalledWith({ reserve: '8', quota_poll: '12' });
    expect(get(accounts.state).fields).toMatchObject({ reserve: '8', quota_poll: '12' });
    expect(get(accounts.state).dirty).toBe(false);
  });
  it('saves a valid field independently of another invalid unsaved input', async () => {
    const api = fixtureApi(),
      accounts = createAccounts(api);
    await accounts.refresh();
    accounts.field('reserve', '51');
    accounts.field('rotation', 'sticky');
    await accounts.saveField('rotation');
    expect(api.saveDefaults).toHaveBeenCalledWith({ rotation: 'sticky' });
    expect(get(accounts.state).fields.reserve).toBe('51');
    expect(get(accounts.state).dirty).toBe(true);
  });
  it('saves placement and treats a missing placement as fill first', async () => {
    const api = fixtureApi();
    // The engine leaves the default placement out of its settings.
    api.saveDefaults = vi.fn(async (body) => {
      const saved = savedDefaults(body);
      if (saved.placement === 'fill') delete saved.placement;
      return saved;
    });
    const accounts = createAccounts(api);
    await accounts.refresh();
    expect(get(accounts.state).fields.placement).toBe('fill');
    expect(fieldsOf({ ...fixtureSettings().defaults, placement: 'spread' }).placement).toBe(
      'spread',
    );
    accounts.field('placement', 'spread');
    await accounts.saveField('placement');
    expect(api.saveDefaults).toHaveBeenLastCalledWith({ placement: 'spread' });
    expect(get(accounts.state)).toMatchObject({ dirty: false, fields: { placement: 'spread' } });
    accounts.field('placement', 'fill');
    await accounts.saveField('placement');
    expect(get(accounts.state)).toMatchObject({ dirty: false, fields: { placement: 'fill' } });
    accounts.field('placement', 'both');
    await accounts.saveField('placement');
    expect(api.saveDefaults).toHaveBeenCalledTimes(2);
    expect(get(accounts.state).error).toBe('Choose where new sessions go.');
  });
});
