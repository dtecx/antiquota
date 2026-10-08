/** Proves composer commands, keyboard behavior and session identity using isolated HTTP fixtures. */
import { mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import type { Locator, Page } from '@playwright/test';
import { test, expect } from './fixtures';
import { addAccount } from './account-support';
import type { FixtureEngine } from '../scripts/engine.ts';
import type { Session } from '../src/gen/Session';
import type { SessionResponse } from '../src/gen/SessionResponse';
import type { EventRecord } from '../src/gen/EventRecord';

const primary = (page: Page) => page.getByRole('region', { name: 'Primary session', exact: true });
const right = (page: Page) => page.getByRole('region', { name: 'Right session', exact: true });
const prompt = (scope: Locator) => scope.getByRole('textbox', { name: 'Message', exact: true });
async function create(engine: FixtureEngine, title: string) {
  const response = await engine.call('POST', '/api/sessions', {
    title,
    cwd: engine.sandbox.project,
  });
  expect(response.status).toBe(200);
  return JSON.parse(response.text) as Session;
}
async function session(engine: FixtureEngine, id: string) {
  const response = await engine.call('GET', `/api/sessions/${id}`);
  expect(response.status).toBe(200);
  return JSON.parse(response.text) as SessionResponse;
}
async function open(page: Page, engine: FixtureEngine, id: string) {
  await page.goto(`${engine.url}/#${id}`);
  await expect(page.getByText('Engine connected')).toBeVisible();
  await expect(prompt(primary(page))).toBeEnabled();
}
async function submit(scope: Locator, text: string) {
  await prompt(scope).fill(text);
  const sent = ['/', '!', '#'].includes(text.trimStart()[0] ?? '')
    ? null
    : scope
        .page()
        .waitForResponse(
          (response) =>
            new URL(response.url()).pathname.endsWith('/send') &&
            response.request().method() === 'POST',
        );
  // Clicking Send dispatches a command rather than accepting its completion first.
  await scope.locator('button.send').click();
  if (sent) expect((await sent).status()).toBe(200);
}
async function idle(engine: FixtureEngine, id: string) {
  await expect.poll(async () => (await session(engine, id)).meta.status).toBe('idle');
}
async function calls(engine: FixtureEngine) {
  const text = await readFile(join(engine.sandbox.root, 'fake-calls.jsonl'), 'utf8');
  return text
    .split('\n')
    .filter(Boolean)
    .map((line) => JSON.parse(line) as { argv: string[] });
}
async function turnPrompts(engine: FixtureEngine) {
  return (await calls(engine))
    .filter((call) => call.argv.includes('stream-json'))
    .map((call) => call.argv[call.argv.indexOf('-p') + 1] ?? '');
}
function user(text: string, kind = 'message'): EventRecord {
  return { t: 'user', ts: 1, text, kind };
}
async function seed(engine: FixtureEngine, records: { meta: Session; events: EventRecord[] }[]) {
  const port = engine.port;
  await engine.stop();
  for (const { meta, events } of records) {
    const directory = join(engine.sandbox.data, 'sessions', meta.id);
    await writeFile(join(directory, 'meta.json'), JSON.stringify(meta));
    await writeFile(
      join(directory, 'events.jsonl'),
      [{ t: 'meta', ts: 1, meta }, ...events].map((event) => JSON.stringify(event)).join('\n') +
        '\n',
    );
  }
  await engine.start(port);
}
async function shot(page: Page, engine: FixtureEngine, name: string) {
  const directory = join(engine.sandbox.repository, '.handoff/evidence/ui-slice9d');
  await mkdir(directory, { recursive: true });
  await page.screenshot({ path: join(directory, `${name}.png`), fullPage: true });
}

test('slash reference and panel commands dispatch all settings targets without sending prompts', async ({
  page,
  engine,
}) => {
  const one = await create(engine, 'Command panels');
  await open(page, engine, one.id);
  await submit(primary(page), '/help');
  const reference = page.getByRole('dialog', { name: 'Keyboard shortcuts' });
  await expect(reference.locator('code')).toHaveCount(31);
  await expect(reference.locator('code')).toContainText(['/help', '/delete']);
  const platform = await page.evaluate(() => navigator.platform);
  await expect(reference.locator('kbd').first()).toContainText(
    /Mac|iPhone|iPad/.test(platform) ? '⌘' : 'Ctrl',
  );
  await reference.getByRole('button', { name: 'Close', exact: true }).click();
  for (const [command, section] of [
    ['/accounts', 'Accounts'],
    ['/usage', 'Usage'],
    ['/settings', 'General'],
    ['/permissions', 'Permissions'],
    ['/skills', 'Skills'],
    ['/hooks', 'Hooks'],
    ['/mcp', 'MCP servers'],
    ['/stack', 'Skills'],
  ]) {
    await submit(primary(page), command!);
    await expect(
      page.getByRole('navigation', { name: 'Settings sections' }).getByRole('button', {
        name: section!,
        exact: true,
      }),
    ).toHaveAttribute('aria-current', 'page');
    await page.getByRole('button', { name: 'Close settings' }).click();
  }
  await submit(primary(page), '/login');
  await expect(page.getByRole('dialog', { name: 'Add Google account' })).toBeVisible();
  await page.getByRole('button', { name: 'Close sign-in' }).click();
  await submit(primary(page), '/new');
  await expect(page.getByRole('dialog', { name: 'New session', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  const quotaCalls = (await calls(engine)).filter((call) => call.argv.includes('/quota')).length;
  await submit(primary(page), '/quota');
  await expect
    .poll(async () => (await calls(engine)).filter((call) => call.argv.includes('/quota')).length)
    .toBeGreaterThan(quotaCalls);
  await submit(primary(page), '/not-a-command');
  await expect(primary(page)).toContainText('Unknown command /not-a-command');
  await expect(prompt(primary(page))).toHaveValue('/not-a-command');
  expect(await turnPrompts(engine)).toEqual([]);
  await shot(page, engine, 'command-dispatch');
});

test('shell output feeds the next prompt while notes memory fallback and state remain local', async ({
  page,
  engine,
}) => {
  const one = await create(engine, 'Local prefixes');
  await open(page, engine, one.id);
  await prompt(primary(page)).fill('!printf shell-marker; pwd');
  await expect(primary(page).locator('form')).toHaveClass(/shell/);
  await expect(primary(page).locator('form')).toHaveCSS('border-top-color', 'rgb(134, 184, 139)');
  await expect(primary(page).getByRole('button', { name: 'Run', exact: true })).toBeVisible();
  await expect(primary(page)).toContainText('output goes to the next prompt');
  await shot(page, engine, 'shell-prefix');
  await primary(page).locator('button.send').click();
  await expect
    .poll(async () => (await session(engine, one.id)).meta.pending_note)
    .toContain('shell-marker');
  expect((await session(engine, one.id)).meta.pending_note).toContain(engine.sandbox.project);
  await submit(primary(page), 'Use the shell output.');
  await idle(engine, one.id);
  expect((await turnPrompts(engine)).at(-1)).toContain('shell-marker');
  expect((await session(engine, one.id)).meta.pending_note).toBe('');
  await submit(primary(page), '/memory');
  await expect(primary(page)).toContainText('No GEMINI.md yet');
  await prompt(primary(page)).fill('# Keep this synthetic convention.');
  await expect(primary(page).locator('form')).toHaveClass(/note/);
  await expect(primary(page).getByRole('button', { name: 'Save', exact: true })).toBeVisible();
  await expect(primary(page)).toContainText('Save a note to project GEMINI.md');
  await shot(page, engine, 'note-prefix');
  await primary(page).locator('button.send').click();
  await expect
    .poll(() => readFile(join(engine.sandbox.project, 'GEMINI.md'), 'utf8'))
    .toContain('Keep this synthetic convention.');
  await submit(primary(page), '# Keep this second note.');
  await expect(primary(page)).toContainText(
    `Saved to ${join(engine.sandbox.project, 'GEMINI.md')}`,
  );
  const notes = await readFile(join(engine.sandbox.project, 'GEMINI.md'), 'utf8');
  expect(notes).toContain('Keep this synthetic convention.');
  expect(notes).toContain('Keep this second note.');
  await submit(primary(page), '/memory');
  await expect(page.getByLabel('File contents')).toContainText('Keep this synthetic convention.');
  await page.getByRole('button', { name: 'Close file' }).click();
  await rm(join(engine.sandbox.project, 'GEMINI.md'));
  await writeFile(join(engine.sandbox.project, 'AGENTS.md'), 'Fallback project guidance.');
  await submit(primary(page), '/memory');
  await expect(page.getByLabel('File contents')).toContainText('Fallback project guidance.');
  await page.getByRole('button', { name: 'Close file' }).click();
  await submit(primary(page), '/state');
  await expect(page.getByRole('dialog', { name: 'handoff state · Local prefixes' })).toBeVisible();
  await expect(page.getByLabel('File contents')).toContainText('Local prefixes');
  await shot(page, engine, 'state-viewer');
  await page.getByRole('button', { name: 'Close file' }).click();
  expect((await session(engine, one.id)).events.filter((event) => event.t === 'user')).toHaveLength(
    1,
  );
});

test('clear and compact preserve durable history and start fresh conversations with a handoff', async ({
  page,
  engine,
}) => {
  const one = await create(engine, 'Recovery commands');
  await open(page, engine, one.id);
  await submit(primary(page), 'Durable original question.');
  await idle(engine, one.id);
  const before = (await session(engine, one.id)).events.filter((event) => event.t === 'user');
  await submit(primary(page), '/clear');
  await expect.poll(async () => (await session(engine, one.id)).meta.conversation_id).toBeNull();
  expect((await session(engine, one.id)).events.filter((event) => event.t === 'user')).toEqual(
    before,
  );
  await expect(primary(page).getByLabel('Your message')).toContainText(
    'Durable original question.',
  );
  await submit(primary(page), 'Question after clear.');
  await idle(engine, one.id);
  await submit(primary(page), '/compact');
  await expect
    .poll(async () => (await session(engine, one.id)).meta.pending_note)
    .toContain('Handoff');
  const compacted = await session(engine, one.id);
  expect(compacted.meta.conversation_id).toBeNull();
  expect(compacted.meta.context).toBe(0);
  expect(compacted.meta.pending_note).toContain('Handoff');
  expect(compacted.events.filter((event) => event.t === 'user')).toHaveLength(2);
  await submit(primary(page), 'Continue from the summary.');
  await idle(engine, one.id);
  expect((await turnPrompts(engine)).at(-1)).toContain('Handoff');
  expect((await turnPrompts(engine)).at(-1)).toContain('Continue from the summary.');
  expect((await session(engine, one.id)).meta.pending_note).toBe('');
  await page.reload();
  await expect(primary(page).getByLabel('Your message')).toHaveCount(3);
  await shot(page, engine, 'clear-compact-preservation');
});

test('rewind cancellation keeps files and confirmation restores checkpoints but refuses user edits', async ({
  page,
  engine,
}) => {
  const one = await create(engine, 'Checkpoint commands');
  const file = join(engine.sandbox.project, 'checkpoint.txt');
  await seed(engine, [{ meta: one, events: [user('Keep the original transcript.')] }]);
  const checkpoints = join(engine.sandbox.data, 'sessions', one.id, 'checkpoints');
  await mkdir(checkpoints, { recursive: true });
  await writeFile(
    join(checkpoints, '1.json'),
    JSON.stringify({ [file]: { before: 'original', after: 'agent edit', existed: true } }),
  );
  await writeFile(file, 'agent edit');
  await open(page, engine, one.id);
  await submit(primary(page), '/rewind');
  await expect(primary(page)).toContainText('Which turn? /rewind <turn number>');
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await submit(primary(page), '/rewind 1');
  await expect(page.getByRole('dialog', { name: 'Rewind before turn 1?' })).toContainText(
    'Later transcript entries stay',
  );
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  expect(await readFile(file, 'utf8')).toBe('agent edit');
  await writeFile(file, 'later user edit');
  await submit(primary(page), '/rewind 1');
  await page.getByRole('button', { name: 'Rewind', exact: true }).click();
  await expect(primary(page)).toContainText('rewind refused');
  await expect(primary(page).getByRole('alert')).toContainText('checkpoint.txt');
  expect(await readFile(file, 'utf8')).toBe('later user edit');
  await primary(page).getByRole('button', { name: 'Dismiss error' }).click();
  await writeFile(file, 'agent edit');
  await submit(primary(page), '/rewind 1');
  await page.getByRole('button', { name: 'Rewind', exact: true }).click();
  await expect.poll(() => readFile(file, 'utf8')).toBe('original');
  await expect(primary(page).getByLabel('Your message')).toContainText(
    'Keep the original transcript.',
  );
  expect((await session(engine, one.id)).events.filter((event) => event.t === 'user')).toHaveLength(
    1,
  );
  await shot(page, engine, 'rewind-preserves-user-edits');
});

test('export downloads full durable Markdown with roles thinking tools notices and a safe filename', async ({
  page,
  engine,
}) => {
  const one = await create(engine, '../ Export / ' + 'long-title-'.repeat(8));
  await seed(engine, [
    {
      meta: one,
      events: [
        user('Repeated durable input.'),
        user('Repeated durable input.'),
        user('Synthetic goal.', 'goal'),
        { t: 'turn_start', ts: 2, turn: 1, slot: 'one@example.com', model: one.model },
        { t: 'thinking', ts: 3, turn: 1, step: 0, text: 'Full reasoning fixture.', tokens: 3 },
        { t: 'assistant', ts: 4, turn: 1, step: 1, text: 'Full answer fixture.' },
        {
          t: 'tool',
          ts: 5,
          turn: 1,
          step: 2,
          name: 'view_file',
          params: { AbsolutePath: join(engine.sandbox.project, 'fixture.txt') },
          state: 'error',
        },
        {
          t: 'turn_end',
          ts: 6,
          turn: 1,
          slot: 'one@example.com',
          status: 'completed',
          usage: {
            input_tokens: 1,
            output_tokens: 2,
            thinking_tokens: 3,
            total_tokens: 6,
            context: 6,
          },
          secs: 0.1,
        },
        { t: 'notice', ts: 7, kind: 'fixture', text: 'Durable notice fixture.', data: null },
      ],
    },
  ]);
  await open(page, engine, one.id);
  const downloaded = page.waitForEvent('download');
  await submit(primary(page), '/export');
  const download = await downloaded;
  expect(download.suggestedFilename()).toMatch(/^[\w.-]{1,60}\.md$/);
  expect(download.suggestedFilename()).not.toContain('/');
  const stream = await download.createReadStream();
  expect(stream).not.toBeNull();
  const chunks: Buffer[] = [];
  for await (const chunk of stream!) chunks.push(Buffer.from(chunk));
  const markdown = Buffer.concat(chunks).toString('utf8');
  expect(markdown).toContain(`# ${one.title}`);
  expect(markdown.match(/Repeated durable input\./g)).toHaveLength(2);
  for (const text of [
    '## you',
    '## goal',
    '<details><summary>Thought</summary>',
    'Full reasoning fixture.',
    'Full answer fixture.',
    '`view_file` fixture.txt (error)',
    'turn 1 ended · completed',
    '> **fixture**: Durable notice fixture.',
  ])
    expect(markdown).toContain(text);
  expect(await turnPrompts(engine)).toEqual([]);
  await shot(page, engine, 'durable-export');
});

test('background commands expose their log and stop while loops and daily schedules persist and clear', async ({
  page,
  engine,
}) => {
  const one = await create(engine, 'Scheduled commands');
  await open(page, engine, one.id);
  await submit(primary(page), '/bg');
  await expect(primary(page)).toContainText('/bg <command>');
  const command = "printf 'line one\\nline two\\n'; sleep 30";
  await submit(primary(page), `/bg ${command}`);
  const job = primary(page).getByRole('region', { name: `Background job ${command}` });
  await expect(job).toBeVisible();
  await job.getByRole('button', { name: 'Log', exact: true }).click();
  await expect(page.getByLabel('Job output')).toContainText('line one');
  await expect(page.getByLabel('Job output')).toContainText('line two');
  await page.getByRole('button', { name: 'Close job log' }).click();
  await job.getByRole('button', { name: 'Stop', exact: true }).click();
  await expect(job).toHaveCount(0);
  await submit(primary(page), '/loop 15m Periodic synthetic prompt.');
  await expect(primary(page).getByRole('region', { name: 'Loop', exact: true })).toContainText(
    'every 15m',
  );
  await expect
    .poll(async () =>
      (await turnPrompts(engine)).some((text) => text.includes('Periodic synthetic prompt.')),
    )
    .toBe(true);
  await idle(engine, one.id);
  await page.reload();
  await expect(primary(page).getByRole('region', { name: 'Loop', exact: true })).toBeVisible();
  await submit(primary(page), '/loop off');
  await expect(primary(page).getByRole('region', { name: 'Loop', exact: true })).toHaveCount(0);
  await submit(primary(page), '/schedule 03:00 Daily synthetic prompt.');
  await expect(primary(page).getByRole('region', { name: 'Daily schedule' })).toContainText(
    'daily at 03:00',
  );
  await page.reload();
  await expect(primary(page).getByRole('region', { name: 'Daily schedule' })).toBeVisible();
  await submit(primary(page), '/schedule off');
  await expect(primary(page).getByRole('region', { name: 'Daily schedule' })).toHaveCount(0);
  expect((await session(engine, one.id)).meta.loop).toBeNull();
  await shot(page, engine, 'jobs-loops-schedules');
});

test('goal and init commands use explicit objectives pause resume clear and preserve project guidance', async ({
  page,
  engine,
}) => {
  const one = await create(engine, 'Goal commands');
  await engine.scenario({ delay_ms: 30000, reply: 'Delayed synthetic reply.' });
  await writeFile(join(engine.sandbox.project, 'GEMINI.md'), 'User project guidance.');
  await open(page, engine, one.id);
  await submit(primary(page), '/goal');
  await expect(page.getByRole('dialog', { name: 'Goal', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  expect(await turnPrompts(engine)).toEqual([]);
  await submit(primary(page), '/goal Complete this synthetic objective.');
  await expect(primary(page).getByRole('region', { name: 'Goal', exact: true })).toContainText(
    'Complete this synthetic objective.',
  );
  await expect.poll(async () => (await session(engine, one.id)).meta.status).toBe('running');
  await submit(primary(page), '/stop');
  await idle(engine, one.id);
  expect((await session(engine, one.id)).meta.goal?.state).toBe('paused');
  await submit(primary(page), '/resume');
  await expect.poll(async () => (await session(engine, one.id)).meta.status).toBe('running');
  await submit(primary(page), '/stop');
  await idle(engine, one.id);
  await submit(primary(page), '/cleargoal');
  await expect(primary(page).getByRole('region', { name: 'Goal', exact: true })).toHaveCount(0);
  await submit(primary(page), '/init Also document the fixture scripts.');
  await expect
    .poll(async () => (await turnPrompts(engine)).at(-1))
    .toContain('Study this project and write a concise GEMINI.md');
  expect((await turnPrompts(engine)).at(-1)).toContain('Also document the fixture scripts.');
  await submit(primary(page), '/stop');
  await idle(engine, one.id);
  expect(await readFile(join(engine.sandbox.project, 'GEMINI.md'), 'utf8')).toBe(
    'User project guidance.',
  );
  await shot(page, engine, 'goal-command-controls');
});

test('model families preserve effort expose pools and unknown models and require bypass confirmation', async ({
  page,
  engine,
}) => {
  const one = await create(engine, 'Model commands');
  const models = [
    { id: 'gemini-3.8-flash-low', label: 'Gemini Flash (low)' },
    { id: 'gemini-3.8-flash-high', label: 'Gemini Flash (high)' },
    { id: 'gemini-pro-low', label: 'Gemini Pro (low)' },
    { id: 'gemini-pro-high', label: 'Gemini Pro (high)' },
    { id: 'claude-opus', label: 'Claude Opus' },
    { id: 'gpt-oss', label: 'GPT OSS' },
  ];
  const port = engine.port;
  await engine.stop();
  await engine.scenario({ metadata: { models } });
  await writeFile(join(engine.sandbox.data, 'state', 'models.json'), JSON.stringify(models));
  await engine.start(port);
  await open(page, engine, one.id);
  const family = primary(page).getByRole('combobox', { name: 'Model family', exact: true });
  const effort = primary(page).getByRole('combobox', { name: 'Effort', exact: true });
  expect(
    await family
      .locator('optgroup')
      .evaluateAll((groups) => groups.map((group) => group.getAttribute('label'))),
  ).toEqual(['Gemini', 'Claude / GPT']);
  await effort.selectOption('gemini-3.8-flash-low');
  await family.selectOption('Gemini Pro');
  await expect(effort).toHaveValue('gemini-pro-low');
  await submit(primary(page), '/effort high');
  await expect(effort).toHaveValue('gemini-pro-high');
  await submit(primary(page), '/effort impossible');
  await expect(primary(page)).toContainText('has no effort "impossible"');
  await family.selectOption('Claude Opus');
  await expect(effort).toBeDisabled();
  await submit(primary(page), '/model provider-future-model');
  await expect(family).toHaveValue('provider-future-model');
  expect((await session(engine, one.id)).meta.model).toBe('provider-future-model');
  const mode = primary(page).getByRole('combobox', { name: 'Permission mode', exact: true });
  await mode.selectOption('plan');
  await expect.poll(async () => (await session(engine, one.id)).meta.mode).toBe('plan');
  for (const value of ['accept-edits', 'plan', 'default']) {
    await submit(primary(page), `/mode ${value}`);
    await expect(mode).toHaveValue(value);
  }
  await mode.selectOption('bypass');
  const bypass = page.getByRole('dialog', { name: 'Allow bypass mode?' });
  await expect(bypass).toContainText('Bypass ignores antiquota permission rules');
  await bypass.getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(mode).toHaveValue('default');
  expect((await session(engine, one.id)).meta.mode).toBe('default');
  await submit(primary(page), '/mode bypass');
  await bypass.getByRole('button', { name: 'Use bypass', exact: true }).click();
  await expect(mode).toHaveValue('bypass');
  await submit(primary(page), '/rename Renamed through command');
  await expect(
    primary(page).getByRole('heading', { name: 'Renamed through command' }),
  ).toBeVisible();
  await submit(primary(page), '/goal');
  expect(
    await page
      .getByRole('combobox', { name: 'Verifier model' })
      .locator('option')
      .evaluateAll((options) => options.map((option) => (option as HTMLOptionElement).value)),
  ).toEqual(expect.arrayContaining(models.map((model) => model.id)));
  await expect(
    page
      .getByRole('combobox', { name: 'Verifier model' })
      .locator('option[value="gemini-3.8-flash-low"]'),
  ).toHaveText('Gemini Flash · low · gemini-3.8-flash-low');
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  expect(await turnPrompts(engine)).toEqual([]);
  await shot(page, engine, 'model-effort-permissions');
});

test('inline title commits on Enter or blur cancels on Escape and leaves empty edits unchanged', async ({
  page,
  engine,
}) => {
  const one = await create(engine, 'Inline identity');
  await open(page, engine, one.id);
  const edit = primary(page).getByRole('button', { name: 'Rename session', exact: true });
  const title = primary(page).getByRole('textbox', { name: 'Session title' });
  await edit.click();
  await expect(title).toBeFocused();
  await title.fill('  Enter title  ');
  await title.press('Enter');
  await expect(
    primary(page).getByRole('heading', { name: 'Enter title', exact: true }),
  ).toBeVisible();
  await edit.click();
  await title.fill('Blur title');
  await prompt(primary(page)).click();
  await expect(
    primary(page).getByRole('heading', { name: 'Blur title', exact: true }),
  ).toBeVisible();
  await edit.click();
  await title.fill('Cancelled title');
  await title.press('Escape');
  await expect(
    primary(page).getByRole('heading', { name: 'Blur title', exact: true }),
  ).toBeVisible();
  await edit.click();
  await title.fill('   ');
  await title.press('Enter');
  expect((await session(engine, one.id)).meta.title).toBe('Blur title');
  await expect(primary(page).locator('header .identity p')).toHaveAttribute(
    'title',
    engine.sandbox.project,
  );
  await expect(primary(page).locator('header .identity p')).toContainText('~/Desktop/test/');
  await shot(page, engine, 'inline-title');
});

test('header and meter render status queue usage worktree paths pool quota and model capacities', async ({
  page,
  engine,
}) => {
  const one = await create(engine, 'Metadata fixture');
  await addAccount(engine, 'one@example.com');
  let fields: Partial<Session> = {
    status: 'running',
    model: 'claude-opus',
    context: 100000,
    usage: { input: 1200, output: 2400, thinking: 3600 },
    queue: [{ text: 'queued one' }, { text: 'queued two' }],
    last_slot: 'one@example.com',
    worktree: {
      repo: engine.sandbox.project,
      path: join(engine.sandbox.root, 'synthetic-worktree'),
      branch: 'fixture-branch',
      base: 'main',
      base_sha: 'synthetic',
    },
  };
  await page.route(`**/api/sessions/${one.id}`, async (route) => {
    const response = await route.fetch();
    const body = (await response.json()) as SessionResponse;
    body.meta = { ...body.meta, ...fields };
    await route.fulfill({ json: body });
  });
  await page.route('**/api/state', async (route) => {
    const response = await route.fetch();
    const body = await response.json();
    body.accounts = body.accounts.map((account: import('../src/gen/Account').Account) => ({
      ...account,
      quota: { ...account.quota, cg: { five: [0.5, null], weekly: [0.7, 4071427200] } },
    }));
    body.sessions = body.sessions.map((item: Session) =>
      item.id === one.id ? { ...item, ...fields } : item,
    );
    await route.fulfill({ json: body });
  });
  await open(page, engine, one.id);
  const header = primary(page).locator('header');
  const meter = primary(page).locator('.meter');
  await expect(header).toContainText('working');
  await expect(header).toContainText('2 queued');
  await expect(header).toContainText('1.2k in · 2.4k out');
  await expect(page).toHaveTitle('● Metadata fixture · antiquota');
  await expect(header.locator('.identity p')).toHaveText('fixture-branch');
  await expect(header.locator('.identity p')).toHaveAttribute(
    'title',
    `${join(engine.sandbox.root, 'synthetic-worktree')} · from main in ${engine.sandbox.project}`,
  );
  await expect(meter).toHaveClass(/warm/);
  await expect(meter).toContainText('50% context');
  await expect(meter).toContainText('1.2k in / 2.4k out / 3.6k thinking');
  await expect(meter).toContainText('one · Claude/GPT 50%');
  await expect(meter).toContainText('1 accounts free');
  for (const [status, label] of [
    ['approval', 'needs your approval'],
    ['waiting', 'waiting for an account'],
    ['checking', 'running check'],
    ['verifying', 'verifier reviewing'],
    ['sleeping', 'sleeping until'],
  ]) {
    fields = { ...fields, status: status!, status_until: Date.now() / 1000 + 3600 };
    await page.reload();
    await expect(header.locator('.status')).toContainText(label!);
  }
  fields = { ...fields, status: 'idle', context: 180000 };
  await page.reload();
  await expect(header.locator('.status')).toHaveCount(0);
  await expect(page).toHaveTitle('Metadata fixture · antiquota');
  await expect(meter).toHaveClass(/hot/);
  await expect(meter).toContainText('90% context');
  for (const [model, context] of [
    ['gpt-oss-fixture', 65536],
    ['gemini-fixture', 524288],
  ] as const) {
    fields = { ...fields, model, context };
    await page.reload();
    await expect(meter).toContainText('50% context');
    await expect(meter).toHaveClass(/warm/);
  }
  await shot(page, engine, 'header-meter');
});

test('history recalls only explicit durable prompts clamps oldest restores drafts and isolates sessions', async ({
  page,
  engine,
}) => {
  const one = await create(engine, 'History A');
  const two = await create(engine, 'History B');
  await seed(engine, [
    {
      meta: one,
      events: [
        user('First explicit prompt.'),
        user('Generated objective.', 'goal'),
        user('Generated job log.', 'job'),
        user('Second explicit prompt.', 'user'),
      ],
    },
    { meta: two, events: [user('Other session prompt.')] },
  ]);
  await open(page, engine, one.id);
  const input = prompt(primary(page));
  await input.fill('Saved unsent draft.');
  await input.press('Home');
  await input.press('ArrowUp');
  await expect(input).toHaveValue('Second explicit prompt.');
  await input.press('ArrowUp');
  await expect(input).toHaveValue('First explicit prompt.');
  await input.press('ArrowUp');
  await expect(input).toHaveValue('First explicit prompt.');
  await input.press('ArrowDown');
  await expect(input).toHaveValue('Second explicit prompt.');
  await input.press('ArrowDown');
  await expect(input).toHaveValue('Saved unsent draft.');
  await input.fill('Edited draft.');
  await input.press('ArrowDown');
  await expect(input).toHaveValue('Edited draft.');
  await page.locator('button.session').filter({ hasText: 'History B' }).click();
  await expect(input).toBeEnabled();
  await input.press('ArrowUp');
  await expect(input).toHaveValue('Other session prompt.');
  await page.locator('button.session').filter({ hasText: 'History A' }).click();
  await expect(input).toHaveValue('Edited draft.');
  await input.fill('');
  await input.press('ArrowUp');
  await expect(input).toHaveValue('Second explicit prompt.');
  await input.fill('New explicit prompt.');
  await input.press('Enter');
  await idle(engine, one.id);
  await input.press('ArrowUp');
  await expect(input).toHaveValue('New explicit prompt.');
  await page.reload();
  await expect(input).toBeEnabled();
  await input.press('ArrowUp');
  await expect(input).toHaveValue('New explicit prompt.');
  await shot(page, engine, 'durable-prompt-history');
});

test('file completions discard late queries insert at the caret and dismiss after the blur grace period', async ({
  page,
  engine,
}) => {
  const one = await create(engine, 'Mention completion');
  await writeFile(join(engine.sandbox.project, 'old.txt'), 'Old synthetic file.');
  await writeFile(join(engine.sandbox.project, 'new.txt'), 'New synthetic file.');
  let releaseOld = () => {};
  const oldPending = new Promise<void>((resolve) => (releaseOld = resolve));
  let requestedOld = false;
  let fulfilledOld = false;
  await page.route(`**/api/sessions/${one.id}/files?*`, async (route) => {
    const response = await route.fetch();
    if (new URL(route.request().url()).searchParams.get('q') === 'old') {
      requestedOld = true;
      await oldPending;
    }
    await route.fulfill({ response });
    if (new URL(route.request().url()).searchParams.get('q') === 'old') fulfilledOld = true;
  });
  await open(page, engine, one.id);
  const input = prompt(primary(page));
  await input.fill('Inspect @old');
  await expect.poll(() => requestedOld).toBe(true);
  await input.fill('Inspect @new');
  await expect(primary(page).getByRole('option', { name: 'new.txt file' })).toBeVisible();
  releaseOld();
  await expect.poll(() => fulfilledOld).toBe(true);
  await expect(primary(page).getByRole('option', { name: 'old.txt file' })).toHaveCount(0);
  await input.press('Tab');
  await expect(input).toHaveValue('Inspect @new.txt ');
  expect(await input.evaluate((element: HTMLTextAreaElement) => element.selectionStart)).toBe(17);
  await input.fill('Inspect @new suffix');
  await input.evaluate((element: HTMLTextAreaElement) => {
    element.setSelectionRange(12, 12);
    element.dispatchEvent(new Event('input', { bubbles: true }));
  });
  await expect(primary(page).getByRole('option', { name: 'new.txt file' })).toBeVisible();
  await primary(page).getByRole('option', { name: 'new.txt file' }).click();
  await expect(input).toHaveValue('Inspect @new.txt  suffix');
  await page.clock.install();
  await page.clock.pauseAt(Date.now() + 1000);
  await input.fill('@new');
  await expect(primary(page).getByRole('listbox', { name: 'Composer suggestions' })).toBeVisible();
  await page.getByRole('textbox', { name: 'Filter sessions' }).focus();
  await page.clock.runFor(99);
  await expect(primary(page).getByRole('listbox')).toBeVisible();
  await page.clock.runFor(1);
  await expect(primary(page).getByRole('listbox')).toHaveCount(0);
  expect(await turnPrompts(engine)).toEqual([]);
  await shot(page, engine, 'file-completion');
});

test('slash completion wraps accepts Tab and Enter respects Escape IME and multiline sizing', async ({
  page,
  engine,
}) => {
  const one = await create(engine, 'Keyboard completion');
  await open(page, engine, one.id);
  const input = prompt(primary(page));
  await input.fill('/');
  const options = primary(page).getByRole('listbox').getByRole('option');
  await expect(options).toHaveCount(31);
  await input.press('ArrowUp');
  await expect(options.last()).toHaveAttribute('aria-selected', 'true');
  await input.press('ArrowDown');
  await expect(options.first()).toHaveAttribute('aria-selected', 'true');
  await input.press('Tab');
  await expect(input).toHaveValue('/help ');
  await expect(page.getByRole('dialog', { name: 'Keyboard shortcuts' })).not.toBeVisible();
  await input.fill('/mem');
  await input.press('Enter');
  await expect(input).toHaveValue('/memory ');
  expect(await turnPrompts(engine)).toEqual([]);
  await input.fill('/');
  await input.press('Escape');
  await expect(primary(page).getByRole('listbox')).toHaveCount(0);
  await expect(input).toHaveValue('/');
  await input.fill('Composing text');
  await input.dispatchEvent('keydown', { key: 'Enter', code: 'Enter', isComposing: true });
  await expect(input).toHaveValue('Composing text');
  expect((await session(engine, one.id)).events.filter((event) => event.t === 'user')).toHaveLength(
    0,
  );
  await input.press('Shift+Enter');
  await expect(input).toHaveValue('Composing text\n');
  await input.fill(Array.from({ length: 40 }, (_, index) => `Line ${index}`).join('\n'));
  await expect.poll(async () => (await input.boundingBox())!.height).toBeGreaterThan(150);
  const tall = await input.boundingBox();
  expect(tall!.height).toBeLessThanOrEqual(330);
  expect(tall!.height).toBeGreaterThan(150);
  await input.fill('Short draft.');
  await expect.poll(async () => (await input.boundingBox())!.height).toBeLessThan(tall!.height);
  const short = await input.boundingBox();
  expect(short!.height).toBeLessThan(tall!.height);
  await shot(page, engine, 'slash-completion-prefixes');
});

test('global modifier shortcuts target the active pane and draft creation and deletion require intent', async ({
  page,
  engine,
}) => {
  test.setTimeout(60000);
  await addAccount(engine, 'two@example.com');
  await engine.scenario({ delay_ms: 30000, reply: 'Delayed synthetic reply.' });
  await create(engine, 'Older shortcut session');
  const one = await create(engine, 'Shortcut left');
  const two = await create(engine, 'Shortcut right');
  await open(page, engine, one.id);
  await primary(page).getByRole('button', { name: 'Split view', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Shortcut right', exact: true }).click();
  const leftInput = prompt(primary(page));
  const rightInput = prompt(right(page));
  await rightInput.fill('Right draft.');
  await page.keyboard.press('Meta+k');
  await expect(page.getByRole('textbox', { name: 'Filter sessions' })).toBeFocused();
  await page.keyboard.press('Meta+j');
  await expect(rightInput).toBeFocused();
  await page.keyboard.press('Meta+b');
  await expect(page.getByRole('complementary', { name: 'Side panel' })).toBeVisible();
  await page.keyboard.press('Meta+b');
  await expect(page.getByRole('complementary', { name: 'Side panel' })).toHaveCount(0);
  await page.keyboard.press('Meta+Shift+b');
  await expect(page.getByRole('complementary', { name: 'Side panel' })).toHaveCount(0);
  await page.keyboard.press('Meta+/');
  await expect(page.getByRole('dialog', { name: 'Keyboard shortcuts' })).toBeVisible();
  await page.getByRole('button', { name: 'Close', exact: true }).click();
  await page.keyboard.press('Meta+,');
  await expect(page.getByRole('heading', { name: 'General', exact: true })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('button', { name: 'Close settings' })).toHaveCount(0);
  await page.keyboard.press('Meta+n');
  await expect(page.getByRole('dialog', { name: 'New session', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  await submit(primary(page), 'Long left turn.');
  await expect.poll(async () => (await session(engine, one.id)).meta.status).toBe('running');
  await submit(right(page), 'Long right turn.');
  await expect.poll(async () => (await session(engine, two.id)).meta.status).toBe('running');
  await page.keyboard.press('Meta+.');
  await idle(engine, two.id);
  expect((await session(engine, one.id)).meta.status).toBe('running');
  await leftInput.focus();
  await leftInput.press('Escape');
  await idle(engine, one.id);
  await right(page).getByRole('button', { name: 'Close side by side' }).click();
  const keep = join(engine.sandbox.project, 'keep-user-file.txt');
  await writeFile(keep, 'Project file survives session deletion.');
  await submit(primary(page), '/delete');
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  expect((await engine.call('GET', `/api/sessions/${one.id}`)).status).toBe(200);
  await submit(primary(page), '/delete');
  await page.getByRole('button', { name: 'Delete', exact: true }).click();
  await expect(primary(page).getByRole('heading', { name: 'Shortcut right' })).toBeVisible();
  expect((await engine.call('GET', `/api/sessions/${one.id}`)).status).toBe(404);
  expect(await readFile(keep, 'utf8')).toBe('Project file survives session deletion.');
  await shot(page, engine, 'active-pane-shortcuts');
  // Starting a draft is independent of creating a durable session.
  await submit(primary(page), '/delete');
  await page.getByRole('button', { name: 'Delete', exact: true }).click();
  await expect(
    primary(page).getByRole('heading', { name: 'Older shortcut session' }),
  ).toBeVisible();
  await submit(primary(page), '/delete');
  await page.getByRole('button', { name: 'Delete', exact: true }).click();
  await expect(primary(page).getByRole('heading', { name: 'New session' })).toBeVisible();
  await prompt(primary(page)).fill('Uncreated draft.');
  await prompt(primary(page)).press('Enter');
  await expect(page.getByRole('dialog', { name: 'New session', exact: true })).toBeVisible();
  expect(JSON.parse((await engine.call('GET', '/api/state')).text).sessions).toHaveLength(0);
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(prompt(primary(page))).toHaveValue('Uncreated draft.');
  await prompt(primary(page)).press('Enter');
  await page.getByRole('textbox', { name: 'Session name' }).fill('Explicitly created');
  await page.getByRole('textbox', { name: 'Project folder' }).fill(engine.sandbox.project);
  await page.getByRole('button', { name: 'Create session', exact: true }).click();
  await expect(primary(page).getByLabel('Your message')).toContainText('Uncreated draft.');
  await expect(prompt(primary(page))).toHaveValue('');
  expect(JSON.parse((await engine.call('GET', '/api/state')).text).sessions).toHaveLength(1);
});
