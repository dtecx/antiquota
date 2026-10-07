/** Proves job and loop bars, the log viewer and job lines in a real browser. */
import type { Page } from '@playwright/test';
import { mkdir } from 'node:fs/promises';
import { join } from 'node:path';
import { test, expect } from './fixtures';
import type { FixtureEngine } from '../scripts/engine.ts';

async function create(page: Page, engine: FixtureEngine, title: string) {
  await page.goto(engine.url);
  await expect(page.getByText('Engine connected')).toBeVisible();
  await page.getByRole('button', { name: 'New session' }).first().click();
  await page.getByRole('textbox', { name: 'Session name' }).fill(title);
  await page.getByRole('textbox', { name: 'Project folder' }).fill(engine.sandbox.project);
  await page.getByRole('button', { name: 'Create session' }).click();
  await expect(page.getByRole('heading', { name: title, exact: true })).toBeVisible();
  const state = JSON.parse((await engine.call('GET', '/api/state')).text) as {
    sessions: { id: string; title: string }[];
  };
  return state.sessions.find((item) => item.title === title)?.id ?? '';
}

test('shows running jobs, their log, stop, and loops', async ({ page, engine }) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  const id = await create(page, engine, 'Jobs fixture');
  await engine.call('POST', `/api/sessions/${id}/jobs`, {
    cmd: "printf 'line one\\nline two\\n'; sleep 30",
    every: '10m',
    notify: false,
  });
  const bar = page.getByRole('region', { name: /Background job/ });
  await expect(bar).toContainText("printf 'line one\\nline two\\n'; sleep 30");
  await expect(bar).toContainText('check-in every 10m');
  await expect(page.getByText('▶ job')).toBeVisible();
  await bar.getByRole('button', { name: 'Log' }).click();
  const log = page.getByRole('dialog', { name: /printf/ });
  await expect(log.getByLabel('Job output')).toHaveText('line one\nline two\n');
  const directory = join(engine.sandbox.repository, '.handoff', 'evidence', 'ui-slice5');
  await mkdir(directory, { recursive: true });
  await page.screenshot({ path: join(directory, 'job-log.png'), fullPage: true });
  await log.getByRole('button', { name: 'Close job log' }).click();
  await page.screenshot({ path: join(directory, 'job-bar.png'), fullPage: true });
  await bar.getByRole('button', { name: 'Stop' }).click();
  await expect(bar).toHaveCount(0);
  await expect(page.locator('.jobline').filter({ hasText: 'stopped' })).toBeVisible();
  await engine.call('POST', `/api/sessions/${id}/loop`, { at: '03:00', prompt: 'Nightly check.' });
  const schedule = page.getByRole('region', { name: 'Daily schedule' });
  await expect(schedule).toContainText('Nightly check.');
  await expect(schedule).toContainText('daily at 03:00 · next');
  await schedule.getByRole('button', { name: 'Remove' }).click();
  await expect(schedule).toHaveCount(0);
  expect(errors).toEqual([]);
});
