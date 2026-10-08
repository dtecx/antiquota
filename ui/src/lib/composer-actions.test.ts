/** Proves local command dispatch, confirmation targets and draft preservation. */
import { get } from 'svelte/store';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createApi, type Api } from './api';
import { commands } from './commands';
import { createComposerActions, INIT_PROMPT } from './composer-actions';
import { question } from './confirm';
import { downloadSession } from './export';
import { fixtureSession, fixtureState } from './fixtures.test-support';
import { viewer } from './git';
import { goalDialog } from './goals';
import { createWorkbench } from './workbench';

vi.mock('./export', () => ({ downloadSession: vi.fn() }));

function fixtureApi(): Api {
  const snapshot = fixtureState([fixtureSession(), fixtureSession('next')]);
  snapshot.defaults.verifier_model = 'gemini-verifier-fixture';
  snapshot.models = [
    { id: 'gemini-3.8-flash-high', label: 'Gemini Flash (high)' },
    { id: 'gemini-3.8-flash-low', label: 'Gemini Flash (low)' },
  ];
  return {
    ...createApi('synthetic', vi.fn<typeof fetch>()),
    state: vi.fn(async () => snapshot),
    session: vi.fn(async (id) => ({ meta: fixtureSession(id), events: [] })),
    update: vi.fn(async (id, patch) => ({ ...fixtureSession(id), ...patch })),
    send: vi.fn(async () => ({ result: 'started' })),
    clear: vi.fn(async () => ({ ok: true })),
    compact: vi.fn(async () => ({ ok: true })),
    rewind: vi.fn(async () => ({ files: ['src/app.ts'] })),
    stop: vi.fn(async () => ({ ok: true })),
    resumeGoal: vi.fn(async () => ({ ok: true })),
    clearGoal: vi.fn(async () => ({ ok: true })),
    deleteSession: vi.fn(async () => ({ ok: true })),
    probeAccounts: vi.fn(async () => ({ ok: true })),
    deleteLoop: vi.fn(async () => ({ ok: true })),
    setLoop: vi.fn(async () => ({ ok: true, next: 1234 })),
    setGoal: vi.fn(async (id) => fixtureSession(id)),
    stateDocument: vi.fn(async () => ({ text: 'Durable handoff' })),
    file: vi.fn(async (_id, name) => ({
      rel: name,
      path: `/Desktop/test/project/${name}`,
      text: 'Project memory',
    })),
    shell: vi.fn<Api['shell']>(async (_id, cmd) => ({
      t: 'shell',
      ts: 1,
      cmd,
      output: 'done',
      code: 0,
      secs: 1,
    })),
    note: vi.fn(async () => ({ path: '/Desktop/test/project/GEMINI.md', file: 'GEMINI.md' })),
    createJob: vi.fn(async (_id, body) => ({
      id: 'job',
      cmd: body.cmd,
      pid: 100,
      log: 'job.log',
      started: 1,
      ended: null,
      code: null,
      every: 0,
      notify: true,
      by: 'user',
      stopped: false,
    })),
  };
}

async function setup() {
  const api = fixtureApi();
  const workbench = createWorkbench(api);
  await workbench.refresh();
  const host = { settings: vi.fn(), login: vi.fn(), help: vi.fn() };
  const actions = createComposerActions(api, workbench, host);
  const submit = (text: string) => {
    workbench.draft(text);
    return actions.submit();
  };
  return { api, workbench, host, actions, submit };
}

afterEach(() => {
  get(question)?.answer(false);
  goalDialog.set(null);
  viewer.set(null);
  vi.clearAllMocks();
});

