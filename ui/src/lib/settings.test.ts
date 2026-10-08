/** Proves settings save ordering, editor isolation and validation before writes. */
import { get } from 'svelte/store';
import { describe, expect, it, vi } from 'vitest';
import { createApi, type Api } from './api';
import { fixtureSettings } from './fixtures.test-support';
import { createSettings, itemPath, itemTemplate, validateItemName } from './settings';
import { question } from './confirm';
import { createAccounts } from './accounts';
function fixtureApi(): Api {
  let defaults = fixtureSettings().defaults;
  return {
    ...createApi('fixture', vi.fn<typeof fetch>()),
    settings: vi.fn(async () => ({ ...fixtureSettings(), defaults })),
    saveDefaults: vi.fn(async (body) => {
      defaults = {
        ...defaults,
        ...Object.fromEntries(Object.entries(body).map(([k, v]) => [k, String(v)])),
      };
      return defaults;
    }),
    stackFile: vi.fn(async (path) => ({ path, text: 'original', exists: true })),
    writeStackFile: vi.fn(async () => ({ ok: true })),
    stackItem: vi.fn(async () => ({ command: 'original' })),
    putStackItem: vi.fn(async () => ({ ok: true })),
    copyStackItem: vi.fn(async () => ({ ok: true })),
    deleteStackItem: vi.fn(async () => ({ ok: true })),
    enableStackItem: vi.fn(async () => ({ ok: true })),
  };
}
describe('private settings', () => {
  it('deletes only the matching editor and preserves drafts opened during a slow delete', async () => {
    const api = fixtureApi(),
      settings = createSettings(api);
    await settings.edit(null, 'GEMINI.md');
    settings.editorText('kept draft');
    const unrelated = settings.remove('rule', 'other.md');
    get(question)?.answer(true);
    await unrelated;
    expect(get(settings.state).editor?.text).toBe('kept draft');
    await settings.edit('rule', 'old.md');
    let complete!: () => void;
    vi.mocked(api.deleteStackItem).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          complete = () => resolve({ ok: true });
        }),
    );
    const removed = settings.remove('rule', 'old.md');
    get(question)?.answer(true);
    await vi.waitFor(() => expect(complete).toBeTypeOf('function'));
    await settings.edit(null, 'GEMINI.md');
    settings.editorText('new draft');
    complete();
    await removed;
    expect(get(settings.state).editor?.text).toBe('new draft');
    await settings.edit('rule', 'same.md');
    const same = settings.remove('rule', 'same.md');
    get(question)?.answer(true);
    await same;
    expect(get(settings.state).editor).toBeNull();
  });
  it('rejects duplicate creation using the current private overview', async () => {
    const api = fixtureApi(),
      settings = createSettings(api);
    for (const kind of ['rule', 'skill', 'hook', 'mcp'] as const) {
      const name = kind === 'rule' ? 'kept.md' : kind === 'hook' ? 'antiquota' : 'kept';
      const data = fixtureSettings(),
        key = kind === 'mcp' ? 'mcps' : (`${kind}s` as 'rules' | 'skills' | 'hooks');
      data[key] = [{ name, here: true, about: '', cli: null }];
      vi.mocked(api.settings).mockResolvedValueOnce(data);
      expect(await settings.create(kind, kind === 'rule' ? 'kept' : name)).toBe(false);
      expect(get(settings.state).error).toContain('already exists');
    }
    expect(api.writeStackFile).not.toHaveBeenCalled();
    expect(api.putStackItem).not.toHaveBeenCalled();
  });
  it('ignores settings reads started before a successful save, including account refreshes', async () => {
    const api = fixtureApi(),
      settings = createSettings(api);
    settings.accept(fixtureSettings());
    let complete!: (value: Awaited<ReturnType<Api['settings']>>) => void;
    vi.mocked(api.settings).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          complete = resolve;
        }),
    );
    const old = settings.refresh();
    settings.field('model', 'new');
    await settings.saveField('model');
    complete(fixtureSettings());
    await old;
    expect(get(settings.state)).toMatchObject({
      loading: false,
      fields: { model: 'new' },
      data: { defaults: { model: 'new' } },
    });
    api.accounts = vi.fn(async () => []);
    api.routing = vi.fn(async () => []);
    vi.mocked(api.settings).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          complete = resolve;
        }),
    );
    const external = createAccounts(api, settings).refresh();
    settings.field('model', 'latest');
    await settings.saveField('model');
    complete(fixtureSettings());
    await external;
    expect(get(settings.state)).toMatchObject({
      fields: { model: 'latest' },
      data: { defaults: { model: 'latest' } },
    });
  });
  it('keeps a newer editor open after a slow older save and canceled creation', async () => {
    const api = fixtureApi(),
      settings = createSettings(api);
    await settings.edit('rule', 'old.md');
    let complete!: (value: { ok: boolean }) => void;
    vi.mocked(api.writeStackFile).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          complete = resolve;
        }),
    );
    const old = settings.saveEditor();
    await Promise.resolve();
    await settings.edit(null, 'GEMINI.md');
    settings.editorText('new draft');
    complete({ ok: true });
    await old;
    expect(get(settings.state).editor).toMatchObject({ name: 'GEMINI.md', text: 'new draft' });
    vi.mocked(api.writeStackFile).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          complete = resolve;
        }),
    );
    const created = settings.create('rule', 'new');
    await Promise.resolve();
    await Promise.resolve();
    settings.closeEditor();
    complete({ ok: true });
    await created;
    expect(get(settings.state).editor).toBeNull();
  });
  it('keeps cached defaults while refreshing and after read errors', async () => {
    const api = fixtureApi(),
      settings = createSettings(api),
      cached = fixtureSettings();
    settings.accept(cached);
    let reject!: (error: Error) => void;
    vi.mocked(api.settings).mockImplementationOnce(
      () =>
        new Promise((_resolve, no) => {
          reject = no;
        }),
    );
    const pending = settings.refresh();
    expect(get(settings.state)).toMatchObject({ loading: true, data: cached });
    reject(new Error('offline'));
    await pending;
    expect(get(settings.state)).toMatchObject({ loading: false, data: cached, error: 'offline' });
  });
  it('serializes rapid saves and keeps edits made during the earlier response', async () => {
    const api = fixtureApi(),
      saved = vi.fn(),
      settings = createSettings(api, saved);
    settings.accept(fixtureSettings());
    let complete!: (value: Awaited<ReturnType<Api['saveDefaults']>>) => void;
    vi.mocked(api.saveDefaults).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          complete = resolve;
        }),
    );
    settings.field('model', 'first');
    const first = settings.saveField('model');
    await Promise.resolve();
    settings.field('model', 'latest');
    const second = settings.saveField('model');
    settings.field('mode', 'plan');
    const third = settings.saveField('mode');
    settings.accept(fixtureSettings());
    expect(get(settings.state).fields).toMatchObject({ model: 'latest', mode: 'plan' });
    complete({ ...fixtureSettings().defaults, model: 'first' });
    await Promise.all([first, second, third]);
    expect(vi.mocked(api.saveDefaults).mock.calls).toEqual([
      [{ model: 'first' }],
      [{ model: 'latest' }],
      [{ mode: 'plan' }],
    ]);
    expect(get(settings.state).fields).toMatchObject({ model: 'latest', mode: 'plan' });
    expect(saved).toHaveBeenCalledTimes(3);
  });
  it('rejects invalid ranges without blocking a later valid field', async () => {
    const api = fixtureApi(),
      settings = createSettings(api);
    settings.accept(fixtureSettings());
    for (const [key, text] of [
      ['max_rounds', '0'],
      ['max_rounds', '501'],
      ['approval_timeout', '241'],
      ['auto_continue', '-1'],
      ['auto_continue', 'oops'],
    ] as const) {
      settings.field(key, text);
      expect(await settings.saveField(key)).toBe(false);
      expect(get(settings.state).saving).toBe(false);
    }
    expect(api.saveDefaults).not.toHaveBeenCalled();
    settings.field('mode', 'plan');
    expect(await settings.saveField('mode')).toBe(true);
    expect(get(settings.state).fields?.auto_continue).toBe('oops');
  });
  it('keeps the newest editor and its draft through slow reads and refreshes', async () => {
    const api = fixtureApi(),
      settings = createSettings(api);
    let complete!: (value: Awaited<ReturnType<Api['stackFile']>>) => void;
    vi.mocked(api.stackFile).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          complete = resolve;
        }),
    );
    const first = settings.edit('rule', 'old.md');
    await settings.edit('skill', 'current');
    settings.editorText('typed draft');
    await settings.refresh();
    complete({ path: 'config/rules/old.md', text: 'stale', exists: true });
    await first;
    expect(get(settings.state).editor).toMatchObject({ name: 'current', text: 'typed draft' });
    await settings.saveEditor();
    expect(api.writeStackFile).toHaveBeenCalledWith(
      'config/skills/current/SKILL.md',
      'typed draft',
    );
    expect(get(settings.state).editor).toBeNull();
  });
  it('cannot save a loading editor or reopen it after cancellation', async () => {
    const api = fixtureApi(),
      settings = createSettings(api);
    let complete!: (value: Awaited<ReturnType<Api['stackFile']>>) => void;
    vi.mocked(api.stackFile).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          complete = resolve;
        }),
    );
    const pending = settings.edit('rule', 'slow.md');
    expect(await settings.saveEditor()).toBe(false);
    settings.closeEditor();
    complete({ path: 'config/rules/slow.md', text: 'old', exists: true });
    await pending;
    expect(get(settings.state)).toMatchObject({ editor: null, loading: false });
    expect(api.writeStackFile).not.toHaveBeenCalled();
  });
  it('rejects invalid JSON, retains failed drafts and permits retry', async () => {
    const api = fixtureApi(),
      settings = createSettings(api);
    await settings.edit('mcp', 'server');
    for (const text of ['{', '[]', 'null', '42']) {
      settings.editorText(text);
      expect(await settings.saveEditor()).toBe(false);
      expect(get(settings.state).editor?.text).toBe(text);
    }
    expect(api.putStackItem).not.toHaveBeenCalled();
    settings.editorText('{"command":"test"}');
    vi.mocked(api.putStackItem).mockRejectedValueOnce(new Error('read-only'));
    expect(await settings.saveEditor()).toBe(false);
    expect(get(settings.state).error).toBe('read-only');
    expect(await settings.saveEditor()).toBe(true);
    expect(api.putStackItem).toHaveBeenLastCalledWith('mcp', 'server', { command: 'test' });
  });
  it('creates templates, copies only differing names and confirms private deletion', async () => {
    const api = fixtureApi(),
      settings = createSettings(api);
    expect(await settings.create('rule', 'new')).toBe(true);
    expect(api.writeStackFile).toHaveBeenCalledWith(
      'config/rules/new.md',
      itemTemplate('rule', 'new.md'),
    );
    expect(await settings.create('skill', '../outside')).toBe(false);
    await settings.copyAll('ide', 'skill', ['one', 'two']);
    expect(vi.mocked(api.copyStackItem).mock.calls).toEqual([
      ['ide', 'skill', 'one'],
      ['ide', 'skill', 'two'],
    ]);
    const canceled = settings.remove('rule', 'new.md');
    get(question)?.answer(false);
    expect(await canceled).toBe(false);
    expect(api.deleteStackItem).not.toHaveBeenCalled();
    const removed = settings.remove('rule', 'new.md');
    get(question)?.answer(true);
    expect(await removed).toBe(true);
    await settings.toggle('hook', 'fixture', false);
    expect(api.deleteStackItem).toHaveBeenCalledWith('rule', 'new.md');
    expect(api.enableStackItem).toHaveBeenCalledWith('hook', 'fixture', false);
  });
  it('validates safe names and uses the actual JSON file paths', () => {
    expect(validateItemName('Ab9._:@+-')).toBe('Ab9._:@+-');
    for (const name of ['', '.hidden', 'a/b', 'a'.repeat(101)])
      expect(() => validateItemName(name)).toThrow();
    expect(itemPath('hook', 'hook')).toBe('config/hooks.json');
    expect(itemPath('mcp', 'server')).toBe('config/mcp_config.json');
  });
});
