/** Proves remaining browser edges and asynchronous ownership with isolated engines. */
import { appendFile, mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { test, expect } from './fixtures';
import type { FixtureEngine } from '../scripts/engine';
import type { Page } from '@playwright/test';
import type { Session } from '../src/gen/Session';
import type { EventRecord } from '../src/gen/EventRecord';
import { EXECUTE_PLAN } from '../src/lib/goals';
const primary = (page: Page) => page.getByRole('region', { name: 'Primary session', exact: true });
const prompt = (page: Page) => primary(page).getByRole('textbox', { name: 'Message', exact: true });
async function create(engine: FixtureEngine, title: string, extra: object = {}) {
  const response = await engine.call('POST', '/api/sessions', {
    title,
    cwd: engine.sandbox.project,
    ...extra,
  });
  expect(response.status).toBe(200);
  return JSON.parse(response.text) as Session;
}
async function open(page: Page, engine: FixtureEngine, id = '') {
  const before = page.url(),
    url = `${engine.url}/#${id}`;
  await page.goto(url);
  if (before.split('#')[0] === url.split('#')[0]) await page.reload();
  await expect(page.getByText('Engine connected')).toBeVisible();
  await expect(prompt(page)).toBeEnabled();
}
async function history(engine: FixtureEngine, id: string, events: EventRecord[]) {
  const port = engine.port;
  await engine.stop();
  await appendFile(
    join(engine.sandbox.data, 'sessions', id, 'events.jsonl'),
    events.map((e) => JSON.stringify(e)).join('\n') + '\n',
  );
  await engine.start(port);
}
async function shot(page: Page, engine: FixtureEngine, name: string) {
  const directory = join(engine.sandbox.repository, '.handoff/evidence/ui-slice9f');
  await mkdir(directory, { recursive: true });
  await page.screenshot({ path: join(directory, `${name}.png`), fullPage: true });
}
const tool = (step: number, path: string, diff: string): EventRecord => ({
  t: 'tool',
  ts: step,
  turn: 1,
  step,
  name: 'replace_file_content',
  state: 'done',
  params: { TargetFile: path },
  diff,
});

test('empty-session earlier choices inherit the selected handoff into exactly the next prompt', async ({
  page,
  engine,
}) => {
  await engine.scenario({ reply: 'Earlier work evidence.', delay_ms: 20 });
  const previous: Session[] = [];
  for (let i = 0; i < 4; i++) {
    const row = await create(engine, `Earlier ${i}`);
    previous.push(row);
    await engine.call('POST', `/api/sessions/${row.id}/send`, { text: `Earlier request ${i}` });
    await expect
      .poll(
        async () =>
          JSON.parse((await engine.call('GET', `/api/sessions/${row.id}`)).text).meta.status,
      )
      .toBe('idle');
  }
  const target = await create(engine, 'Empty target');
  await open(page, engine, target.id);
  const earlier = primary(page).getByLabel('Earlier sessions');
  await expect(earlier.getByRole('button')).toHaveCount(3);
  await expect(earlier.getByRole('button')).toContainText(['Earlier 3', 'Earlier 2', 'Earlier 1']);
  await expect(primary(page).getByText(`Commands run in ${engine.sandbox.project}.`)).toBeVisible();
  await expect(
    primary(page).getByRole('button', { name: 'Set a goal', exact: true }),
  ).toBeVisible();
  await earlier.getByRole('button', { name: 'Earlier 3', exact: true }).click();
  await expect(
    page.getByRole('status').filter({ hasText: 'Picks up from Earlier 3' }).first(),
  ).toBeVisible();
  await prompt(page).fill('Use inherited evidence.');
  await primary(page).locator('button.send').click();
  await expect
    .poll(
      async () =>
        JSON.parse((await engine.call('GET', `/api/sessions/${target.id}`)).text).meta.status,
    )
    .toBe('idle');
  const calls = (await readFile(join(engine.sandbox.root, 'fake-calls.jsonl'), 'utf8'))
    .trim()
    .split('\n')
    .map((line) => JSON.parse(line) as { argv: string[] });
  const prompts = calls
    .filter((call) => call.argv.includes('stream-json'))
    .map((call) => call.argv[call.argv.indexOf('-p') + 1]);
  expect(prompts.at(-1)).toContain('Earlier request 3');
  expect(prompts.at(-1)).toContain('Earlier work evidence.');
  expect(prompts.at(-1)).toContain('Use inherited evidence.');
  await shot(page, engine, 'inherited-empty-session');
});

test('file viewer opens inclusive line ranges centers them copies complete text and closes', async ({
  page,
  engine,
}) => {
  await page.context().grantPermissions(['clipboard-read', 'clipboard-write']);
  const one = await create(engine, 'File range');
  const text = Array.from({ length: 180 }, (_, i) => `line ${i + 1}`).join('\n') + '\n';
  await writeFile(join(engine.sandbox.project, 'range.txt'), text);
  await history(engine, one.id, [
    { t: 'user', ts: 1, kind: 'message', text: 'Open range' },
    {
      t: 'assistant',
      ts: 2,
      turn: 1,
      step: 2,
      text: `[Middle](file://${join(engine.sandbox.project, 'range.txt')}#L100-L103)`,
    },
  ]);
  await open(page, engine, one.id);
  await page.getByRole('link', { name: 'Middle', exact: true }).click();
  const viewer = page.getByRole('dialog', { name: 'range.txt', exact: true });
  await expect(viewer.locator('.line.highlight')).toHaveCount(4);
  await expect(viewer.locator('.line.highlight .number')).toHaveText(['100', '101', '102', '103']);
  await expect(viewer.getByRole('heading')).toHaveAttribute(
    'title',
    join(engine.sandbox.project, 'range.txt'),
  );
  const centered = await viewer
    .locator('.line.highlight')
    .first()
    .evaluate((node) => {
      const body = node.closest('.body')!;
      const rect = node.getBoundingClientRect(),
        bounds = body.getBoundingClientRect();
      return (
        rect.top >= bounds.top &&
        rect.bottom <= bounds.bottom &&
        body.scrollTop > 0 &&
        Math.abs((rect.top + rect.bottom) / 2 - (bounds.top + bounds.bottom) / 2) < rect.height * 2
      );
    });
  expect(centered).toBe(true);
  await viewer.getByRole('button', { name: 'Copy', exact: true }).click();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(text);
  await shot(page, engine, 'file-range');
  await viewer.getByRole('button', { name: 'Close file' }).click();
  await expect(viewer).not.toBeVisible();
});

test('edit history stays lazy until expanded and side-panel visibility and selection persist safely', async ({
  page,
  engine,
}) => {
  const one = await create(engine, 'Old edits'),
    two = await create(engine, 'Other edits');
  await history(engine, one.id, [
    tool(1, 'one.txt', '+first edit'),
    tool(2, 'one.txt', '+second edit'),
  ]);
  await history(engine, two.id, [tool(1, 'one.txt', '+other-session edit')]);
  await open(page, engine, one.id);
  await page.keyboard.press('ControlOrMeta+b');
  const panel = page.getByRole('complementary', { name: 'Side panel' });
  const edits = panel.getByRole('list', { name: 'Agent edits' });
  await expect(edits.locator('pre')).toHaveCount(0);
  await edits.locator('summary').click();
  await expect(edits.locator('pre')).toHaveText(['+first edit', '+second edit']);
  await page.reload();
  await expect(panel).toBeVisible();
  await page
    .getByRole('button', { name: /Other edits/ })
    .first()
    .click();
  await expect(edits.locator('details')).not.toHaveAttribute('open');
  await expect(edits.locator('pre')).toHaveCount(0);
  await edits.locator('summary').click();
  await expect(edits.locator('pre')).toHaveText(['+other-session edit']);
  await panel.getByRole('button', { name: 'Close side panel' }).click();
  await page.reload();
  await expect(panel).toHaveCount(0);
  await shot(page, engine, 'lazy-edit-history');
  expect(one.id).not.toBe(two.id);
});

test('stale git status and confirmation cannot discard into a replacement session', async ({
  page,
  engine,
}) => {
  const one = await create(engine, 'Old repository'),
    two = await create(engine, 'Replacement repository');
  let release: (() => void) | undefined;
  await page.route(`**/api/sessions/${one.id}/git`, async (route) => {
    await new Promise<void>((resolve) => {
      release = resolve;
    });
    await route.fulfill({
      json: {
        root: engine.sandbox.project,
        branch: 'old',
        files: [{ path: 'old.txt', code: '??', add: 1, del: 0 }],
        worktree: null,
      },
    });
  });
  await page.route(`**/api/sessions/${two.id}/git`, (route) =>
    route.fulfill({
      json: {
        root: engine.sandbox.project,
        branch: 'replacement',
        files: [{ path: 'new.txt', code: '??', add: 1, del: 0 }],
        worktree: null,
      },
    }),
  );
  await open(page, engine, one.id);
  await page.keyboard.press('ControlOrMeta+b');
  await expect.poll(() => !!release).toBe(true);
  await page
    .getByRole('button', { name: /Replacement repository/ })
    .first()
    .click();
  const panel = page.getByRole('complementary', { name: 'Side panel' });
  await expect(panel.getByText('new.txt', { exact: true })).toBeVisible();
  release!();
  await expect(panel.getByText('old.txt', { exact: true })).toHaveCount(0);
  await panel.getByRole('list', { name: 'Uncommitted files' }).locator('summary').click();
  await panel.getByRole('button', { name: 'Discard', exact: true }).click();
  const confirm = page.getByRole('dialog', { name: 'Discard new.txt?' });
  await expect(confirm.getByRole('button', { name: 'Discard', exact: true })).toHaveClass(/danger/);
  const discards: string[] = [];
  page.on('request', (r) => {
    if (r.url().includes('/git/discard')) discards.push(r.url());
  });
  await engine.call('DELETE', `/api/sessions/${two.id}`);
  await expect(
    primary(page).getByRole('heading', { name: 'Old repository', exact: true }),
  ).toBeVisible();
  await confirm.getByRole('button', { name: 'Discard', exact: true }).click();
  expect(discards).toEqual([]);
  await shot(page, engine, 'discard-owner-guard');
});

test('confirmation supports Cancel and Escape and approval execution patches mode before sending', async ({
  page,
  engine,
}) => {
  const one = await create(engine, 'Plan approvals', { mode: 'plan' });
  await history(engine, one.id, [
    { t: 'user', ts: 1, kind: 'message', text: 'Make a plan' },
    {
      t: 'turn_end',
      ts: 2,
      turn: 1,
      slot: 'one@example.com',
      status: 'SUCCESS',
      secs: 1,
      usage: { input_tokens: 0, output_tokens: 0, thinking_tokens: 0, total_tokens: 0, context: 0 },
    },
  ]);
  await open(page, engine, one.id);
  const mode = primary(page).getByRole('combobox', { name: 'Permission mode', exact: true });
  await mode.selectOption('bypass');
  const confirm = page.getByRole('dialog', { name: 'Allow bypass mode?' });
  await confirm.getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(mode).toHaveValue('plan');
  await mode.selectOption('bypass');
  await page.keyboard.press('Escape');
  await expect(confirm).not.toBeVisible();
  await expect(mode).toHaveValue('plan');
  const requests: { path: string; method: string; body: unknown }[] = [];
  page.on('request', (r) => {
    if (['PATCH', 'POST'].includes(r.method()))
      requests.push({
        path: new URL(r.url()).pathname,
        method: r.method(),
        body: r.postDataJSON(),
      });
  });
  await primary(page).getByRole('button', { name: 'Execute with approvals', exact: true }).click();
  await expect.poll(() => requests.some((r) => r.path.endsWith('/send'))).toBe(true);
  expect(requests[0]).toMatchObject({
    path: `/api/sessions/${one.id}`,
    method: 'PATCH',
    body: { mode: 'default' },
  });
  expect(requests[1]).toMatchObject({
    path: `/api/sessions/${one.id}/send`,
    method: 'POST',
    body: { text: EXECUTE_PLAN },
  });
  await shot(page, engine, 'execute-approvals');
});

test('files explain missing sessions and navigate breadcrumbs sizes dim files and empty folders', async ({
  page,
  engine,
}) => {
  await open(page, engine);
  await page.keyboard.press('ControlOrMeta+b');
  const panel = page.getByRole('complementary', { name: 'Side panel' });
  await panel.getByRole('tab', { name: 'Changes', exact: true }).click();
  await expect(panel.getByText('Open a session to review its changes.')).toBeVisible();
  await panel.getByRole('tab', { name: 'Files', exact: true }).click();
  await expect(panel.getByText('Open a session to browse its folder.')).toBeVisible();
  await panel.getByRole('tab', { name: 'Terminal', exact: true }).click();
  await expect(panel.getByText('Open a session to use its terminal.')).toBeVisible();
  await panel.getByRole('button', { name: 'Close side panel' }).click();
  const one = await create(engine, 'Files fixture');
  await mkdir(join(engine.sandbox.project, 'child', 'empty'), { recursive: true });
  await writeFile(join(engine.sandbox.project, '.hidden'), 'abcd');
  await writeFile(join(engine.sandbox.project, 'sized.txt'), 'a'.repeat(2048));
  await open(page, engine, one.id);
  await page.keyboard.press('ControlOrMeta+b');
  await panel.getByRole('tab', { name: 'Files', exact: true }).click();
  await expect(panel.getByRole('button', { name: /sized.txt.*2.0 KB/ })).toBeVisible();
  await expect(panel.locator('li.dim')).toContainText('.hidden');
  await panel.getByRole('button', { name: '▸ child/', exact: true }).click();
  await panel.getByRole('button', { name: '▸ empty/', exact: true }).click();
  await expect(panel.getByText('Empty folder', { exact: true })).toBeVisible();
  await panel
    .getByRole('navigation', { name: 'Folder path' })
    .getByRole('button', { name: 'child', exact: true })
    .click();
  await expect(panel.getByRole('button', { name: '▸ empty/', exact: true })).toBeVisible();
  await panel
    .getByRole('navigation', { name: 'Folder path' })
    .getByRole('button', { name: 'project', exact: true })
    .click();
  await expect(panel.getByRole('button', { name: /sized.txt/ }).first()).toBeVisible();
  await shot(page, engine, 'files-browser');
});

test('terminal fills a wider drawer and reports an asset load failure', async ({
  page,
  engine,
}) => {
  const one = await create(engine, 'Terminal layout');
  await open(page, engine, one.id);
  await page.keyboard.press('ControlOrMeta+b');
  const panel = page.getByRole('complementary', { name: 'Side panel' });
  const before = (await panel.boundingBox())!.width;
  await page.route('**/assets/xterm-*.js', (route) => route.abort());
  await page.route('**/assets/addon-fit-*.js', (route) => route.abort());
  await panel.getByRole('tab', { name: 'Terminal', exact: true }).click();
  expect((await panel.boundingBox())!.width).toBeGreaterThan(before);
  const terminal = panel.locator('.terminal');
  expect((await terminal.boundingBox())!.height).toBeGreaterThan(600);
  await expect(terminal.getByRole('alert')).toContainText('could not load');
  await shot(page, engine, 'terminal-asset-error');
});

test('raw text paste stays composer input and first actual send requests notification permission once', async ({
  page,
  engine,
}) => {
  await page.addInitScript(() => {
    const state = { requested: 0, sent: [] as string[] };
    Object.assign(window, { testNotifications: state });
    class MockNotification {
      static permission = 'default';
      static async requestPermission() {
        state.requested++;
        this.permission = 'granted';
        return 'granted';
      }
      constructor(title: string) {
        state.sent.push(title);
      }
    }
    Object.assign(window, { Notification: MockNotification });
    localStorage.setItem('antiquota.prefs', JSON.stringify({ focused: true }));
  });
  const one = await create(engine, 'Notification fixture');
  await open(page, engine, one.id);
  await prompt(page).evaluate((node) => {
    const data = new DataTransfer();
    data.setData('text/plain', 'Ordinary pasted text');
    const event = new ClipboardEvent('paste', {
      clipboardData: data,
      bubbles: true,
      cancelable: true,
    });
    node.dispatchEvent(event);
    if (!event.defaultPrevented) {
      const input = node as HTMLTextAreaElement;
      input.value = data.getData('text/plain');
      input.dispatchEvent(new Event('input', { bubbles: true }));
    }
  });
  await expect(prompt(page)).toHaveValue('Ordinary pasted text');
  await expect(page.locator('.attachments')).toHaveCount(0);
  await primary(page).locator('button.send').click();
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as unknown as { testNotifications: { requested: number } }).testNotifications
            .requested,
      ),
    )
    .toBe(1);
  await expect
    .poll(() =>
      page.evaluate(() =>
        (
          window as unknown as { testNotifications: { sent: string[] } }
        ).testNotifications.sent.some((title) => title.includes('turn 1 SUCCESS')),
      ),
    )
    .toBe(true);
  await engine.scenario({ done_after: 1, delay_ms: 20 });
  await engine.call('POST', `/api/sessions/${one.id}/goal`, {
    objective: 'Finish fixture',
    verifier: false,
  });
  await expect
    .poll(() =>
      page.evaluate(() =>
        (
          window as unknown as { testNotifications: { sent: string[] } }
        ).testNotifications.sent.includes('Goal met'),
      ),
    )
    .toBe(true);
  await engine.call('POST', `/api/sessions/${one.id}/jobs`, {
    cmd: 'printf fixture',
    every: 0,
    notify: false,
  });
  await expect
    .poll(() =>
      page.evaluate(() =>
        (
          window as unknown as { testNotifications: { sent: string[] } }
        ).testNotifications.sent.includes('Job finished'),
      ),
    )
    .toBe(true);
  await shot(page, engine, 'live-browser-notifications');
});

