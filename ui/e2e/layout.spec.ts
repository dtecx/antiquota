/** Proves layout, native-browser boundaries and dialog defaults in background Chromium. */
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import type { Page } from '@playwright/test';
import { test, expect } from './fixtures';
import type { FixtureEngine } from '../scripts/engine';
import type { Session } from '../src/gen/Session';
import type { EventRecord } from '../src/gen/EventRecord';
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
async function shot(page: Page, engine: FixtureEngine, name: string) {
  const directory = join(engine.sandbox.repository, '.handoff/evidence/ui-slice9f');
  await mkdir(directory, { recursive: true });
  await page.screenshot({ path: join(directory, `${name}.png`), fullPage: true });
}
async function seed(engine: FixtureEngine, meta: Session, events: EventRecord[] = []) {
  const port = engine.port;
  await engine.stop();
  const directory = join(engine.sandbox.data, 'sessions', meta.id);
  await writeFile(join(directory, 'meta.json'), JSON.stringify(meta));
  await writeFile(
    join(directory, 'events.jsonl'),
    [{ t: 'meta', ts: 1, meta }, ...events].map((e) => JSON.stringify(e)).join('\n') + '\n',
  );
  await engine.start(port);
}
async function settings(page: Page, section: string) {
  await page.getByRole('button', { name: /^Settings/ }).click();
  await page
    .getByRole('navigation', { name: 'Settings sections' })
    .getByRole('button', { name: section, exact: true })
    .click();
}

test('uses centralized warm dark and light palettes, visible keyboard focus, hidden controls and reduced motion', async ({
  page,
  engine,
}) => {
  const one = await create(engine, 'Theme fixture');
  await open(page, engine, one.id);
  await expect(page.locator('body')).toHaveCSS('background-color', 'rgb(14, 15, 17)');
  expect(
    await page.evaluate(() =>
      getComputedStyle(document.documentElement).getPropertyValue('--baton').trim(),
    ),
  ).toBe('#e8a33d');
  await page.keyboard.press('Tab');
  const focus = await page.evaluate(() => {
    const style = getComputedStyle(document.activeElement!);
    return [style.outlineStyle, style.outlineWidth, style.outlineColor];
  });
  expect(focus).toEqual(['solid', '2px', 'rgb(232, 163, 61)']);
  const box = await page.evaluate(() => {
    const button = document.createElement('button');
    button.hidden = true;
    button.textContent = 'hidden';
    document.body.append(button);
    const rect = button.getBoundingClientRect();
    const result = [rect.width, rect.height, getComputedStyle(button).display];
    button.remove();
    return result;
  });
  expect(box).toEqual([0, 0, 'none']);
  await shot(page, engine, 'desktop-dark');
  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  await expect(page.locator('body')).toHaveCSS('background-color', 'rgb(246, 245, 241)');
  expect(
    await page.evaluate(() =>
      getComputedStyle(document.documentElement).getPropertyValue('--baton').trim(),
    ),
  ).toBe('#b56d0a');
  await expect(page.getByRole('button', { name: /^New session/ }).first()).toHaveCSS(
    'transition-duration',
    '0s',
  );
  const duration = await page.evaluate(() => {
    const node = document.createElement('div');
    node.style.animation = 'probe 3s infinite';
    document.body.append(node);
    const duration = getComputedStyle(node).animationName;
    node.remove();
    return duration;
  });
  expect(duration).toBe('none');
  await shot(page, engine, 'desktop-light-reduced');
});

