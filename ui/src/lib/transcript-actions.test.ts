/** Proves transcript actions retain their initiating session and preserve newer viewer state. */
import { get } from 'svelte/store';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createApi, type Api } from './api';
import { question } from './confirm';
import { fixtureSession, fixtureSettings, fixtureState } from './fixtures.test-support';
import { viewer, type FileText } from './git';
import { createTranscriptActions } from './transcript-actions';
import { createWorkbench } from './workbench';

function deferred<T>() {
  let resolve: (value: T) => void = () => {};
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}
function file(name = 'src/app.ts'): FileText {
  return { path: `/Desktop/test/project/${name}`, rel: name, text: `Complete text for ${name}` };
}
async function setup(selected = true) {
  const snapshot = fixtureState(selected ? [fixtureSession(), fixtureSession('next')] : []);
  const settings = fixtureSettings();
  settings.permissions.allow = ['command(ls)', 'command(git)'];
  settings.permissions.deny = ['command(rm)'];
  const api: Api = {
    ...createApi('synthetic', vi.fn<typeof fetch>()),
    state: vi.fn(async () => snapshot),
    session: vi.fn(async (id) => ({ meta: fixtureSession(id), events: [] })),
    file: vi.fn(async (_id, name) => file(name)),
    settings: vi.fn(async () => settings),
    savePermissions: vi.fn(async (body) => ({
      allow: body.allow ?? [...new Set([...settings.permissions.allow, ...(body.add_allow ?? [])])],
      deny: settings.permissions.deny,
    })),
    update: vi.fn(async (id, patch) => ({ ...fixtureSession(id), ...patch })),
    send: vi.fn(async () => ({ result: 'started' })),
    rewind: vi.fn(async () => ({ files: ['src/app.ts'] })),
  };
  const workbench = createWorkbench(api);
  await workbench.refresh();
  return { api, settings, workbench, actions: createTranscriptActions(api, workbench) };
}
afterEach(() => {
  get(question)?.answer(false);
  viewer.set(null);
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

describe('transcript files and clipboard', () => {
  it('loads exact prompt source without sending or changing the transcript', async () => {
    const { api, actions, workbench } = await setup();
    const events = get(workbench.state).events;
    const text = '  Keep **formatting**.\n\n';
    expect(await actions.reuse(text)).toBe(true);
    expect(get(workbench.state).drafts.fixture).toBe(text);
    expect(get(workbench.state).events).toBe(events);
    expect(get(question)).toBeNull();
    expect(api.send).not.toHaveBeenCalled();
  });
  it('confirms draft replacement, preserving it on cancellation', async () => {
    const { actions, workbench } = await setup();
    workbench.draft('Unsent text');
    const cancelled = actions.reuse('Earlier prompt');
    expect(get(question)?.ok).toBe('Replace draft');
    get(question)?.answer(false);
    expect(await cancelled).toBe(false);
    expect(get(workbench.state).drafts.fixture).toBe('Unsent text');
    const accepted = actions.reuse('Earlier prompt');
    get(question)?.answer(true);
    expect(await accepted).toBe(true);
    expect(get(workbench.state).drafts.fixture).toBe('Earlier prompt');
  });
  it.each(['selection', 'draft', 'disposed'])(
    'keeps newer state after a pending reuse: %s',
    async (change) => {
      const { api, actions, workbench } = await setup();
      workbench.draft('Original unsent');
      const pending = actions.reuse('Earlier prompt');
      if (change === 'selection') await workbench.select('next');
      else if (change === 'draft') workbench.draft('Newer unsent');
      else actions.dispose();
      get(question)?.answer(true);
      expect(await pending).toBe(false);
      expect(get(workbench.state).drafts.fixture).toBe(
        change === 'draft' ? 'Newer unsent' : 'Original unsent',
      );
      expect(get(workbench.state).drafts.next).not.toBe('Earlier prompt');
      expect(api.send).not.toHaveBeenCalled();
    },
  );
  it('ignores reuse when no session is selected or history is loading', async () => {
    const empty = await setup(false);
    expect(await empty.actions.reuse('Prompt')).toBe(false);
    const loading = await setup();
    loading.workbench.state.update((value) => ({ ...value, loading: true }));
    expect(await loading.actions.reuse('Prompt')).toBe(false);
    expect(get(loading.workbench.state).drafts.fixture).toBeUndefined();
    expect(get(question)).toBeNull();
  });
  it('opens the complete decoded file target and passes its requested line to the viewer', async () => {
    const { api, actions } = await setup();
    await actions.file('src/my%20file.ts#L27');
    expect(api.file).toHaveBeenCalledExactlyOnceWith('fixture', 'src/my file.ts');
    expect(get(viewer)).toEqual({ ...file('src/my file.ts'), line: 27 });
    expect(api.send).not.toHaveBeenCalled();
  });

  it('does not let a delayed file response replace a different viewer', async () => {
    const { api, actions } = await setup();
    const response = deferred<FileText>();
    api.file = vi.fn(() => response.promise);
    const pending = actions.file('src/old.ts');
    const newer = file('src/new.ts');
    viewer.set(newer);
    response.resolve(file('src/old.ts'));
    await pending;
    expect(get(viewer)).toBe(newer);
  });

  it('keeps the newer file request authoritative across separate pane controllers', async () => {
    const left = await setup(),
      right = await setup();
    await right.workbench.select('next');
    const older = deferred<FileText>(),
      newer = deferred<FileText>();
    left.api.file = vi.fn(() => older.promise);
    right.api.file = vi.fn(() => newer.promise);
    const oldRequest = left.actions.file('src/left.ts');
    const newRequest = right.actions.file('src/right.ts#L9');
    older.resolve(file('src/left.ts'));
    await oldRequest;
    expect(get(viewer)).toBeNull();
    newer.resolve(file('src/right.ts'));
    await newRequest;
    expect(get(viewer)).toEqual({ ...file('src/right.ts'), line: 9 });
    expect(left.api.file).toHaveBeenCalledExactlyOnceWith('fixture', 'src/left.ts');
    expect(right.api.file).toHaveBeenCalledExactlyOnceWith('next', 'src/right.ts');
  });

  it('does not open a pending file after disposing its transcript controller', async () => {
    const { api, actions } = await setup();
    const response = deferred<FileText>();
    api.file = vi.fn(() => response.promise);
    const pending = actions.file('src/removed-pane.ts');
    actions.dispose();
    response.resolve(file('src/removed-pane.ts'));
    await pending;
    expect(get(viewer)).toBeNull();
    await actions.file('src/after-dispose.ts');
    expect(api.file).toHaveBeenCalledExactlyOnceWith('fixture', 'src/removed-pane.ts');
  });

  it('does not open an old session file after selecting another session', async () => {
    const { api, workbench, actions } = await setup();
    const response = deferred<FileText>();
    api.file = vi.fn(() => response.promise);
    const pending = actions.file('src/old.ts#3');
    await workbench.select('next');
    response.resolve(file('src/old.ts'));
    await pending;
    expect(api.file).toHaveBeenCalledExactlyOnceWith('fixture', 'src/old.ts');
    expect(get(viewer)).toBeNull();
    expect(get(workbench.state).selected?.id).toBe('next');
  });

  it('reports a failed file read without replacing its previous viewer', async () => {
    const { api, workbench, actions } = await setup();
    const previous = file('src/previous.ts');
    viewer.set(previous);
    api.file = vi.fn(async () => {
      throw new Error('File is outside the project.');
    });
    await actions.file('../outside.ts');
    expect(get(viewer)).toBe(previous);
    expect(get(workbench.state).error).toBe('File is outside the project.');
  });

  it('copies exactly the source text rather than rendered markup', async () => {
    const { workbench, actions } = await setup();
    const writeText = vi.fn(async () => {});
    vi.stubGlobal('navigator', { clipboard: { writeText } });
    const source = '**bold**\n<script>literal</script>\n\n';
    await actions.copy(source);
    expect(writeText).toHaveBeenCalledExactlyOnceWith(source);
    expect(get(workbench.state).feedback).toBe('Copied');
  });

  it.each([new Error('Clipboard access denied'), 'untyped rejection'])(
    'reports clipboard failure without claiming success: %s',
    async (error) => {
      const { workbench, actions } = await setup();
      vi.stubGlobal('navigator', { clipboard: { writeText: vi.fn().mockRejectedValue(error) } });
      await actions.copy('Keep this text');
      expect(get(workbench.state).error).toBe(
        error instanceof Error ? error.message : 'Could not copy text.',
      );
      expect(get(workbench.state).feedback).not.toBe('Copied');
    },
  );
});

describe('transcript rewind', () => {
  it.each([0, -1, 1.5, NaN, Infinity, Number.MAX_SAFE_INTEGER + 1])(
    'ignores invalid turn %s without a question or API call',
    async (turn) => {
      const { api, actions } = await setup();
      await actions.rewind(turn);
      expect(get(question)).toBeNull();
      expect(api.rewind).not.toHaveBeenCalled();
    },
  );

  it('asks a dangerous question and cancellation leaves the original session unchanged', async () => {
    const { api, workbench, actions } = await setup();
    const pending = actions.rewind(2);
    expect(get(question)).toMatchObject({
      title: 'Rewind before turn 2?',
      danger: true,
      ok: 'Rewind',
    });
    expect(get(question)?.text).toContain('Later user edits are preserved');
    get(question)!.answer(false);
    await pending;
    expect(api.rewind).not.toHaveBeenCalled();
    expect(get(workbench.state).selected?.id).toBe('fixture');
  });

  it('confirms the specified turn for its original session across selection and draft changes', async () => {
    const { api, workbench, actions } = await setup();
    const pending = actions.rewind(3);
    const confirmation = get(question)!;
    await workbench.select('next');
    workbench.draft('New session draft');
    workbench.draft('New original draft', 'fixture');
    confirmation.answer(true);
    await pending;
    expect(api.rewind).toHaveBeenCalledExactlyOnceWith('fixture', 3);
    expect(get(workbench.state).selected?.id).toBe('next');
    expect(get(workbench.state).drafts).toMatchObject({
      fixture: 'New original draft',
      next: 'New session draft',
    });
    expect(api.send).not.toHaveBeenCalled();
  });
});

describe('denied action retries', () => {
  it('submits only deduplicated additions and never rewrites existing allow or deny', async () => {
    const { api, settings, workbench, actions } = await setup();
    settings.permissions.allow.push('command(cat)');
    const original = structuredClone(settings);
    await actions.allow(['command(cat)', 'command(ls)', 'command(test)', 'command(test)'], false);
    expect(api.settings).not.toHaveBeenCalled();
    expect(api.savePermissions).toHaveBeenCalledExactlyOnceWith({
      add_allow: ['command(cat)', 'command(ls)', 'command(test)'],
    });
    expect(settings).toEqual(original);
    expect(api.send).not.toHaveBeenCalled();
    expect(get(workbench.state).feedback).toBe('Allowed command(cat), command(ls), command(test)');
  });

  it('retains its original retry target and newer draft while an atomic grant is pending', async () => {
    const { api, workbench, actions } = await setup();
    const response = deferred<Awaited<ReturnType<Api['savePermissions']>>>();
    api.savePermissions = vi.fn(() => response.promise);
    const pending = actions.allow(['command(test)'], true);
    await workbench.select('next');
    workbench.draft('Newer draft');
    response.resolve({ allow: ['command(test)'], deny: ['command(rm)'] });
    await pending;
    expect(api.savePermissions).toHaveBeenCalledExactlyOnceWith({ add_allow: ['command(test)'] });
    expect(api.send).toHaveBeenCalledExactlyOnceWith('fixture', {
      text: 'Permission granted (command(test)). Retry what was denied and continue.',
    });
    expect(get(workbench.state).selected?.id).toBe('next');
    expect(get(workbench.state).drafts.next).toBe('Newer draft');
  });

  it('grants before retry and retains the original target across a delayed grant', async () => {
    const { api, workbench, actions } = await setup();
    const response = deferred<Awaited<ReturnType<Api['savePermissions']>>>();
    api.savePermissions = vi.fn(() => response.promise);
    const pending = actions.allow(['command(test)', 'command(test)'], true);
    await vi.waitFor(() => expect(api.savePermissions).toHaveBeenCalledOnce());
    expect(api.send).not.toHaveBeenCalled();
    await workbench.select('next');
    response.resolve({ allow: ['command(test)'], deny: ['command(rm)'] });
    await pending;
    expect(api.send).toHaveBeenCalledExactlyOnceWith('fixture', {
      text: 'Permission granted (command(test)). Retry what was denied and continue.',
    });
    expect(get(workbench.state).selected?.id).toBe('next');
  });

  it('does not retry after a failed permission grant', async () => {
    const { api, workbench, actions } = await setup();
    api.savePermissions = vi.fn(async () => {
      throw new Error('Grant failed');
    });
    await actions.allow(['command(test)'], true);
    expect(api.send).not.toHaveBeenCalled();
    expect(get(workbench.state).error).toBe('Grant failed');
  });

  it('two pane grants submit separate atomic additions without stale replacement lists', async () => {
    const first = await setup(),
      second = await setup();
    await Promise.all([
      first.actions.allow(['command(git status)'], true),
      second.actions.allow(['command(cargo test)'], true),
    ]);
    expect(first.api.savePermissions).toHaveBeenCalledExactlyOnceWith({
      add_allow: ['command(git status)'],
    });
    expect(second.api.savePermissions).toHaveBeenCalledExactlyOnceWith({
      add_allow: ['command(cargo test)'],
    });
    expect(first.api.settings).not.toHaveBeenCalled();
    expect(second.api.settings).not.toHaveBeenCalled();
    expect(first.api.send).toHaveBeenCalledOnce();
    expect(second.api.send).toHaveBeenCalledOnce();
  });

  it('patches accept edits before retrying its captured session and retains newer selection', async () => {
    const { api, workbench, actions } = await setup();
    const response = deferred<ReturnType<typeof fixtureSession>>();
    api.update = vi.fn(() => response.promise);
    const pending = actions.edits();
    expect(api.update).toHaveBeenCalledExactlyOnceWith('fixture', { mode: 'accept-edits' });
    expect(api.send).not.toHaveBeenCalled();
    await workbench.select('next');
    workbench.draft('Do not consume this draft');
    response.resolve({ ...fixtureSession(), mode: 'accept-edits' });
    await pending;
    expect(api.send).toHaveBeenCalledExactlyOnceWith('fixture', {
      text: 'Edits are now allowed. Retry what was denied and continue.',
    });
    expect(get(workbench.state).selected).toMatchObject({ id: 'next', mode: 'default' });
    expect(get(workbench.state).drafts.next).toBe('Do not consume this draft');
    expect(
      get(workbench.state).snapshot?.sessions.find((session) => session.id === 'fixture')?.mode,
    ).toBe('accept-edits');
  });

  it('does not retry after an accept edits patch fails', async () => {
    const { api, workbench, actions } = await setup();
    api.update = vi.fn(async () => {
      throw new Error('Mode patch failed');
    });
    await actions.edits();
    expect(api.send).not.toHaveBeenCalled();
    expect(get(workbench.state).error).toBe('Mode patch failed');
    expect(get(workbench.state).selected?.mode).toBe('default');
  });

  it('ignores duplicate actions while a permission operation holds the workbench lock', async () => {
    const { api, settings, actions } = await setup();
    const response = deferred<typeof settings>();
    api.settings = vi.fn(() => response.promise);
    const pending = actions.allow(['command(test)'], true);
    await actions.allow(['command(cat)'], true);
    await actions.edits();
    await actions.rewind(1);
    expect(api.settings).not.toHaveBeenCalled();
    expect(api.update).not.toHaveBeenCalled();
    expect(get(question)).toBeNull();
    response.resolve(settings);
    await pending;
    expect(api.savePermissions).toHaveBeenCalledOnce();
    expect(api.send).toHaveBeenCalledOnce();
  });

  it('does not invalidate the admitted file request when a duplicate click is locked out', async () => {
    const { api, actions } = await setup();
    const response = deferred<FileText>();
    api.file = vi.fn(() => response.promise);
    const pending = actions.file('src/first.ts');
    await actions.file('src/ignored.ts');
    expect(api.file).toHaveBeenCalledExactlyOnceWith('fixture', 'src/first.ts');
    response.resolve(file('src/first.ts'));
    await pending;
    expect(get(viewer)).toEqual(file('src/first.ts'));
  });

  it('does nothing without a selected session or without supplied permission rules', async () => {
    const { api, actions } = await setup(false);
    await actions.file('src/app.ts');
    await actions.rewind(1);
    await actions.allow(['command(test)'], true);
    await actions.edits();
    expect(get(question)).toBeNull();
    for (const operation of [
      api.file,
      api.rewind,
      api.settings,
      api.savePermissions,
      api.update,
      api.send,
    ])
      expect(operation).not.toHaveBeenCalled();
    const selected = await setup();
    await selected.actions.allow([], true);
    expect(selected.api.settings).not.toHaveBeenCalled();
  });
});
