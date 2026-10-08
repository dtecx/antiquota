/** Owns synthetic account preparation and local screenshot evidence for browser tests. */
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import type { Page } from '@playwright/test';
import { expect } from './fixtures';
import type { Account } from '../src/gen/Account';
import type { FixtureEngine } from '../scripts/engine.ts';
export async function addAccount(
  engine: FixtureEngine,
  slot: string,
  fields: Partial<Account> = {},
) {
  if (!/^[A-Za-z0-9_@.+-]+$/.test(slot)) throw new Error('Unsafe fixture account name');
  const auth = join(engine.sandbox.data, 'slots', slot, '.gemini', 'antigravity-cli');
  await mkdir(auth, { recursive: true });
  await writeFile(join(auth, 'antigravity-oauth-token'), 'synthetic-account-token', {
    mode: 0o600,
  });
  const state = join(engine.sandbox.data, 'state');
  await mkdir(join(state, 'quota'), { recursive: true });
  await writeFile(
    join(state, 'quota', `${slot}.json`),
    JSON.stringify({
      at: fields.probed ?? Date.now() / 1000,
      quota: fields.quota ?? {
        gemini: { '5h': [0.9, 4070926800], weekly: [0.9, 4071427200] },
        cg: { '5h': [0.5, null], weekly: [0.7, 4071427200] },
      },
    }),
  );
  if (fields.paused) {
    await mkdir(join(state, 'paused'), { recursive: true });
    await writeFile(join(state, 'paused', slot), '');
  }
  for (const [pool, until] of Object.entries(fields.walls ?? {})) {
    await mkdir(join(state, 'walls'), { recursive: true });
    await writeFile(
      join(state, 'walls', `${slot}.${pool}`),
      `${until} ${fields.why?.[pool] ?? 'quota spent'}\n`,
    );
  }
  if (fields.verify) {
    await mkdir(join(state, 'verify'), { recursive: true });
    await writeFile(
      join(state, 'verify', `${slot}.json`),
      JSON.stringify({ url: fields.verify, since: Date.now() / 1000 }),
    );
  }
}
export function quotaResponse(fraction = 0.9) {
  return {
    status: 'SUCCESS',
    command: {
      name: 'quota',
      data: {
        groups: ['Gemini', 'Claude / GPT'].map((name) => ({
          name,
          buckets: [
            {
              window: '5h',
              remaining_fraction: name === 'Gemini' ? fraction : 0.9,
              reset_time: '2099-01-01T05:00:00Z',
            },
            { window: 'weekly', remaining_fraction: 0.9, reset_time: '2099-01-07T00:00:00Z' },
          ],
        })),
      },
    },
  };
}
export async function screenshot(page: Page, engine: FixtureEngine, name: string) {
  const directory = join(engine.sandbox.repository, '.handoff', 'evidence', 'ui-slice2');
  await mkdir(directory, { recursive: true });
  await page.screenshot({ path: join(directory, `${name}.png`), fullPage: true });
}
export async function fakeConsent(page: Page) {
  for (const address of ['https://example.invalid/**', 'https://accounts.google.com/**'])
    await page.context().route(address, (route) =>
      route.fulfill({
        contentType: 'text/html',
        body: '<p>Synthetic consent fixture. No provider was contacted.</p>',
      }),
    );
}
export async function openAccounts(page: Page, engine: FixtureEngine) {
  await page.goto(engine.url);
  await expect(page.getByText('Engine connected')).toBeVisible();
  await page.getByRole('button', { name: 'Accounts and quota' }).click();
  await page
    .getByRole('navigation', { name: 'Settings sections' })
    .getByRole('button', { name: 'Accounts', exact: true })
    .click();
  await expect(page.getByRole('heading', { name: 'Accounts', exact: true })).toBeVisible();
  await expect(page.getByText('Refreshing accounts and settings…')).not.toBeVisible();
}
export async function createSession(page: Page, engine: FixtureEngine, title: string) {
  await page.goto(engine.url);
  await expect(page.getByText('Engine connected')).toBeVisible();
  await page.getByRole('button', { name: 'New session' }).first().click();
  await page.getByRole('textbox', { name: 'Session name' }).fill(title);
  await page.getByRole('textbox', { name: 'Project folder' }).fill(engine.sandbox.project);
  await page.getByRole('button', { name: 'Create session' }).click();
  await expect(page.getByRole('heading', { name: title, exact: true })).toBeVisible();
}
export async function startLogin(page: Page) {
  await page.getByRole('button', { name: 'Add account', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Add Google account' });
  await dialog.getByRole('button', { name: 'Open Google sign-in' }).click();
  await expect(dialog.getByRole('textbox', { name: 'Sign-in code' })).toBeEnabled();
  return dialog;
}