test('job logs explain no output scroll to the latest line and loop stop sends DELETE', async ({
  page,
  engine,
}) => {
  const one = await create(engine, 'Job edges');
  await open(page, engine, one.id);
  const empty = JSON.parse(
    (
      await engine.call('POST', `/api/sessions/${one.id}/jobs`, {
        cmd: 'sleep 10',
        every: 0,
        notify: false,
      })
    ).text,
  ) as { id: string };
  const bar = primary(page).getByRole('region', { name: 'Background job sleep 10', exact: true });
  await bar.getByRole('button', { name: 'Log', exact: true }).click();
  const log = page.getByRole('dialog', { name: 'sleep 10', exact: true });
  await expect(log.getByLabel('Job output')).toHaveText('(no output yet)');
  await log.getByRole('button', { name: 'Close job log' }).click();
  await engine.call('POST', `/api/sessions/${one.id}/jobs/${empty.id}/stop`, {});
  const output = Array.from({ length: 300 }, (_, i) => `line-${i}`).join('\\n');
  const cmd = `printf '${output}\\n'; sleep 10`;
  const noisy = JSON.parse(
    (await engine.call('POST', `/api/sessions/${one.id}/jobs`, { cmd, every: 0, notify: false }))
      .text,
  ) as { id: string };
  await expect
    .poll(async () =>
      JSON.parse(
        (await engine.call('GET', `/api/sessions/${one.id}/jobs/${noisy.id}/log`)).text,
      ).text.includes('line-299'),
    )
    .toBe(true);
  await primary(page)
    .getByRole('region', { name: `Background job ${cmd}`, exact: true })
    .getByRole('button', { name: 'Log', exact: true })
    .click();
  const body = page.getByLabel('Job output');
  await expect(body).toContainText('line-299');
  expect(
    await body.evaluate((node) => node.scrollHeight - node.clientHeight - node.scrollTop),
  ).toBeLessThan(2);
  await shot(page, engine, 'job-log-latest');
  await page.getByRole('button', { name: 'Close job log' }).click();
  await engine.call('POST', `/api/sessions/${one.id}/jobs/${noisy.id}/stop`, {});
  await engine.call('POST', `/api/sessions/${one.id}/loop`, {
    every: '10m',
    prompt: 'Loop fixture',
  });
  const loop = primary(page).getByRole('region', { name: 'Loop', exact: true });
  await expect(loop).toBeVisible();
  const stopped = page.waitForRequest(
    (r) => new URL(r.url()).pathname.endsWith('/loop') && r.method() === 'DELETE',
  );
  await loop.getByRole('button', { name: 'Stop loop', exact: true }).click();
  await stopped;
  await expect(loop).not.toBeVisible();
});

