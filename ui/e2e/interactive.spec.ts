/** Proves the terminal harness and browser share one durable conversation and engine. */
import { mkdir } from 'node:fs/promises';
import { join } from 'node:path';
import { test, expect } from './fixtures';

test('interactive CLI streams in a real xterm and resumes the same browser session', async ({
  page,
  engine,
}) => {
  await page.goto(engine.url);
  await expect(page.getByText('Engine connected')).toBeVisible();
  const response = await engine.call('POST', '/api/sessions', {
    cwd: engine.sandbox.project,
    title: 'CLI and browser',
    model: 'gemini-3.8-flash-high',
    mode: 'default',
  });
  expect(response.status).toBe(200);
  const session = JSON.parse(response.text) as { id: string };
  await page.locator('button.session', { hasText: 'CLI and browser' }).click();
  await expect(page.getByRole('heading', { name: 'CLI and browser', exact: true })).toBeVisible();
  await page.keyboard.press('ControlOrMeta+b');
  await page.getByRole('tab', { name: 'Terminal' }).click();
  const input = page.locator('.xterm-helper-textarea');
  const output = page.getByLabel('Terminal output').locator('.xterm-rows');
  const discovery = (await engine.call('GET', '/api/engine')).text;
  await input.focus();
  await page.keyboard.type(`'${engine.sandbox.executable('antiquota')}' -i --resume ${session.id}`);
  await page.keyboard.press('Enter');
  await expect(output).toContainText('Shared sessions');
  await expect(output).toContainText('account automatic');
  await page.keyboard.type('A prompt from the interactive terminal.');
  await page.keyboard.press('Enter');
  await expect(output).toContainText('Fixture reply.');
  await expect(page.getByRole('region', { name: 'Conversation' })).toContainText(
    'A prompt from the interactive terminal.',
  );
  await expect(page.getByRole('region', { name: 'Conversation' })).toContainText('Fixture reply.');
  await expect(output).toContainText('account one@example.com');
  const directory = join(engine.sandbox.repository, '.handoff/evidence/ui-slice23');
  await mkdir(directory, { recursive: true });
  await page.screenshot({
    path: join(directory, 'interactive-shared-session.png'),
    fullPage: true,
  });
  await page.setViewportSize({ width: 900, height: 820 });
  await page.keyboard.type('/sta');
  await page.keyboard.press('Tab');
  await page.keyboard.press('Enter');
  await expect(output).toContainText('1 turns');
  await page.screenshot({ path: join(directory, 'interactive-narrow-status.png'), fullPage: true });
  await page.keyboard.press('Control+d');
  await expect(output).toContainText('Saved session');
  expect((await engine.call('GET', '/api/engine')).text).toBe(discovery);
  const saved = JSON.parse((await engine.call('GET', `/api/sessions/${session.id}`)).text);
  expect(saved.meta.turns).toBe(1);
  expect(saved.meta.status).toBe('idle');
});
