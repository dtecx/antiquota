/** Proves recovery route effects arrive in the existing UI over one event connection. */
import { mkdir } from 'node:fs/promises';
import { join } from 'node:path';
import { test, expect } from './fixtures';
import type { Session } from '../src/gen/Session';

test('fork and compact preserve visible history across fresh turns and reload', async ({
  page,
  engine,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  const session: Session = JSON.parse(
    (
      await engine.call('POST', '/api/sessions', {
        cwd: engine.sandbox.project,
        title: 'Recovery routes',
      })
    ).text,
  );
  await page.goto(`${engine.url}/#${session.id}`);
  await expect(page.getByText('Engine connected')).toBeVisible();
  const message = page.getByRole('textbox', { name: 'Message', exact: true });
  await message.fill('Original request.');
  await message.press('Enter');
  await expect(page.getByLabel('Agent reply')).toContainText('Fixture reply.');
  await expect(page.locator('.session-state .status')).toHaveText('idle');
  expect((await engine.call('POST', `/api/sessions/${session.id}/compact`, {})).status).toBe(200);
  await expect(
    page.getByText(
      'Compacted: the next message starts a fresh conversation from a summary of this one.',
    ),
  ).toBeVisible();
  await expect(page.getByLabel('Your message')).toContainText('Original request.');
  await engine.scenario({ reply: 'Fresh conversation reply.' });
  await message.fill('Continue after compact.');
  await message.press('Enter');
  await expect(page.getByLabel('Agent reply').last()).toContainText('Fresh conversation reply.');
  await expect(page.locator('.session-state .status')).toHaveText('idle');
  expect((await engine.call('POST', `/api/sessions/${session.id}/clear`, {})).status).toBe(200);
  await expect(
    page.getByText('New conversation: agy starts fresh from the next message.'),
  ).toBeVisible();
  const fork: Session = JSON.parse(
    (await engine.call('POST', `/api/sessions/${session.id}/fork`, {})).text,
  );
  await expect(
    page.getByRole('button', { name: 'Recovery routes (fork)', exact: false }),
  ).toBeVisible();
  const directory = join(engine.sandbox.repository, '.handoff', 'evidence', 'ui-slice9a');
  await mkdir(directory, { recursive: true });
  await page.screenshot({ path: join(directory, 'recovery-history.png'), fullPage: true });
  await page.getByRole('button', { name: 'Recovery routes (fork)', exact: false }).click();
  await expect(
    page.getByRole('heading', { name: 'Recovery routes (fork)', exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText('Picks up from "Recovery routes" with the next message.'),
  ).toBeVisible();
  await message.fill('Continue in fork.');
  await message.press('Enter');
  await expect(page.getByLabel('Agent reply')).toContainText('Fresh conversation reply.');
  await page.reload();
  await expect(
    page.getByRole('heading', { name: 'Recovery routes (fork)', exact: true }),
  ).toBeVisible();
  await expect(page.getByLabel('Your message')).toContainText('Continue in fork.');
  const restored = JSON.parse((await engine.call('GET', `/api/sessions/${fork.id}`)).text);
  expect(restored.meta.pending_note).toBe('');
  await page.screenshot({ path: join(directory, 'fork-restored.png'), fullPage: true });
  expect(errors).toEqual([]);
});