const sections = [
  ['/accounts', 'accounts'],
  ['/usage', 'usage'],
  ['/settings', 'general'],
  ['/permissions', 'permissions'],
  ['/skills', 'skills'],
  ['/hooks', 'hooks'],
  ['/mcp', 'mcp'],
  ['/stack', 'skills'],
] as const;
const dispatched = [
  ['/clear', 'clear', ['fixture']],
  ['/compact', 'compact', ['fixture']],
  ['/rewind 2', 'rewind', ['fixture', 2]],
  ['/init', 'send', ['fixture', { text: INIT_PROMPT }]],
  ['/bg npm test', 'createJob', ['fixture', { cmd: 'npm test' }]],
  ['/loop 15m check work', 'setLoop', ['fixture', { every: '15m', prompt: 'check work' }]],
  ['/schedule 03:00 check work', 'setLoop', ['fixture', { at: '03:00', prompt: 'check work' }]],
  ['/stop', 'stop', ['fixture']],
  ['/resume', 'resumeGoal', ['fixture']],
  ['/cleargoal', 'clearGoal', ['fixture']],
  ['/model custom-model', 'update', ['fixture', { model: 'custom-model' }]],
  ['/effort LOW', 'update', ['fixture', { model: 'gemini-3.8-flash-low' }]],
  ['/mode plan', 'update', ['fixture', { mode: 'plan' }]],
  ['/rename Renamed session', 'update', ['fixture', { title: 'Renamed session' }]],
  ['/quota', 'probeAccounts', []],
  ['/delete', 'deleteSession', ['fixture']],
] as const;

