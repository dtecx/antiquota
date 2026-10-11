/** Proves sidebar management, search and two independent panes against an isolated engine. */
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join, basename } from 'node:path';
import type { Page } from '@playwright/test';
import { test, expect } from './fixtures';
import type { FixtureEngine } from '../scripts/engine.ts';
import type { Session } from '../src/gen/Session';
async function create(engine: FixtureEngine, title: string, cwd = engine.sandbox.project) {
  return JSON.parse((await engine.call('POST', '/api/sessions', { title, cwd })).text) as Session;
}
async function shot(page: Page, engine: FixtureEngine, name: string) {
  const directory = join(engine.sandbox.repository, '.handoff/evidence/ui-slice9c');
  await mkdir(directory, { recursive: true });
  await page.screenshot({ path: join(directory, `${name}.png`), fullPage: true });
}
async function menu(page: Page, title: string) {
  await page.getByRole('button', { name: `Session actions: ${title}`, exact: true }).click();
}
async function projectMenu(page: Page, project: string) {
  await page.getByRole('button', { name: `Project actions: ${project}`, exact: true }).click();
}
const primary = (page: Page) => page.getByRole('region', { name: 'Primary session', exact: true });
const right = (page: Page) => page.getByRole('region', { name: 'Right session', exact: true });

test('session menus rename pin fork archive and confirm deletion without touching project files', async ({
  page,
  engine,
}) => {
  const one = await create(engine, 'First'),
    two = await create(engine, 'Second');
  const keep = join(engine.sandbox.project, 'keep.txt');
  await writeFile(keep, 'Keep project files.');
  await page.goto(`${engine.url}/#${one.id}`);
  await page.locator('button.session').filter({ hasText: 'Second' }).click({ button: 'right' });
  await expect(page.getByRole('menu', { name: 'Session actions' })).toBeVisible();
  await page.getByRole('menuitem', { name: 'Rename', exact: true }).press('Enter');
  await expect(page.getByRole('textbox', { name: 'Session name' })).toHaveValue('Second');
  await page.getByRole('textbox', { name: 'Session name' }).fill('Renamed');
  await page.getByRole('button', { name: 'Save name' }).click();
  await expect(primary(page).getByRole('heading', { name: 'Renamed', exact: true })).toBeVisible();
  await menu(page, 'Renamed');
  await page.getByRole('menuitem', { name: 'Pin to top' }).click();
  await expect(page.getByRole('region', { name: 'Pinned', exact: true })).toContainText('Renamed');
  await page.reload();
  await expect(page.getByRole('region', { name: 'Pinned', exact: true })).toContainText('Renamed');
  await menu(page, 'Renamed');
  await page.getByRole('menuitem', { name: 'Unpin' }).click();
  await expect(page.getByRole('region', { name: 'Pinned', exact: true })).toHaveCount(0);
  await menu(page, 'Renamed');
  await page.getByRole('menuitem', { name: 'Fork:', exact: false }).click();
  await expect(
    primary(page).getByRole('heading', { name: 'Renamed (fork)', exact: true }),
  ).toBeVisible();
  await menu(page, 'Renamed');
  await page.getByRole('menuitem', { name: 'Archive', exact: true }).click();
  await expect(
    page.getByRole('button', { name: 'Session actions: Renamed', exact: true }),
  ).toHaveCount(0);
  await page.getByRole('button', { name: 'Show archived (1)' }).click();
  await menu(page, 'Renamed');
  await page.getByRole('menuitem', { name: 'Unarchive', exact: true }).click();
  expect(JSON.parse((await engine.call('GET', `/api/sessions/${two.id}`)).text).meta.archived).toBe(
    false,
  );
  await menu(page, 'Renamed');
  await page.getByRole('menuitem', { name: 'Archive', exact: true }).click();
  await page.getByRole('button', { name: 'Renamed', exact: false }).first().click();
  await page.getByRole('button', { name: 'Hide archived' }).click();
  await expect(
    page.getByRole('button', { name: 'Session actions: Renamed', exact: true }),
  ).toBeVisible();
  await menu(page, 'Renamed');
  await page.getByRole('menuitem', { name: 'Delete…', exact: true }).click();
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  expect((await engine.call('GET', `/api/sessions/${two.id}`)).status).toBe(200);
  await menu(page, 'Renamed');
  await page.getByRole('menuitem', { name: 'Delete…', exact: true }).click();
  await page.getByRole('button', { name: 'Delete', exact: true }).click();
  await expect(
    page.getByRole('button', { name: 'Session actions: Renamed', exact: true }),
  ).toHaveCount(0);
  expect((await engine.call('GET', `/api/sessions/${two.id}`)).status).toBe(404);
  expect(await readFile(keep, 'utf8')).toBe('Keep project files.');
  await shot(page, engine, 'session-actions');
});

