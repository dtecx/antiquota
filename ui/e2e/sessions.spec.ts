/** Proves the first end-to-end slice with real HTTP/SSE and a synthetic agy process. */
import type { Page } from '@playwright/test';
import { appendFile, mkdir } from 'node:fs/promises';
import { join } from 'node:path';
import { test, expect } from './fixtures';
import type { FixtureEngine } from '../scripts/engine.ts';
import type { EventRecord } from '../src/gen/EventRecord';

async function create(page: Page, engine: FixtureEngine, title = 'Browser fixture') {
  await page.goto(engine.url);
  await expect(page.getByText('Engine connected')).toBeVisible();
  await page.getByRole('button', { name: 'New session' }).first().click();
  await page.getByRole('textbox', { name: 'Session name' }).fill(title);
  await page.getByRole('textbox', { name: 'Project folder' }).fill(engine.sandbox.project);
  await page.getByRole('button', { name: 'Create session' }).click();
  await expect(page.getByRole('heading', { name: title, exact: true })).toBeVisible();
}
async function screenshot(page: Page, engine: FixtureEngine, name: string) {
  const directory = join(engine.sandbox.repository, '.handoff', 'evidence', 'ui-slice1');
  await mkdir(directory, { recursive: true });
  await page.screenshot({ path: join(directory, `${name}.png`), fullPage: true });
}

test('creates sends streams groups renames and reloads a session', async ({ page, engine }) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await engine.scenario({ reply: 'A streamed fixture reply.', delay_ms: 2500 });
  await create(page, engine);
  await page
    .getByRole('textbox', { name: 'Message', exact: true })
    .fill('Inspect this fixture project.');
  await page.getByRole('button', { name: 'Send', exact: true }).click();
  await expect(page.getByLabel('Your message')).toContainText('Inspect this fixture project.');
  await expect(page.getByRole('button', { name: 'Stop', exact: true })).toBeVisible();
  await expect(page.getByLabel('Agent reply')).toContainText('A streamed fixture reply.');
  await expect(page.getByText('Agent activity', { exact: true })).toBeVisible();
  await page.getByText('Agent activity', { exact: true }).click();
  await expect(page.locator('.tool .name').filter({ hasText: /^Write$/ })).toBeVisible();
  await page
    .locator('.tool .name')
    .filter({ hasText: /^Write$/ })
    .click();
  await expect(page.locator('.tool .body')).toContainText('notes.txt');
  await screenshot(page, engine, 'desktop-dark-expanded');
  await page.getByText('Agent activity', { exact: true }).click();
  await expect(page.locator('.tool .name').filter({ hasText: /^Write$/ })).not.toBeVisible();
  await page.getByRole('button', { name: 'Rename session' }).click();
  await page.getByRole('textbox', { name: 'Session title' }).fill('Renamed browser fixture');
  await page.getByRole('textbox', { name: 'Session title' }).press('Enter');
  await expect(
    page.getByRole('heading', { name: 'Renamed browser fixture', exact: true }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Pin session', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Unpin session', exact: true })).toBeVisible();
  await page.reload();
  await expect(
    page.getByRole('heading', { name: 'Renamed browser fixture', exact: true }),
  ).toBeVisible();
  await expect(page.getByLabel('Agent reply')).toContainText('A streamed fixture reply.');
  await screenshot(page, engine, 'desktop-dark-collapsed');
  expect(errors).toEqual([]);
});