test('docks the composer below independent scrolling and uses pane-width token breakpoints', async ({
  page,
  engine,
}) => {
  const one = await create(engine, 'Dock fixture');
  await seed(
    engine,
    { ...one, usage: { input: 2000, output: 1000, thinking: 50 }, context: 10000 },
    [
      { t: 'user', ts: 1, kind: 'message', text: 'long transcript' },
      {
        t: 'assistant',
        ts: 2,
        turn: 1,
        step: 1,
        text: Array.from({ length: 150 }, (_, i) => `Line ${i}`).join('\n'),
      },
    ],
  );
  await open(page, engine, one.id);
  const transcript = primary(page).locator('.transcript'),
    composer = primary(page).locator('form.composer');
  const before = await composer.boundingBox();
  await transcript.evaluate((node) => {
    node.scrollTop = 0;
  });
  expect(await composer.boundingBox()).toEqual(before);
  expect(await transcript.evaluate((node) => node.scrollHeight > node.clientHeight)).toBe(true);
  const meter = primary(page).locator('.meter');
  await expect(meter.locator('.tokens')).toBeVisible();
  await meter.evaluate((node) => {
    (node as HTMLElement).style.width = '610px';
  });
  await expect(meter.locator('.tokens')).toBeHidden();
  await expect(meter.locator('.context-label')).toBeHidden();
  await expect(meter.locator('.free')).toBeVisible();
  await meter.evaluate((node) => {
    (node as HTMLElement).style.width = '430px';
  });
  await expect(meter.locator('.free')).toBeHidden();
  await shot(page, engine, 'docked-transcript');
  expect(await page.evaluate(() => getComputedStyle(document.documentElement).zoom)).toBe('1');
});

test('uses responsive side-panel overlay, narrow metadata, wrapped Settings and single-column shortcuts', async ({
  page,
  engine,
}) => {
  const one = await create(engine, 'Responsive fixture');
  await open(page, engine, one.id);
  await page.setViewportSize({ width: 900, height: 820 });
  await primary(page).getByRole('button', { name: 'Changes and files' }).click();
  const panel = page.getByRole('complementary', { name: 'Side panel' });
  await expect(panel).toHaveCSS('position', 'static');
  await page.setViewportSize({ width: 820, height: 820 });
  await expect(panel).toHaveCSS('position', 'absolute');
  await expect(primary(page).locator('header .cwd')).toBeHidden();
  await expect(primary(page).locator('header .tokens')).toBeHidden();
  await panel.getByRole('button', { name: 'Close side panel' }).click();
  await page.getByRole('button', { name: 'Open sessions' }).click();
  await settings(page, 'General');
  const nav = page.getByRole('navigation', { name: 'Settings sections' });
  await expect(nav).toHaveCSS('flex-direction', 'row');
  await expect(nav).toHaveCSS('flex-wrap', 'wrap');
  const layout = page.getByRole('dialog', { name: 'General', exact: true }).locator('.layout');
  expect(
    (await layout.evaluate((node) => getComputedStyle(node).gridTemplateColumns)).split(' '),
  ).toHaveLength(1);
  await shot(page, engine, 'narrow-settings');
  await nav.getByRole('button', { name: 'Usage', exact: true }).click();
  await expect(page.locator('.usage .stats')).toBeVisible();
  await page.setViewportSize({ width: 710, height: 820 });
  expect(
    (
      await page
        .locator('.usage .stats')
        .evaluate((node) => getComputedStyle(node).gridTemplateColumns)
    ).split(' '),
  ).toHaveLength(2);
  await page.getByRole('button', { name: 'Close settings' }).click();
  await page.keyboard.press('Control+/');
  const keys = page.getByRole('dialog', { name: 'Keyboard shortcuts' });
  await page.setViewportSize({ width: 670, height: 820 });
  expect(
    (
      await keys.locator('.grid').evaluate((node) => getComputedStyle(node).gridTemplateColumns)
    ).split(' '),
  ).toHaveLength(1);
  await shot(page, engine, 'narrow-shortcuts');
});