test('Alt-click opens the other pane and session time labels update after one minute', async ({
  page,
  engine,
}) => {
  const one = await create(engine, 'Clock first'),
    two = await create(engine, 'Clock second');
  await page.clock.install();
  await open(page, engine, one.id);
  const row = page
    .getByRole('navigation', { name: 'Projects and sessions' })
    .getByRole('button', { name: /idle Clock second/ });
  await expect(row.locator('time')).toHaveText('now');
  await page.clock.fastForward(60000);
  await expect(row.locator('time')).toHaveText('1m');
  await row.click({ modifiers: ['Alt'] });
  await expect(
    page
      .getByRole('region', { name: 'Right session', exact: true })
      .getByRole('heading', { name: 'Clock second', exact: true }),
  ).toBeVisible();
  await expect(
    primary(page).getByRole('heading', { name: 'Clock first', exact: true }),
  ).toBeVisible();
  expect(two.id).not.toBe(one.id);
  await shot(page, engine, 'alt-click-minute-clock');
});

test('committed diffs load only when expanded and all git empty states remain explicit', async ({
  page,
  engine,
}) => {
  const one = await create(engine, 'Committed review');
  const worktree = {
    repo: engine.sandbox.project,
    path: engine.sandbox.project,
    branch: 'test',
    base: 'main',
    base_sha: '1',
  };
  let committed = true;
  await page.route(`**/api/sessions/${one.id}/git`, (route) =>
    route.fulfill({
      json: {
        root: engine.sandbox.project,
        branch: 'test',
        files: [],
        worktree,
        committed: committed ? [{ path: 'committed.txt', add: 2, del: 1 }] : [],
        ahead: 1,
        merged: false,
      },
    }),
  );
  const reads: string[] = [];
  await page.route('**/git/diff?**', (route) => {
    reads.push(route.request().url());
    return route.fulfill({ json: { diff: '+base-relative fixture' } });
  });
  await open(page, engine, one.id);
  await page.keyboard.press('ControlOrMeta+b');
  const panel = page.getByRole('complementary', { name: 'Side panel' });
  await expect(panel.getByText('No uncommitted changes.')).toBeVisible();
  await expect(panel.getByText('agy has not edited files in this session yet.')).toBeVisible();
  const files = panel.getByRole('list', { name: 'Committed files' });
  await expect(files).toBeVisible();
  expect(reads).toEqual([]);
  await files.locator('summary').click();
  await expect(files.locator('pre')).toHaveText('+base-relative fixture');
  expect(reads).toHaveLength(1);
  expect(new URL(reads[0]!).searchParams.get('committed')).toBe('1');
  committed = false;
  await page.reload();
  await expect(panel.getByText('Nothing committed on this branch yet.')).toBeVisible();
  await shot(page, engine, 'committed-review');
});