test('queues a second message then stops the running turn', async ({ page, engine }) => {
  await engine.scenario({ scenario: 'delay', delay_ms: 10_000, reply: 'Delayed fixture reply.' });
  await create(page, engine);
  const prompt = page.getByRole('textbox', { name: 'Message', exact: true });
  await prompt.fill('Begin a long fixture turn.');
  await prompt.press('Enter');
  await expect(page.getByRole('button', { name: 'Stop', exact: true })).toBeVisible();
  await prompt.fill('Keep this queued message.');
  await page.getByRole('button', { name: 'Queue', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Message queued for the next turn.');
  await page.getByRole('button', { name: 'Stop', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Stop', exact: true })).not.toBeVisible();
  await expect(page.locator('.session-state .status')).toHaveText('idle');
  await screenshot(page, engine, 'stopped-queued-turn');
});

test('recovers after an engine restart and keeps the unsent draft', async ({ page, engine }) => {
  await create(page, engine, 'Restart fixture');
  const prompt = page.getByRole('textbox', { name: 'Message', exact: true });
  await prompt.fill('Unsent draft survives reconnect.');
  const port = engine.port;
  await engine.stop();
  await expect(
    page.getByText('Engine disconnected. Reconnecting; your draft is kept.'),
  ).toBeVisible();
  await expect(prompt).toHaveValue('Unsent draft survives reconnect.');
  await engine.start(port);
  await expect(page.getByText('Engine connected')).toBeVisible({ timeout: 12_000 });
  await expect(prompt).toHaveValue('Unsent draft survives reconnect.');
  await page.getByRole('button', { name: 'Send', exact: true }).click();
  await expect(page.getByLabel('Agent reply')).toContainText('Fixture reply.');
  await page.reload();
  await expect(page.getByRole('heading', { name: 'Restart fixture', exact: true })).toBeVisible();
  await expect(page.getByLabel('Agent reply')).toContainText('Fixture reply.');
  await screenshot(page, engine, 'engine-restart-recovered');
});

test('supports light theme narrow navigation filtering and validation errors', async ({
  page,
  engine,
}) => {
  await page.emulateMedia({ colorScheme: 'light' });
  await create(page, engine, 'Narrow fixture');
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(page.getByRole('button', { name: 'Open sessions' })).toBeVisible();
  await page.getByRole('button', { name: 'Open sessions' }).click();
  await page.getByRole('textbox', { name: 'Filter sessions' }).fill('nothing matches');
  await expect(page.getByText('No matching sessions.')).toBeVisible();
  await page.getByRole('textbox', { name: 'Filter sessions' }).fill('Narrow');
  await page.locator('button.session').filter({ hasText: 'Narrow fixture' }).click();
  await expect(page.getByLabel('Conversation', { exact: true })).toHaveAttribute(
    'aria-busy',
    'false',
  );
  await screenshot(page, engine, 'narrow-light');
  await page.setViewportSize({ width: 1280, height: 820 });
  await page.getByRole('button', { name: 'New session' }).first().click();
  await page
    .getByRole('textbox', { name: 'Project folder' })
    .fill(join(engine.sandbox.home, 'missing'));
  await page.getByRole('button', { name: 'Create session' }).click();
  await expect(page.getByRole('alert')).toBeVisible();
  await expect(page.getByRole('dialog')).toBeVisible();
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  await screenshot(page, engine, 'desktop-light');
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});

test('shows an agent crash and allows the next turn to recover', async ({ page, engine }) => {
  await engine.scenario({ scenario: 'crash' });
  await create(page, engine, 'Crash recovery');
  await page
    .getByRole('textbox', { name: 'Message', exact: true })
    .fill('Crash this fixture turn.');
  await page.getByRole('button', { name: 'Send', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('fake agy crashed');
  await expect(page.locator('.session-state .status')).toHaveText('idle');
  await screenshot(page, engine, 'agent-crash');
  await engine.scenario({ reply: 'Recovered fixture reply.' });
  await page
    .getByRole('textbox', { name: 'Message', exact: true })
    .fill('Recover this fixture turn.');
  await page.getByRole('button', { name: 'Send', exact: true }).click();
  await expect(page.getByLabel('Agent reply')).toContainText('Recovered fixture reply.');
});

test('reads expandable thinking from compatible durable history', async ({ page, engine }) => {
  await create(page, engine, 'Thinking history');
  const id = decodeURIComponent(new URL(page.url()).hash.slice(1));
  expect(id).toMatch(/^[a-zA-Z0-9_-]+$/);
  const port = engine.port;
  await engine.stop();
  const thinking: EventRecord = {
    t: 'thinking',
    ts: Date.now() / 1000,
    turn: 1,
    step: 2,
    text: 'A preserved reasoning step from the conversation history.',
    tokens: 42,
  };
  await appendFile(
    join(engine.sandbox.data, 'sessions', id, 'events.jsonl'),
    `${JSON.stringify(thinking)}\n`,
  );
  await engine.start(port);
  await page.reload();
  await expect(page.locator('.thinking')).toHaveAttribute('open', '');
  await expect(page.locator('.thinking .markdown')).toHaveText(thinking.text);
  await expect(page.locator('.thinking summary')).toContainText('42 tokens');
  await screenshot(page, engine, 'thinking-history');
});