test('project menus persist collapse prefill creation copy paths and confirm bulk deletion', async ({
  page,
  engine,
  context,
}) => {
  const other = join(engine.sandbox.root, 'other-project');
  await mkdir(other);
  const one = await create(engine, 'Primary'),
    two = await create(engine, 'Elsewhere', other);
  await engine.call('PATCH', `/api/sessions/${two.id}`, { archived: true });
  const three = await create(engine, 'Other active', other);
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await page.goto(`${engine.url}/#${one.id}`);
  const p = basename(engine.sandbox.project),
    o = basename(other);
  await projectMenu(page, p);
  await page.getByRole('menuitem', { name: 'New session here', exact: true }).click();
  await expect(page.getByRole('textbox', { name: 'Project folder' })).toHaveValue(
    engine.sandbox.project,
  );
  await expect(page.getByRole('checkbox', { name: 'Own git worktree' })).not.toBeChecked();
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  await projectMenu(page, p);
  await page.getByRole('menuitem', { name: 'New session in a git worktree', exact: true }).click();
  await expect(page.getByRole('textbox', { name: 'Project folder' })).toHaveValue(
    engine.sandbox.project,
  );
  await expect(page.getByRole('checkbox', { name: 'Own git worktree' })).toBeChecked();
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  await projectMenu(page, p);
  await page.getByRole('menuitem', { name: 'Copy path', exact: true }).click();
  await expect
    .poll(() => page.evaluate(() => navigator.clipboard.readText()))
    .toBe(engine.sandbox.project);
  await page
    .getByRole('region', { name: p, exact: true })
    .getByRole('button', { expanded: true })
    .click({ button: 'right' });
  await page.getByRole('menuitem', { name: 'Collapse every other project' }).click();
  await expect(
    page.getByRole('button', { name: 'Session actions: Other active', exact: true }),
  ).toHaveCount(0);
  await page.reload();
  await expect(
    page.getByRole('region', { name: o, exact: true }).getByRole('button', { expanded: false }),
  ).toBeVisible();
  await page.getByRole('textbox', { name: 'Filter sessions' }).fill('OTHER');
  await expect(
    page.getByRole('button', { name: 'Session actions: Other active', exact: true }),
  ).toBeVisible();
  await page.getByRole('textbox', { name: 'Filter sessions' }).fill('');
  await projectMenu(page, p);
  await page.getByRole('menuitem', { name: 'Expand every project' }).click();
  await page.getByRole('button', { name: 'Show archived (1)' }).click();
  await projectMenu(page, o);
  await page.getByRole('menuitem', { name: 'Delete 1 archived sessions…' }).click();
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  expect((await engine.call('GET', `/api/sessions/${two.id}`)).status).toBe(200);
  await projectMenu(page, o);
  await page.getByRole('menuitem', { name: 'Delete 1 archived sessions…' }).click();
  await page.getByRole('button', { name: 'Delete', exact: true }).click();
  await expect(
    page.getByRole('button', { name: 'Session actions: Elsewhere', exact: true }),
  ).toHaveCount(0);
  await projectMenu(page, o);
  await page.getByRole('menuitem', { name: 'Archive its 1 sessions' }).click();
  await expect
    .poll(
      async () =>
        JSON.parse((await engine.call('GET', `/api/sessions/${three.id}`)).text).meta.archived,
    )
    .toBe(true);
  await projectMenu(page, o);
  await page.getByRole('menuitem', { name: 'Delete all 1 sessions…' }).click();
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  expect((await engine.call('GET', `/api/sessions/${three.id}`)).status).toBe(200);
  await projectMenu(page, o);
  await page.getByRole('menuitem', { name: 'Delete all 1 sessions…' }).click();
  await page.getByRole('button', { name: 'Delete all', exact: true }).click();
  await expect(page.getByRole('region', { name: o, exact: true })).toHaveCount(0);
  await shot(page, engine, 'project-actions');
});

