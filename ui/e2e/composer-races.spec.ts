/** Proves asynchronous composer controls retain ownership across dialogs and pane changes. */
import { test, expect } from './fixtures';
import type { Session } from '../src/gen/Session';
import type { FixtureEngine } from '../scripts/engine';
import type { Page, Route } from '@playwright/test';
async function create(engine: FixtureEngine, title: string) {
  const response = await engine.call('POST', '/api/sessions', {
    cwd: engine.sandbox.project,
    title,
  });
  expect(response.status).toBe(200);
  return JSON.parse(response.text) as Session;
}
async function open(page: Page, engine: FixtureEngine, id: string) {
  await page.goto(`${engine.url}/#${id}`);
  await expect(page.getByRole('textbox', { name: 'Message', exact: true })).toBeEnabled();
}
function heldRoute(page: Page, pattern: string) {
  let release: () => void = () => {};
  const wait = new Promise<void>((resolve) => {
    release = resolve;
  });
  let arrived: () => void = () => {};
  const seen = new Promise<void>((resolve) => {
    arrived = resolve;
  });
  return page
    .route(pattern, async (route: Route) => {
      arrived();
      await wait;
      await route.continue();
    })
    .then(() => ({ release, seen }));
}
test('late goal save preserves a newly opened objective', async ({ page, engine }) => {
  await engine.scenario({ scenario: 'delay', delay_ms: 10000 });
  const session = await create(engine, 'Goal ownership');
  await open(page, engine, session.id);
  const hold = await heldRoute(page, `**/api/sessions/${session.id}/goal`);
  await page.getByRole('button', { name: '◎ Goal', exact: true }).click();
  const goal = page.getByRole('dialog', { name: 'Goal', exact: true });
  await goal.getByRole('textbox', { name: 'Objective', exact: true }).fill('Old objective');
  await goal.getByRole('button', { name: 'Start goal', exact: true }).click();
  await hold.seen;
  await goal.getByRole('button', { name: 'Cancel', exact: true }).click();
  await page.getByRole('button', { name: '◎ Goal', exact: true }).click();
  await goal.getByRole('textbox', { name: 'Objective', exact: true }).fill('New objective');
  const response = page.waitForResponse(
    (r) => r.url().endsWith(`/sessions/${session.id}/goal`) && r.request().method() === 'POST',
  );
  hold.release();
  await response;
  await expect(page.getByRole('button', { name: 'Rename session', exact: true })).toBeEnabled();
  await expect(goal).toBeVisible();
  await expect(goal.getByRole('textbox', { name: 'Objective', exact: true })).toHaveValue(
    'New objective',
  );
  await page.unroute(`**/api/sessions/${session.id}/goal`);
});
test('busy inline rename retains edited text until its patch can run', async ({ page, engine }) => {
  const session = await create(engine, 'Rename ownership');
  await open(page, engine, session.id);
  const prompt = page.getByRole('textbox', { name: 'Message', exact: true });
  await prompt.fill('# pending note');
  await page.getByRole('button', { name: 'Rename session', exact: true }).click();
  const input = page.getByRole('textbox', { name: 'Session title', exact: true });
  await input.fill('Retained rename');
  const hold = await heldRoute(page, `**/api/sessions/${session.id}/note`);
  await prompt.dispatchEvent('keydown', { key: 'Enter' });
  await hold.seen;
  await input.press('Enter');
  await expect(input).toHaveValue('Retained rename');
  const response = page.waitForResponse((r) => r.url().endsWith(`/sessions/${session.id}/note`));
  hold.release();
  await response;
  await expect(page.getByRole('button', { name: 'Rename session', exact: true })).toBeEnabled();
  await input.press('Enter');
  await expect(page.getByRole('heading', { name: 'Retained rename', exact: true })).toBeVisible();
  expect(
    JSON.parse((await engine.call('GET', `/api/sessions/${session.id}`)).text).meta.title,
  ).toBe('Retained rename');
});
test('late attachment completion cannot activate a different pane', async ({ page, engine }) => {
  const left = await create(engine, 'Upload left');
  await create(engine, 'Upload right');
  await open(page, engine, left.id);
  await page.getByRole('button', { name: 'Split view', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Upload right', exact: true }).click();
  const primary = page.getByRole('region', { name: 'Primary session', exact: true }),
    secondary = page.getByRole('region', { name: 'Right session', exact: true });
  const hold = await heldRoute(page, `**/api/sessions/${left.id}/attach`);
  const picker = primary.locator('input[type=file]');
  await picker.setInputFiles({
    name: 'owned.txt',
    mimeType: 'text/plain',
    buffer: Buffer.from('Owned upload'),
  });
  await hold.seen;
  const rightPrompt = secondary.getByRole('textbox', { name: 'Message', exact: true });
  await rightPrompt.fill('Right remains active');
  const response = page.waitForResponse((r) => r.url().endsWith(`/sessions/${left.id}/attach`));
  hold.release();
  await response;
  await expect(primary.getByText('owned.txt', { exact: true })).toBeVisible();
  await expect(rightPrompt).toBeFocused();
  await expect(rightPrompt).toHaveValue('Right remains active');
});

test('live durable history refreshes without duplicating submitted prompts and same-session reopen restores it', async ({
  page,
  engine,
}) => {
  const session = await create(engine, 'History synchronization');
  await engine.call('POST', `/api/sessions/${session.id}/send`, { text: 'First server prompt' });
  await expect
    .poll(
      async () =>
        JSON.parse((await engine.call('GET', `/api/sessions/${session.id}`)).text).meta.status,
    )
    .toBe('idle');
  await open(page, engine, session.id);
  const input = page.getByRole('textbox', { name: 'Message', exact: true });
  await input.press('ArrowUp');
  await expect(input).toHaveValue('First server prompt');
  await input.fill('');
  await engine.call('POST', `/api/sessions/${session.id}/send`, { text: 'Second external prompt' });
  await expect(page.getByLabel('Your message').last()).toContainText('Second external prompt');
  await expect
    .poll(
      async () =>
        JSON.parse((await engine.call('GET', `/api/sessions/${session.id}`)).text).meta.status,
    )
    .toBe('idle');
  await input.press('ArrowUp');
  await expect(input).toHaveValue('Second external prompt');
  await input.press('ArrowUp');
  await expect(input).toHaveValue('First server prompt');
  await page.locator('button.session').filter({ hasText: 'History synchronization' }).click();
  await input.fill('');
  await input.press('ArrowUp');
  await expect(input).toHaveValue('Second external prompt');
});
