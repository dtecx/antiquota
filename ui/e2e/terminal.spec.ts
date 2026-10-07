/** Proves the Terminal tab: a shell in the session folder, input, output, clear, restart and kill. */
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
}

test('runs a shell in the session folder', async ({ page, engine }) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await create(page, engine, 'Terminal fixture');
  await page.keyboard.press('ControlOrMeta+b');
  await page.getByRole('tab', { name: 'Terminal' }).click();
  const title = page.locator('.terminal .title');
  await expect(title).toContainText(engine.sandbox.project);
  const output = page.getByLabel('Terminal output');
  await page.locator('.xterm-helper-textarea').focus();
  await page.keyboard.type('printf "term-%s\\n" ok; pwd');
  await page.keyboard.press('Enter');
  await expect(output.locator('.xterm-rows')).toContainText('term-ok');
  await expect(output.locator('.xterm-rows')).toContainText(engine.sandbox.project);
  const directory = join(engine.sandbox.repository, '.handoff', 'evidence', 'ui-slice7');
  await mkdir(directory, { recursive: true });
  await page.screenshot({ path: join(directory, 'terminal.png'), fullPage: true });
  await page.getByRole('tab', { name: 'Files' }).click();
  await page.getByRole('tab', { name: 'Terminal' }).click();
  await expect(output.locator('.xterm-rows')).toContainText('term-ok', { timeout: 2000 });
  await page.getByRole('button', { name: 'Clear' }).click();
  await expect(output.locator('.xterm-rows')).not.toContainText('term-ok');
  await page.getByRole('button', { name: 'Kill' }).click();
  await expect(title).toContainText('· exited');
  await page.getByRole('button', { name: 'Restart' }).click();
  await expect(title).not.toContainText('exited');
  await page.locator('.xterm-helper-textarea').focus();
  await page.keyboard.type('echo again-$((40+2))');
  await page.keyboard.press('Enter');
  await expect(output.locator('.xterm-rows')).toContainText('again-42');
  expect(errors).toEqual([]);
});