test('message search renders matching content and session selection replaces history and focuses prompt', async ({
  page,
  engine,
}) => {
  const one = await create(engine, 'Search source'),
    two = await create(engine, 'Other session');
  await engine.call('POST', `/api/sessions/${one.id}/send`, {
    text: 'The synthetic needle is here.',
  });
  await page.goto(`${engine.url}/#${one.id}`);
  await expect(page.getByLabel('Agent reply')).toContainText('Fixture reply.');
  await page.locator('button.session').filter({ hasText: 'Other session' }).click();
  await expect(primary(page).getByLabel('Your message')).toHaveCount(0);
  await expect(primary(page).getByRole('textbox', { name: 'Message', exact: true })).toBeFocused();
  await page.getByRole('textbox', { name: 'Filter sessions' }).fill('NEEDLE');
  const hit = page.getByRole('region', { name: 'In messages', exact: true }).getByRole('button');
  await expect(hit).toContainText('Search source');
  await expect(hit).toContainText('you:');
  await expect(hit).toContainText('synthetic needle');
  await expect(hit.locator('time')).not.toBeEmpty();
  await expect(hit).toHaveAttribute('title', engine.sandbox.project);
  await shot(page, engine, 'message-search');
  await hit.click();
  await expect(primary(page).getByLabel('Your message')).toContainText(
    'The synthetic needle is here.',
  );
  await expect(primary(page).getByRole('textbox', { name: 'Message', exact: true })).toBeFocused();
  await page.getByRole('textbox', { name: 'Filter sessions' }).fill('missing needle');
  await expect(page.getByText('No matching sessions.')).toBeVisible();
  await expect(page.getByText('Nothing found.', { exact: true })).toBeVisible();
  void two;
});

test('context menus wrap keyboard focus dismiss restore focus and clamp near viewport edges', async ({
  page,
  engine,
}) => {
  const one = await create(engine, 'Menu target');
  await page.goto(`${engine.url}/#${one.id}`);
  const trigger = page.getByRole('button', { name: 'Session actions: Menu target', exact: true });
  await trigger.click();
  const items = page.getByRole('menuitem');
  await expect(items.first()).toBeFocused();
  await page.keyboard.press('ArrowUp');
  await expect(items.last()).toBeFocused();
  await page.keyboard.press('ArrowDown');
  await expect(items.first()).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('menu')).toHaveCount(0);
  await expect(trigger).toBeFocused();
  await trigger.click();
  await primary(page).getByRole('heading', { level: 1 }).click();
  await expect(page.getByRole('menu')).toHaveCount(0);
  await trigger.dispatchEvent('contextmenu', { clientX: 1278, clientY: 818 });
  // The More button opens the same menu; a context event on the row supplies edge coordinates.
  await page
    .getByRole('button', { name: 'Menu target', exact: false })
    .first()
    .dispatchEvent('contextmenu', { clientX: 1278, clientY: 818 });
  const box = await page.getByRole('menu').boundingBox();
  expect(box).not.toBeNull();
  expect(box!.x + box!.width).toBeLessThanOrEqual(1280);
  expect(box!.y + box!.height).toBeLessThanOrEqual(820);
  await shot(page, engine, 'bounded-context-menu');
});