test('new-session form preserves current model mode parent folder and explicit worktree defaults', async ({
  page,
  engine,
}) => {
  const one = await create(engine, 'Parent fixture', {
    mode: 'plan',
    model: 'gemini-3.8-flash-low',
  });
  await seed(engine, {
    ...one,
    cwd: join(engine.sandbox.project, 'tree'),
    worktree: {
      repo: engine.sandbox.project,
      path: join(engine.sandbox.project, 'tree'),
      branch: 'isolated',
      base: 'main',
      base_sha: '1',
    },
  });
  expect(
    (await engine.call('PUT', '/api/settings/defaults', { worktree_default: 'on' })).status,
  ).toBe(200);
  await open(page, engine, one.id);
  await page
    .getByRole('button', { name: /^New session/ })
    .first()
    .click();
  const dialog = page.getByRole('dialog', { name: /^New session/ });
  await expect(dialog.getByRole('textbox', { name: 'Project folder' })).toHaveValue(
    engine.sandbox.project,
  );
  await expect(dialog.getByRole('combobox', { name: 'New session effort' })).toHaveValue(
    'gemini-3.8-flash-low',
  );
  await expect(dialog.getByRole('combobox', { name: 'New session mode', exact: true })).toHaveValue(
    'plan',
  );
  await expect(dialog.getByRole('checkbox', { name: 'Own git worktree' })).toBeChecked();
  await dialog.getByRole('checkbox', { name: 'Own git worktree' }).uncheck();
  await dialog.getByRole('textbox', { name: 'Session name' }).fill('Dialog-created');
  const created = page.waitForResponse(
    (r) => new URL(r.url()).pathname === '/api/sessions' && r.request().method() === 'POST',
  );
  await dialog.getByRole('button', { name: 'Create session' }).click();
  const response = await created;
  expect(response.request().headers()['x-antiquota-token']).toBe(engine.token);
  expect(response.request().postDataJSON()).toMatchObject({
    cwd: engine.sandbox.project,
    model: 'gemini-3.8-flash-low',
    mode: 'plan',
    worktree: false,
  });
  await expect(
    primary(page).getByRole('heading', { name: 'Dialog-created', exact: true }),
  ).toBeVisible();
  const state = JSON.parse((await engine.call('GET', '/api/state')).text) as {
    sessions: Session[];
  };
  expect(state.sessions[0]).toMatchObject({ model: 'gemini-3.8-flash-low', mode: 'plan' });
  await page
    .getByRole('button', { name: `New session in ${engine.sandbox.project.split('/').at(-1)}` })
    .click();
  await expect(dialog.getByRole('checkbox', { name: 'Own git worktree' })).not.toBeChecked();
  await shot(page, engine, 'new-session-defaults');
});

test('folder browser offers four recent unique folders navigates child and parent and debounces typing', async ({
  page,
  engine,
}) => {
  const paths = await Promise.all(
    [0, 1, 2, 3, 4].map(async (i) => {
      const path = join(engine.sandbox.project, `recent-${i}`);
      await mkdir(join(path, 'child'), { recursive: true });
      await create(engine, `Recent ${i}`, { cwd: path });
      return path;
    }),
  );
  await create(engine, 'Duplicate recent', { cwd: paths[4] });
  await open(page, engine);
  await page
    .getByRole('button', { name: /^New session/ })
    .first()
    .click();
  const dialog = page.getByRole('dialog', { name: /^New session/ }),
    folder = dialog.getByRole('textbox', { name: 'Project folder' });
  await expect(dialog.locator('.browser .recent')).toHaveCount(4);
  const titles = await dialog
    .locator('.browser .recent')
    .evaluateAll((nodes) => nodes.map((node) => node.getAttribute('title')));
  expect(new Set(titles).size).toBe(4);
  expect(titles).toContain(paths[4]);
  await dialog.locator('.browser .recent').filter({ hasText: paths[4]! }).click();
  await expect(folder).toHaveValue(paths[4]!);
  await dialog.getByRole('button', { name: 'child/', exact: true }).click();
  await expect(folder).toHaveValue(join(paths[4]!, 'child'));
  await dialog.getByRole('button', { name: 'Parent folder' }).click();
  await expect(folder).toHaveValue(paths[4]!);
  const requests: string[] = [];
  page.on('request', (r) => {
    if (new URL(r.url()).pathname === '/api/fs') requests.push(r.url());
  });
  await folder.fill(engine.sandbox.project);
  await folder.fill(paths[0]!);
  await expect.poll(() => requests.length).toBe(1);
  expect(new URL(requests[0]!).searchParams.get('path')).toBe(paths[0]);
  await shot(page, engine, 'folder-browser');
});