describe('composer commands', () => {
  it('covers the complete 31-command reference', () => {
    const covered = [
      ...sections.map(([command]) => command),
      ...dispatched.map(([text]) => text.split(' ')[0]),
      '/help',
      '/login',
      '/new',
      '/goal',
      '/export',
      '/memory',
      '/state',
    ];
    expect(new Set(covered)).toEqual(new Set(commands.map(([command]) => command)));
    expect(new Set(covered).size).toBe(31);
  });
  it.each(sections)(
    '%s opens its settings section without an agent prompt',
    async (command, section) => {
      const { api, workbench, host, submit } = await setup();
      await submit(command.toUpperCase());
      expect(host.settings).toHaveBeenCalledExactlyOnceWith(section);
      expect(get(workbench.state).drafts.fixture).toBe('');
      expect(api.send).not.toHaveBeenCalled();
    },
  );
  it.each(['/help', '/login', '/new'])('%s opens its local entry point', async (command) => {
    const { api, workbench, host, submit } = await setup();
    await submit(command);
    if (command === '/help') expect(host.help).toHaveBeenCalledOnce();
    else if (command === '/login') expect(host.login).toHaveBeenCalledOnce();
    else expect(get(workbench.state).dialog?.kind).toBe('create');
    expect(api.send).not.toHaveBeenCalled();
    expect(get(workbench.state).drafts.fixture).toBe('');
  });
  it.each(dispatched)('%s dispatches its application action', async (text, method, args) => {
    const { api, workbench, submit } = await setup();
    const pending = submit(text);
    if (text.startsWith('/rewind') || text === '/delete') {
      expect(get(question)?.danger).toBe(true);
      get(question)!.answer(true);
    }
    await pending;
    expect(api[method]).toHaveBeenCalledExactlyOnceWith(...args);
    expect(get(workbench.state).error).toBe('');
    expect(get(workbench.state).drafts.fixture).toBe('');
    if (method !== 'send') expect(api.send).not.toHaveBeenCalled();
  });
  it('exports durable events through the download boundary', async () => {
    const { api, submit } = await setup();
    await submit('/export');
    expect(api.session).toHaveBeenLastCalledWith('fixture');
    expect(downloadSession).toHaveBeenCalledExactlyOnceWith(fixtureSession(), []);
    expect(api.send).not.toHaveBeenCalled();
  });
  it('opens the verified goal dialog or starts a verified goal with the configured model', async () => {
    const { api, submit } = await setup();
    await submit('/goal');
    expect(get(goalDialog)).toMatchObject({
      objective: '',
      verifier: true,
      verifier_model: 'gemini-verifier-fixture',
    });
    expect(api.setGoal).not.toHaveBeenCalled();
    await submit('/goal Finish the rewrite');
    expect(api.setGoal).toHaveBeenCalledExactlyOnceWith('fixture', {
      objective: 'Finish the rewrite',
      verifier: true,
      verifier_model: 'gemini-verifier-fixture',
    });
    expect(api.send).not.toHaveBeenCalled();
  });
  it('shows handoff state and falls back from GEMINI.md to AGENTS.md for memory', async () => {
    const { api, submit } = await setup();
    await submit('/state');
    expect(api.stateDocument).toHaveBeenCalledExactlyOnceWith('fixture');
    expect(get(viewer)).toEqual({
      rel: 'handoff state · Fixture session',
      path: 'STATE.md',
      text: 'Durable handoff',
    });
    vi.mocked(api.file).mockRejectedValueOnce(new Error('missing'));
    await submit('/memory');
    expect(vi.mocked(api.file).mock.calls).toEqual([
      ['fixture', 'GEMINI.md'],
      ['fixture', 'AGENTS.md'],
    ]);
    expect(get(viewer)?.rel).toBe('AGENTS.md');
    viewer.set(null);
    vi.mocked(api.file).mockRejectedValue(new Error('missing'));
    await submit('/memory');
    expect(get(viewer)).toBeNull();
    expect(api.send).not.toHaveBeenCalled();
  });
  it.each(['/delete', '/rewind 3', '/mode bypass'])(
    '%s keeps its initiating session and newer drafts across confirmation',
    async (text) => {
      const { api, workbench, submit } = await setup();
      const pending = submit(text);
      const confirmation = get(question)!;
      expect(confirmation.danger).toBe(true);
      await workbench.select('next');
      workbench.draft('New session draft');
      workbench.draft('New original draft', 'fixture');
      confirmation.answer(true);
      await pending;
      if (text === '/delete') expect(api.deleteSession).toHaveBeenCalledExactlyOnceWith('fixture');
      else if (text.startsWith('/rewind'))
        expect(api.rewind).toHaveBeenCalledExactlyOnceWith('fixture', 3);
      else expect(api.update).toHaveBeenCalledExactlyOnceWith('fixture', { mode: 'bypass' });
      expect(get(workbench.state).selected?.id).toBe('next');
      expect(get(workbench.state).drafts).toMatchObject({
        fixture: 'New original draft',
        next: 'New session draft',
      });
      expect(api.send).not.toHaveBeenCalled();
    },
  );
  it.each(['/delete', '/rewind 1', '/mode bypass'])(
    'canceling %s keeps the command without executing it',
    async (text) => {
      const { api, workbench, submit } = await setup();
      const pending = submit(text);
      get(question)!.answer(false);
      await pending;
      expect(get(workbench.state).drafts.fixture).toBe(text);
      for (const action of [api.deleteSession, api.rewind, api.update, api.send])
        expect(action).not.toHaveBeenCalled();
    },
  );
  it('captures the original session for a pending permission-picker confirmation', async () => {
    const { api, workbench, actions } = await setup();
    const pending = actions.changeMode('bypass');
    const confirmation = get(question)!;
    await workbench.select('next');
    confirmation.answer(true);
    await pending;
    expect(api.update).toHaveBeenCalledExactlyOnceWith('fixture', { mode: 'bypass' });
    expect(get(workbench.state).selected?.id).toBe('next');
    await actions.changeMode('invalid');
    expect(get(workbench.state).error).toBe('Mode: default · accept-edits · plan · bypass');
  });
  it.each([
    '/unknown',
    '/model',
    '/rename',
    '/mode invalid',
    '/rewind',
    '/rewind 0',
    '/rewind -1',
    '/rewind 1.5',
    '!',
    '#',
  ])('%s retains the draft on validation failure and never invokes the agent', async (text) => {
    const { api, workbench, submit } = await setup();
    await submit(text);
    expect(get(workbench.state).drafts.fixture).toBe(text);
    expect(get(workbench.state).error).not.toBe('');
    expect(api.send).not.toHaveBeenCalled();
    expect(get(question)).toBeNull();
  });
  it('retains failed commands and a newer draft entered while an action is pending', async () => {
    const { api, workbench, submit } = await setup();
    vi.mocked(api.clear).mockRejectedValueOnce(new Error('busy'));
    await submit('/clear');
    expect(get(workbench.state)).toMatchObject({ error: 'busy', drafts: { fixture: '/clear' } });
    let finish!: (answer: { ok: boolean }) => void;
    vi.mocked(api.compact).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    const pending = submit('/compact');
    workbench.draft('Newer message');
    finish({ ok: true });
    await pending;
    expect(get(workbench.state).drafts.fixture).toBe('Newer message');
    expect(api.send).not.toHaveBeenCalled();
  });
  it('routes shell commands and notes locally and sends the exact init instruction with optional context', async () => {
    const { api, workbench, submit } = await setup();
    await submit('  ! printf done  ');
    expect(api.shell).toHaveBeenCalledExactlyOnceWith('fixture', 'printf done');
    await submit('# Prefer focused tests');
    expect(api.note).toHaveBeenCalledExactlyOnceWith('fixture', 'Prefer focused tests');
    expect(get(workbench.state).feedback).toBe('Saved to /Desktop/test/project/GEMINI.md');
    expect(api.send).not.toHaveBeenCalled();
    await submit('/init Use Rust');
    expect(api.send).toHaveBeenCalledExactlyOnceWith('fixture', {
      text: `${INIT_PROMPT}\n\nUse Rust`,
    });
    expect(INIT_PROMPT).toBe(
      'Study this project and write a concise GEMINI.md at its root for future agents: what it is, how it is laid out, how to build, run and test it, and the conventions to follow. If GEMINI.md or AGENTS.md already exists, improve it instead of starting over. Keep it under 80 lines.',
    );
  });
  it.each(['/loop', '/loop off', '/loop STOP', '/schedule', '/schedule off', '/schedule stop'])(
    '%s removes the loop or schedule',
    async (text) => {
      const { api, submit } = await setup();
      await submit(text);
      expect(api.deleteLoop).toHaveBeenCalledExactlyOnceWith('fixture');
      expect(api.setLoop).not.toHaveBeenCalled();
      expect(api.send).not.toHaveBeenCalled();
    },
  );
  it('explains empty background commands and unavailable effort instead of sending prompts', async () => {
    const { api, workbench, submit } = await setup();
    await submit('/bg');
    expect(get(workbench.state).feedback).toBe('/bg <command>');
    expect(api.createJob).not.toHaveBeenCalled();
    await submit('/effort medium');
    expect(get(workbench.state).feedback).toBe('Gemini Flash has no effort "medium"');
    expect(api.update).not.toHaveBeenCalled();
    expect(api.send).not.toHaveBeenCalled();
  });
  it('sends ordinary text and attachments, opens creation without a session, and ignores a locked composer', async () => {
    const { api, workbench, actions } = await setup();
    workbench.draft('Regular prompt');
    expect(await actions.submit(['/Desktop/test/attachment.png'])).toBe(true);
    expect(api.send).toHaveBeenCalledExactlyOnceWith('fixture', {
      text: 'Regular prompt',
      attachments: ['/Desktop/test/attachment.png'],
    });
    workbench.state.update((value) => ({ ...value, mutating: true }));
    workbench.draft('/stop');
    await actions.submit();
    expect(api.stop).not.toHaveBeenCalled();
    workbench.state.update((value) => ({ ...value, mutating: false }));
    workbench.clearSelection();
    workbench.draft('First prompt');
    await actions.submit();
    expect(get(workbench.state).dialog).toMatchObject({
      kind: 'create',
      pendingText: 'First prompt',
    });
    expect(get(workbench.state).drafts['']).toBe('First prompt');
  });
});