test('split shares one event connection retains independent drafts attachments scroll and swaps sessions', async ({
  page,
  engine,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  let connections = 0;
  page.on('request', (r) => {
    if (new URL(r.url()).pathname === '/api/events') connections++;
  });
  const one = await create(engine, 'Left'),
    two = await create(engine, 'Right');
  await page.goto(`${engine.url}/#${one.id}`);
  await page.getByRole('button', { name: 'Changes and files' }).click();
  await page.getByRole('button', { name: 'Split view', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Right', exact: true }).click();
  await expect(right(page).getByRole('heading', { name: 'Right', exact: true })).toBeVisible();
  await expect(page.getByRole('complementary', { name: 'Session details' })).toHaveCount(0);
  await expect(right(page).getByRole('button', { name: 'Split view', exact: true })).toHaveCount(0);
  await expect(right(page).getByRole('button', { name: 'Open sessions', exact: true })).toHaveCount(
    0,
  );
  await expect(right(page).getByRole('button', { name: 'Changes and files' })).toHaveCount(0);
  await expect(page.locator('iframe')).toHaveCount(0);
  expect(connections).toBe(1);
  const l = primary(page).getByRole('textbox', { name: 'Message', exact: true }),
    r = right(page).getByRole('textbox', { name: 'Message', exact: true });
  await l.fill('left unsent');
  await r.fill('right unsent');
  await r.press('ArrowLeft');
  await primary(page)
    .locator('input[type=file]')
    .setInputFiles({
      name: 'left.txt',
      mimeType: 'text/plain',
      buffer: Buffer.from('Left attachment'),
    });
  await right(page)
    .locator('input[type=file]')
    .setInputFiles({
      name: 'right.txt',
      mimeType: 'text/plain',
      buffer: Buffer.from('Right attachment'),
    });
  await expect(primary(page).getByLabel('Attached files')).toContainText('left.txt');
  await expect(right(page).getByLabel('Attached files')).toContainText('right.txt');
  await expect(primary(page).getByLabel('Attached files')).not.toContainText('right.txt');
  await page.evaluate(() =>
    dispatchEvent(
      new MessageEvent('message', {
        origin: 'https://untrusted.invalid',
        data: { type: 'antiquota-close-split' },
      }),
    ),
  );
  await expect(right(page)).toBeVisible();
  await right(page).getByRole('button', { name: 'Swap sides' }).click();
  await expect(primary(page).getByRole('heading', { name: 'Right', exact: true })).toBeVisible();
  await expect(l).toHaveValue('right unsent');
  await expect(r).toHaveValue('left unsent');
  await expect(primary(page).getByLabel('Attached files')).toContainText('right.txt');
  await expect(right(page).getByLabel('Attached files')).toContainText('left.txt');
  await engine.call('POST', `/api/sessions/${one.id}/send`, { text: 'Stream to original left.' });
  await expect(right(page).getByLabel('Agent reply')).toContainText('Fixture reply.');
  await expect(primary(page).getByLabel('Agent reply')).toHaveCount(0);
  await shot(page, engine, 'split-dark');
  await page.reload();
  await expect(right(page).getByRole('heading', { name: 'Left', exact: true })).toBeVisible();
  expect(connections).toBe(2);
  await page.setViewportSize({ width: 740, height: 820 });
  const lb = await primary(page).boundingBox(),
    rb = await right(page).boundingBox();
  expect(rb!.x).toBeGreaterThanOrEqual(lb!.x + lb!.width - 1);
  expect(Math.abs(rb!.y - lb!.y)).toBeLessThan(2);
  await shot(page, engine, 'split-narrow');
  await right(page).getByRole('button', { name: 'Close side by side', exact: true }).click();
  await expect(right(page)).toHaveCount(0);
  expect(
    await page.evaluate(() => JSON.parse(localStorage.getItem('antiquota.prefs')!).split),
  ).toBeNull();
  expect(errors).toEqual([]);
  void two;
});

test('split menu handles duplicate titles no choices and keyboard navigation on narrow screens', async ({
  page,
  engine,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  const one = await create(engine, 'Only');
  await page.goto(`${engine.url}/#${one.id}`);
  await page.keyboard.press('Control+Backslash');
  await expect(page.getByText('No other session yet.')).toBeVisible();
  await page.keyboard.press('Escape');
  const second = await create(engine, 'Duplicate');
  await create(engine, 'Duplicate');
  await page.reload();
  await page.keyboard.press('Control+Backslash');
  await expect(page.getByRole('menuitem', { name: 'Duplicate', exact: true })).toHaveCount(2);
  await page.getByRole('menuitem', { name: 'Duplicate', exact: true }).first().click();
  await expect(right(page)).toBeVisible();
  await page.keyboard.press('Control+Backslash');
  await expect(right(page)).toHaveCount(0);
  await page.setViewportSize({ width: 740, height: 820 });
  await expect(
    page.getByRole('complementary', { name: 'Sessions', exact: true }),
  ).not.toBeVisible();
  await page.keyboard.press('Control+k');
  const filter = page.getByRole('textbox', { name: 'Filter sessions' });
  await expect(filter).toBeFocused();
  await filter.fill('Duplicate');
  await page.keyboard.press('Control+k');
  expect(
    await filter.evaluate((e: HTMLInputElement) => [e.selectionStart, e.selectionEnd]),
  ).toEqual([0, 9]);
  await page.keyboard.press('Control+Shift+BracketRight');
  await expect(
    primary(page).getByRole('heading', { name: 'Duplicate', exact: true }),
  ).toBeVisible();
  await page.keyboard.press('Control+Shift+BracketLeft');
  await expect(
    primary(page).getByRole('heading', { name: 'Duplicate', exact: true }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Open sessions', exact: true }).click();
  await page.getByRole('button', { name: 'Close sessions', exact: true }).click();
  await expect(filter).not.toBeVisible();
  expect(errors).toEqual([]);
  void second;
});

test('sidebar shows status wake times goal rounds worktree parents and collapsed working counts', async ({
  page,
  engine,
}) => {
  const one = await create(engine, 'Current'),
    two = await create(engine, 'Sleeping');
  const other = join(engine.sandbox.root, 'other-project');
  await mkdir(other);
  const three = await create(engine, 'Working elsewhere', other);
  const now = Date.now() / 1000;
  const overrides: Record<string, Partial<Session>> = {
    [one.id]: {
      updated: now,
      goal: {
        objective: 'Display goal',
        check: '',
        verifier: false,
        verifier_model: one.model,
        max_rounds: 5,
        round: 2,
        state: 'active',
      },
    },
    [two.id]: {
      updated: now - 120,
      status: 'sleeping',
      status_until: now + 3600,
      worktree: {
        repo: engine.sandbox.project,
        path: join(engine.sandbox.root, 'worktree'),
        base: 'main',
        branch: 'feature',
        base_sha: 'synthetic',
      },
    },
    [three.id]: { updated: now - 240, status: 'running' },
  };
  await page.route('**/api/state', async (route) => {
    const response = await route.fetch(),
      state = await response.json();
    state.sessions = state.sessions.map((s: Session) => ({ ...s, ...overrides[s.id] }));
    await route.fulfill({ json: state });
  });
  await page.route(`**/api/sessions/${one.id}`, async (route) => {
    const response = await route.fetch(),
      body = await response.json();
    body.meta = { ...body.meta, ...overrides[one.id] };
    await route.fulfill({ json: body });
  });
  await page.goto(`${engine.url}/#${one.id}`);
  const group = page.getByRole('region', { name: basename(engine.sandbox.project), exact: true });
  await expect(group.locator('.session .title')).toHaveText(['Current', 'Sleeping']);
  await expect(group.locator('time').first()).toHaveText('now');
  await expect(group.locator('time').last()).toHaveText('2m');
  await expect(group.locator('.dot[title=idle]')).toHaveCount(1);
  await expect(group.locator('.dot[title=sleeping]')).toHaveCount(1);
  await expect(group).toContainText('round 2/5');
  await expect(group).toContainText('sleeping until');
  await expect(group.getByTitle('Own git worktree')).toBeVisible();
  await group.getByRole('button', { expanded: true }).click();
  await expect(group.getByRole('button', { expanded: true })).toBeVisible();
  const otherGroup = page.getByRole('region', { name: 'other-project', exact: true });
  await otherGroup.getByRole('button', { expanded: true }).click();
  await expect(otherGroup.locator('.count')).toHaveText('1');
  await expect(otherGroup.getByTitle('A session here is working')).toBeVisible();
  await shot(page, engine, 'sidebar-status');
  await page.reload();
  await expect(otherGroup.getByRole('button', { expanded: false })).toBeVisible();
  await page.getByRole('textbox', { name: 'Filter sessions' }).fill('WORKING');
  await expect(otherGroup.getByRole('button', { expanded: true })).toBeVisible();
  await expect(otherGroup.locator('.session .title')).toHaveText('Working elsewhere');
  await expect(otherGroup.locator('.dot[title=running]')).toHaveClass(/running/);
  await page.getByRole('textbox', { name: 'Filter sessions' }).fill('');
  overrides[one.id] = {
    ...overrides[one.id],
    pinned: true,
    goal: { ...overrides[one.id]!.goal!, state: 'done' },
  };
  await page.reload();
  const pinned = page.getByRole('region', { name: 'Pinned', exact: true });
  await expect(pinned).toContainText('done');
  await expect(pinned.locator('svg')).toHaveCount(2);
});

test('streaming keeps search focus and split retains scroll and caret when switching sessions', async ({
  page,
  engine,
}) => {
  const one = await create(engine, 'Scroll left'),
    two = await create(engine, 'Scroll right'),
    three = await create(engine, 'Third');
  const port = engine.port;
  await engine.stop();
  for (const session of [one, two]) {
    const events = Array.from({ length: 35 }, (_, index) => ({
      t: 'user',
      ts: index + 1,
      text: `${session.title} long durable line ${index}`,
      kind: 'message',
    }));
    await writeFile(
      join(engine.sandbox.data, 'sessions', session.id, 'events.jsonl'),
      events.map((e) => JSON.stringify(e)).join('\n') + '\n',
    );
  }
  await engine.start(port);
  await page.goto(`${engine.url}/#${one.id}`);
  await page
    .locator('button.session')
    .filter({ hasText: 'Scroll right' })
    .click({ modifiers: ['Alt'] });
  const l = primary(page).getByLabel('Conversation', { exact: true }),
    r = right(page).getByLabel('Conversation', { exact: true });
  await expect(primary(page).getByLabel('Your message')).toHaveCount(35);
  await expect(right(page).getByLabel('Your message')).toHaveCount(35);
  await l.evaluate((e) => {
    e.scrollTop = 220;
  });
  await r.evaluate((e) => {
    e.scrollTop = 440;
  });
  await expect.poll(() => l.evaluate((e) => e.scrollTop)).toBe(220);
  await expect.poll(() => r.evaluate((e) => e.scrollTop)).toBe(440);
  const prompt = primary(page).getByRole('textbox', { name: 'Message', exact: true });
  await prompt.fill('Saved caret draft');
  await prompt.evaluate((e: HTMLTextAreaElement) => {
    e.setSelectionRange(3, 5);
    e.dispatchEvent(new Event('select'));
  });
  await page.locator('button.session').filter({ hasText: 'Third' }).click();
  await expect(primary(page).getByRole('heading', { name: 'Third', exact: true })).toBeVisible();
  await page.locator('button.session').filter({ hasText: 'Scroll left' }).click();
  await expect(prompt).toHaveValue('Saved caret draft');
  await expect(prompt).toBeFocused();
  expect(
    await prompt.evaluate((e: HTMLTextAreaElement) => [e.selectionStart, e.selectionEnd]),
  ).toEqual([3, 5]);
  await expect.poll(() => l.evaluate((e) => e.scrollTop)).toBe(220);
  await expect.poll(() => r.evaluate((e) => e.scrollTop)).toBe(440);
  const filter = page.getByRole('textbox', { name: 'Filter sessions' });
  await filter.fill('Scroll');
  await engine.scenario({
    scenario: 'delay',
    delay_ms: 600,
    reply: 'Streaming without stealing focus.',
  });
  await engine.call('POST', `/api/sessions/${one.id}/send`, { text: 'Stream during search.' });
  await expect(primary(page).getByLabel('Agent reply')).toContainText(
    'Streaming without stealing focus.',
  );
  await expect(filter).toBeFocused();
  await shot(page, engine, 'split-scroll');
  void three;
});

test('empty sidebar has its own state and split choices mark busy sessions and cap at fourteen', async ({
  page,
  engine,
}) => {
  await page.goto(engine.url);
  await expect(page.getByText('Your sessions will appear here.')).toBeVisible();
  const current = await create(engine, 'Current');
  for (let i = 0; i < 16; i++) await create(engine, `Choice ${i}`);
  const archived = await create(engine, 'Archived');
  await engine.call('PATCH', `/api/sessions/${archived.id}`, { archived: true });
  await page.route('**/api/state', async (route) => {
    const response = await route.fetch(),
      state = await response.json();
    state.sessions = state.sessions.map((s: Session) =>
      s.title === 'Choice 15' ? { ...s, status: 'running' } : s,
    );
    await route.fulfill({ json: state });
  });
  await page.goto(`${engine.url}/#${current.id}`);
  await page.reload();
  await page.getByRole('button', { name: 'Split view', exact: true }).click();
  await expect(page.getByRole('menuitem')).toHaveCount(14);
  await expect(page.getByRole('menuitem', { name: '● Choice 15', exact: true })).toBeVisible();
  await expect(page.getByRole('menuitem', { name: 'Archived', exact: true })).toHaveCount(0);
  await expect(page.getByRole('menuitem', { name: 'Current', exact: true })).toHaveCount(0);
});

test('split dictation cancels its original session and ignores late speech after swapping or closing', async ({
  page,
  engine,
}) => {
  await page.addInitScript(() => {
    class Speech {
      continuous = false;
      interimResults = false;
      lang = '';
      aborted = false;
      onresult: ((event: { results: { transcript: string }[][] }) => void) | null = null;
      onend: (() => void) | null = null;
      onerror: ((event: { error: string }) => void) | null = null;
      constructor() {
        const scope = window as unknown as { fixtureSpeech: Speech[] };
        (scope.fixtureSpeech ??= []).push(this);
      }
      start() {}
      stop() {
        this.onend?.();
      }
      abort() {
        this.aborted = true;
        this.onend?.();
      }
    }
    Object.assign(window, { SpeechRecognition: Speech, fixtureSpeech: [] });
  });
  const one = await create(engine, 'Speech A');
  await create(engine, 'Speech B');
  await page.goto(`${engine.url}/#${one.id}`);
  await page
    .locator('button.session')
    .filter({ hasText: 'Speech B' })
    .click({ modifiers: ['Alt'] });
  const l = primary(page).getByRole('textbox', { name: 'Message', exact: true }),
    r = right(page).getByRole('textbox', { name: 'Message', exact: true });
  await l.fill('A draft');
  await r.fill('B draft');
  await primary(page).locator('button.mic').click();
  async function words(index: number, text: string) {
    await page.evaluate(
      ({ index, text }) => {
        const scope = window as unknown as {
          fixtureSpeech: { onresult: (event: { results: { transcript: string }[][] }) => void }[];
        };
        scope.fixtureSpeech[index]!.onresult({ results: [[{ transcript: text }]] });
      },
      { index, text },
    );
  }
  await words(0, 'current words');
  await expect(l).toHaveValue('A draft current words');
  await right(page).getByRole('button', { name: 'Swap sides' }).click();
  await expect(l).toHaveValue('B draft');
  await words(0, 'late private words');
  await expect(l).toHaveValue('B draft');
  expect(
    await page.evaluate(
      () =>
        (window as unknown as { fixtureSpeech: { aborted: boolean }[] }).fixtureSpeech[0]!.aborted,
    ),
  ).toBe(true);
  await primary(page).locator('button.mic').click();
  await words(1, 'new B words');
  await expect(l).toHaveValue('B draft new B words');
  await words(0, 'another stale callback');
  await expect(l).toHaveValue('B draft new B words');
  await right(page).locator('button.mic').click();
  await right(page).getByRole('button', { name: 'Close side by side' }).click();
  expect(
    await page.evaluate(
      () =>
        (window as unknown as { fixtureSpeech: { aborted: boolean }[] }).fixtureSpeech[2]!.aborted,
    ),
  ).toBe(true);
  await words(2, 'closed pane words');
  await expect(l).toHaveValue('B draft new B words');
  await page.keyboard.press('Escape');
});