test('dialog cancellation and validation send nothing and failed creation keeps the API error and no local session', async ({
  page,
  engine,
}) => {
  const posts: string[] = [];
  page.on('request', (r) => {
    if (r.method() === 'POST') posts.push(new URL(r.url()).pathname);
  });
  await open(page, engine);
  await expect(page.getByRole('heading', { name: 'A place for the next idea.' })).toBeVisible();
  await primary(page).getByRole('textbox', { name: 'Message', exact: true }).fill('/clear');
  await primary(page).locator('button.send').click();
  await expect(page.locator('.toast')).toHaveText('Open a session first');
  expect(posts).toEqual([]);
  await primary(page).getByRole('textbox', { name: 'Message', exact: true }).fill('');
  await page
    .getByRole('button', { name: /^New session/ })
    .first()
    .click();
  const dialog = page.getByRole('dialog', { name: /^New session/ });
  await dialog.getByRole('textbox', { name: 'Project folder' }).fill('');
  await dialog.getByRole('button', { name: 'Create session' }).click();
  expect(posts).not.toContain('/api/sessions');
  await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(dialog).not.toBeVisible();
  expect(posts).not.toContain('/api/sessions');
  await page
    .getByRole('button', { name: /^New session/ })
    .first()
    .click();
  await dialog
    .getByRole('textbox', { name: 'Project folder' })
    .fill(join(engine.sandbox.project, 'missing'));
  await dialog.getByRole('button', { name: 'Create session' }).click();
  await expect(dialog.getByRole('alert')).toContainText('directory');
  await expect(page.locator('.toast')).toContainText('directory');
  expect(JSON.parse((await engine.call('GET', '/api/state')).text).sessions).toHaveLength(0);
  await shot(page, engine, 'creation-error');
  await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
  const one = await create(engine, 'Goal defaults');
  await open(page, engine, one.id);
  await primary(page).getByRole('button', { name: 'Set a goal', exact: true }).first().click();
  const goal = page.getByRole('dialog', { name: 'Goal', exact: true });
  await expect(goal.getByRole('checkbox', { name: 'Independent verifier' })).toBeChecked();
  await expect(goal.getByRole('spinbutton', { name: 'Rounds' })).toHaveValue('20');
  expect(
    await goal
      .getByRole('textbox', { name: 'Objective' })
      .evaluate((node) => (node as HTMLTextAreaElement).required),
  ).toBe(true);
  await goal.getByRole('button', { name: 'Start goal', exact: true }).click();
  expect(posts.some((p) => p.endsWith('/goal'))).toBe(false);
  await goal.getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(goal).not.toBeVisible();
  expect(posts.some((p) => p.endsWith('/goal'))).toBe(false);
});

test('initial fragment fallback and delayed initial models recover without CSS root zoom', async ({
  page,
  engine,
}) => {
  const first = await create(engine, 'First fixture'),
    older = await create(engine, 'Older fragment');
  const until = Date.now() + 4000;
  const requests: number[] = [];
  await page.route('**/api/state', async (route) => {
    requests.push(Date.now());
    const response = await route.fetch();
    const state = await response.json();
    if (Date.now() < until) state.models = [];
    await route.fulfill({ response, json: state });
  });
  await open(page, engine, first.id);
  await expect(
    primary(page).getByRole('heading', { name: 'First fixture', exact: true }),
  ).toBeVisible();
  await expect.poll(() => requests.some((time) => time >= until), { timeout: 6500 }).toBe(true);
  await open(page, engine, older.id);
  await expect(
    primary(page).getByRole('heading', { name: 'Older fragment', exact: true }),
  ).toBeVisible();
  await open(page, engine, 'missing');
  await expect(
    primary(page).getByRole('heading', { name: 'Older fragment', exact: true }),
  ).toBeVisible();
  await open(page, engine, '%E0%A4%A');
  await expect(
    primary(page).getByRole('heading', { name: 'Older fragment', exact: true }),
  ).toBeVisible();
  expect(
    await page.evaluate(() => [
      getComputedStyle(document.documentElement).zoom,
      document.querySelectorAll('iframe').length,
    ]),
  ).toEqual(['1', 0]);
  await shot(page, engine, 'initial-models-recovery');
});