test('engine diagnostics presents every identity path hook and count from the exact engine response', async ({
  page,
  engine,
}) => {
  const one = await create(engine, 'Diagnostics fixture');
  await open(page, engine, one.id);
  const value = JSON.parse((await engine.call('GET', '/api/engine')).text) as {
    version: string;
    url: string;
    pid: number;
    by: string;
    base: string;
    stack: string;
    agy: string;
    hook: boolean;
    sessions: number;
    accounts: number;
  };
  await page.getByRole('button', { name: /^Settings/ }).click();
  await page
    .getByRole('navigation', { name: 'Settings sections' })
    .getByRole('button', { name: 'Engine', exact: true })
    .click();
  const dialog = page.getByRole('dialog', { name: 'Engine', exact: true });
  await expect(dialog.locator('dl dt')).toHaveText([
    'Version',
    'Engine',
    'Data',
    'Stack',
    'agy',
    'Permissions',
    'Sessions',
  ]);
  await expect(dialog.locator('dl dd')).toHaveText([
    value.version,
    `${value.url} · pid ${value.pid} · ${value.by}`,
    value.base,
    value.stack,
    value.agy || 'not found',
    value.hook
      ? 'antiquota decides each tool call (hook on)'
      : 'agy’s own headless rules (hook off)',
    `${value.sessions} · ${value.accounts} accounts`,
  ]);
  await shot(page, engine, 'engine-diagnostics');
});
