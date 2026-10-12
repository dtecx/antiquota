/** Proves device presentation choices rebuild preserved transcript rows. */
import { appendFile, mkdir } from 'node:fs/promises';
import { join } from 'node:path';
import { test, expect } from './fixtures';
import { createSession } from './account-support';
import type { EventRecord } from '../src/gen/EventRecord';
test('switches grouped activity, independent steps, previews, thinking and timestamps', async ({
  page,
  engine,
}) => {
  await createSession(page, engine, 'Appearance fixture');
  const id = decodeURIComponent(new URL(page.url()).hash.slice(1)),
    port = engine.port,
    ts = Date.now() / 1000;
  await engine.stop();
  const events: EventRecord[] = [
    { t: 'user', ts, text: 'Inspect this project.', kind: 'message' },
    { t: 'turn_start', ts, turn: 1, slot: 'one@example.com', model: 'gemini-3.8-flash-high' },
    {
      t: 'tool',
      ts,
      turn: 1,
      step: 1,
      name: 'run_command',
      params: { CommandLine: 'printf fixture' },
      state: 'done',
      output: 'first line\nsecond line\nlast line',
    },
    { t: 'thinking', ts, turn: 1, step: 2, text: 'Preserved reasoning.', tokens: 42 },
    { t: 'assistant', ts, turn: 1, step: 3, text: 'The project looks good.' },
    {
      t: 'turn_end',
      ts,
      turn: 1,
      slot: 'one@example.com',
      status: 'SUCCESS',
      secs: 1,
      usage: { input_tokens: 3, output_tokens: 2, thinking_tokens: 1, total_tokens: 6, context: 0 },
    },
  ];
  await appendFile(
    join(engine.sandbox.data, 'sessions', id, 'events.jsonl'),
    events.map((event) => JSON.stringify(event)).join('\n') + '\n',
  );
  await engine.start(port);
  await page.reload();
  await expect(page.getByLabel('Agent reply')).toContainText('The project looks good.');
  await expect(page.locator('details.activity')).not.toHaveAttribute('open', '');
  await page.locator('details.activity > summary').click();
  await expect(page.locator('.tool .preview')).toContainText('last line');
  await expect(page.locator('.thinking summary')).toContainText('42 tokens');
  await expect(page.locator('.turn-end time')).toBeVisible();
  async function appearance() {
    await page.keyboard.press('ControlOrMeta+,');
    await page
      .getByRole('navigation', { name: 'Settings sections' })
      .getByRole('button', { name: 'Appearance', exact: true })
      .click();
  }
  await appearance();
  await page.getByRole('button', { name: 'Every step', exact: true }).click();
  await page.getByRole('button', { name: 'Close settings' }).click();
  await expect(page.locator('details.activity')).toHaveCount(0);
  await expect(page.locator('.tool > summary')).toBeVisible();
  await expect(page.locator('.thinking > summary')).toBeVisible();
  const directory = join(engine.sandbox.repository, '.handoff', 'evidence', 'ui-slice9b');
  await mkdir(directory, { recursive: true });
  await page.screenshot({ path: join(directory, 'every-step.png'), fullPage: true });
  await appearance();
  await page
    .getByRole('group', { name: 'Transcript mode' })
    .last()
    .getByRole('button', { name: 'Compact', exact: true })
    .click();
  await page.getByLabel('Times on turns').uncheck();
  await page.getByRole('button', { name: 'Close settings' }).click();
  // Compact keeps tools folded without previews and leaves thinking and turn ends out.
  await expect(page.locator('.tool .preview')).toHaveCount(0);
  await expect(page.locator('.thinking')).toHaveCount(0);
  await expect(page.locator('.turn-end')).toHaveCount(0);
  await appearance();
  await page
    .getByRole('group', { name: 'Transcript mode' })
    .last()
    .getByRole('button', { name: 'Verbose', exact: true })
    .click();
  await page.getByRole('button', { name: 'Close settings' }).click();
  await expect(page.locator('.thinking')).toHaveCount(1);
  await expect(page.locator('.turn-end time')).toHaveCount(0);
  await appearance();
  await page.getByRole('button', { name: 'Grouped steps', exact: true }).click();
  await page.getByRole('button', { name: 'Close settings' }).click();
  await expect(page.locator('details.activity')).toHaveCount(1);
  await expect(page.locator('details.activity')).toHaveAttribute('open', '');
});
