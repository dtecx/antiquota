/** Proves copy buttons, the token breakdown, background usage refresh and session placement. */
import { mkdir, readFile } from 'node:fs/promises';
import { join } from 'node:path';
import type { Page } from '@playwright/test';
import type { FixtureEngine } from '../scripts/engine';
import { addAccount, createSession, openAccounts } from './account-support';
import { test, expect } from './fixtures';

async function shot(page: Page, engine: FixtureEngine, name: string) {
  const path = join(engine.sandbox.repository, '.handoff/evidence/ui-slice14');
  await mkdir(path, { recursive: true });
  await page.screenshot({ path: join(path, `${name}.png`), fullPage: true });
}
async function send(page: Page, text: string) {
  await page.getByRole('textbox', { name: 'Message', exact: true }).fill(text);
  await page.getByRole('button', { name: 'Send', exact: true }).click();
}
const sid = (page: Page) => decodeURIComponent(new URL(page.url()).hash.slice(1));

test('copies a prompt and a completed reply exactly', async ({ page, engine }) => {
  await page.context().grantPermissions(['clipboard-read', 'clipboard-write']);
  await engine.scenario({ reply: 'Reply text with **bold** words.', done_after: 99 });
  await createSession(page, engine, 'Copy fixture');
  await send(page, 'Please explain\nthe second line.');
  const reply = page.getByLabel('Agent reply').last();
  await expect(reply).toContainText('Reply text with bold words.');
  await page
    .getByLabel('Your message')
    .getByRole('button', { name: 'Copy prompt', exact: true })
    .click();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
    'Please explain\nthe second line.',
  );
  await reply.getByRole('button', { name: 'Copy message', exact: true }).click();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
    'Reply text with **bold** words.',
  );
  await shot(page, engine, 'copy-buttons');
});

test('shows every token part with the cache hit rate and refreshes as turns end', async ({
  page,
  engine,
}) => {
  await engine.scenario({
    input_tokens: 300,
    output_tokens: 50,
    cache_read_tokens: 900,
    thinking_tokens: 25,
    done_after: 99,
  });
  await createSession(page, engine, 'Usage fixture');
  await send(page, 'First counted turn.');
  await expect(page.getByLabel('Agent reply').last()).toContainText('turn 1');
  const id = sid(page);
  await page.keyboard.press('ControlOrMeta+,');
  await page
    .getByRole('navigation', { name: 'Settings sections' })
    .getByRole('button', { name: 'Usage', exact: true })
    .click();
  const usage = page.getByRole('region', { name: 'Usage', exact: true });
  const parts = usage.getByLabel('Token breakdown').locator('div');
  await expect(parts).toHaveText([
    /^Input\s*300\s*today 300$/,
    /^Cached input\s*900\s*today 900$/,
    /^Output\s*50\s*today 50$/,
    /^Thinking\s*25\s*today 25$/,
    /^Cache hit rate\s*75%\s*today 75%$/,
  ]);
  const account = usage.getByRole('table', { name: 'Usage by account' }).locator('tbody tr');
  await expect(account.locator('td')).toHaveText([
    'one@example.com',
    '375',
    '375',
    '1',
    '300',
    '50',
    '75%',
  ]);
  await expect(
    usage.getByRole('table', { name: 'Usage by model' }).locator('tbody td').last(),
  ).toHaveText('75%');
  await usage.getByRole('group', { name: 'Daily usage' }).getByRole('button').last().focus();
  await expect(usage.getByRole('tooltip')).toContainText(
    'input 300 · cached input 900 · output 50 · thinking 25',
  );
  await shot(page, engine, 'usage-breakdown');
  // A turn that ends while the panel is open updates it without any click, and the open
  // tooltip stays with the new totals.
  const sent = await engine.call('POST', `/api/sessions/${id}/send`, { text: 'Second turn.' });
  expect(sent.status).toBe(200);
  await expect(parts.first()).toHaveText(/^Input\s*600\s*today 600$/);
  await expect(usage.getByRole('tooltip')).toContainText(
    'input 600 · cached input 1.8k · output 100 · thinking 50',
  );
  await expect(usage.locator('.stats')).toContainText('2 turns');
  await expect(account.locator('td').nth(4)).toHaveText('600');
});

test('places new sessions fill first by default and saves spread first', async ({
  page,
  engine,
}) => {
  await openAccounts(page, engine);
  const rotation = page.getByRole('region', { name: 'Rotation settings', exact: true });
  const fill = rotation.getByRole('radio', { name: /^Fill first/ }),
    spread = rotation.getByRole('radio', { name: /^Spread first/ });
  await expect(fill).toBeChecked();
  await expect(spread).not.toBeChecked();
  // Each choice is text only: its title, its kind and what it does.
  await expect(rotation.locator('.choice')).toHaveText([
    /^Fill first\s*sequential · default\s*Keep new sessions on one account until its quota runs out, then move to the next\.$/,
    /^Spread first\s*parallel\s*Put new sessions on accounts without a running session first, so their quotas work at once\.$/,
  ]);
  await shot(page, engine, 'placement-fill');
  await spread.check();
  await expect(page.getByText('Rotation settings saved.')).toBeVisible();
  await expect
    .poll(async () =>
      JSON.parse(await readFile(join(engine.sandbox.data, 'settings.json'), 'utf8')),
    )
    .toMatchObject({ placement: 'spread' });
  await openAccounts(page, engine);
  await expect(
    page
      .getByRole('region', { name: 'Rotation settings', exact: true })
      .getByRole('radio', { name: /^Spread first/ }),
  ).toBeChecked();
  await shot(page, engine, 'placement-spread');
  await page
    .getByRole('region', { name: 'Rotation settings', exact: true })
    .getByRole('radio', { name: /^Fill first/ })
    .check();
  // The default placement is left out of the saved settings.
  await expect
    .poll(
      async () =>
        JSON.parse(await readFile(join(engine.sandbox.data, 'settings.json'), 'utf8')).placement,
    )
    .toBeUndefined();
});

test('refreshes quotas without a button and says when each account was refreshed', async ({
  page,
  engine,
}) => {
  await addAccount(engine, 'two@example.com', {
    probed: Date.now() / 1000 - 7_200,
    quota: {
      gemini: { '5h': [0.2, 4070926800], weekly: [0.3, 4071427200] },
      cg: { '5h': [0.5, 4070926800], weekly: [0.7, 4071427200] },
    },
  });
  await openAccounts(page, engine);
  const intro = page.locator('.introduction');
  await expect(intro).toContainText(
    'Quotas refresh by themselves every 10 minutes without spending tokens.',
  );
  await expect(page.getByRole('button', { name: /probe/i })).toHaveCount(0);
  const two = page.getByRole('article', { name: 'two@example.com', exact: true });
  await expect(two.locator('.refreshed')).toHaveText('refreshed 2h');
  await expect(two.getByRole('meter', { name: 'Gemini 5h remaining' })).toHaveAttribute(
    'aria-valuenow',
    '20',
  );
  // The engine's own refresh (the one its timer runs) updates the open cards.
  expect((await engine.call('POST', '/api/accounts/probe', {})).status).toBe(200);
  await expect(two.locator('.refreshed')).toHaveText('refreshed now');
  await expect(two.getByRole('meter', { name: 'Gemini 5h remaining' })).toHaveAttribute(
    'aria-valuenow',
    '90',
  );
  await expect(page.locator('body')).not.toContainText(/probed|probing/i);
  await shot(page, engine, 'accounts-refresh');
});
