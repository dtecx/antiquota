/** Proves browser sign-in, clipboard and empty-account edge states with synthetic identities. */
import { test, expect } from './fixtures';
import {
  fakeConsent,
  openAccounts,
  screenshot,
  startLogin,
  createSession,
} from './account-support';
const consent = 'https://accounts.google.com/o/oauth2/auth?client_id=stub&state=stub';
const code = '4/abcdefghijklmnopqrstuv';
test('opens sign-in from an empty account pill welcome and empty session', async ({
  page,
  engine,
}) => {
  await expect
    .poll(async () => (await engine.call('DELETE', '/api/accounts/one%40example.com')).status)
    .toBe(200);
  await page.goto(engine.url);
  await expect(
    page.getByRole('button', { name: 'Add a Google account', exact: true }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Accounts and quota' }).click();
  const dialog = page.getByRole('dialog', { name: 'Add Google account' });
  await expect(dialog).toBeVisible();
  await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
  await page.getByRole('button', { name: 'Add a Google account', exact: true }).click();
  await expect(dialog).toBeVisible();
  await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
  await createSession(page, engine, 'No signed-in accounts');
  await page.getByRole('button', { name: 'Add a Google account', exact: true }).click();
  await expect(dialog).toBeVisible();
  await screenshot(page, engine, 'login-empty-accounts');
  await page.keyboard.press('Escape');
  await expect(dialog).not.toBeVisible();
});
test('highlights fifteen seconds and displays countdown expiry without retaining it after cancel', async ({
  page,
  engine,
}) => {
  await fakeConsent(page);
  await page.clock.install();
  await engine.scenario({ login: { email: 'expiry@example.com', url: consent } });
  await page.route('**/api/accounts/login', async (route) => {
    const response = await route.fetch();
    const value: unknown = await response.json();
    if (!value || typeof value !== 'object') throw new Error('Invalid fixture login response');
    await route.fulfill({ response, json: { ...value, seconds: 15 } });
  });
  await openAccounts(page, engine);
  const dialog = await startLogin(page);
  await expect(dialog.locator('time.expiring')).toHaveText('15s left');
  await page.clock.fastForward(15000);
  await expect(dialog.locator('time.expiring')).toHaveText('link expired');
  await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
  await page.getByRole('button', { name: 'Add account', exact: true }).click();
  await expect(dialog.locator('time')).not.toBeVisible();
  await expect(dialog.getByRole('textbox', { name: 'Sign-in code' })).toHaveValue('');
});
test('reads a valid clipboard code on returning focus only while the code field is empty', async ({
  page,
  engine,
}) => {
  await fakeConsent(page);
  await page.addInitScript((validCode) => {
    Object.defineProperty(navigator, 'clipboard', { value: { readText: async () => validCode } });
  }, code);
  await engine.scenario({ login: { email: 'focus@example.com', code, url: consent } });
  await openAccounts(page, engine);
  const dialog = await startLogin(page);
  const field = dialog.getByRole('textbox', { name: 'Sign-in code' });
  await field.fill('manual');
  await page.evaluate(() => window.dispatchEvent(new Event('focus')));
  await expect(field).toHaveValue('manual');
  await expect(dialog.getByRole('status')).not.toContainText('Signed in');
  await field.fill('');
  await page.evaluate(() => window.dispatchEvent(new Event('focus')));
  await expect(dialog.getByRole('status')).toContainText('Signed in: focus@example.com');
  await expect(dialog).not.toBeVisible();
});
test('explains denied clipboard access and rejects an invalid pasted code', async ({
  page,
  engine,
}) => {
  await fakeConsent(page);
  await page.addInitScript(() => {
    Object.defineProperty(navigator, 'clipboard', {
      value: {
        readText: async () => {
          throw new Error('Fixture clipboard denied');
        },
      },
    });
  });
  await engine.scenario({ login: { email: 'clipboard@example.com', url: consent } });
  await openAccounts(page, engine);
  const dialog = await startLogin(page);
  await dialog.getByRole('button', { name: 'Paste from clipboard' }).click();
  await expect(dialog.getByRole('alert')).toContainText('paste the code with ⌘/Ctrl+V');
  await dialog.getByRole('textbox', { name: 'Sign-in code' }).evaluate((input) => {
    const clipboardData = new DataTransfer();
    clipboardData.setData('text', 'not-a-code');
    input.dispatchEvent(new ClipboardEvent('paste', { bubbles: true, clipboardData }));
  });
  await expect(dialog.getByRole('alert')).toContainText('Paste a sign-in code beginning with 4/');
  await expect(dialog.getByRole('textbox', { name: 'Sign-in code' })).toHaveValue('');
  await expect(dialog.getByRole('button', { name: 'Add account', exact: true })).toBeDisabled();
});
test('disables duplicate code submission and restores controls after an API error', async ({
  page,
  engine,
}) => {
  await fakeConsent(page);
  await engine.scenario({ login: { email: 'retry@example.com', code: 'good-code', url: consent } });
  await openAccounts(page, engine);
  const dialog = await startLogin(page);
  let release: () => void = () => {};
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  let submissions = 0;
  await page.route('**/api/accounts/login/*', async (route) => {
    if (route.request().method() !== 'POST') {
      await route.continue();
      return;
    }
    submissions++;
    if (submissions === 1) {
      await held;
      await route.fulfill({
        status: 400,
        contentType: 'application/json',
        body: JSON.stringify({ error: 'Fixture submit failed.' }),
      });
    } else await route.continue();
  });
  await dialog.getByRole('textbox', { name: 'Sign-in code' }).fill('good-code');
  await dialog.getByRole('button', { name: 'Add account', exact: true }).click();
  await expect(dialog.getByRole('button', { name: 'Checking…', exact: true })).toBeDisabled();
  await expect(dialog.getByRole('textbox', { name: 'Sign-in code' })).toBeDisabled();
  release();
  await expect(dialog.getByRole('alert')).toHaveText('Fixture submit failed.');
  await expect(dialog.getByRole('button', { name: 'Add account', exact: true })).toBeEnabled();
  expect(submissions).toBe(1);
  await dialog.getByRole('button', { name: 'Add account', exact: true }).click();
  await expect(dialog.getByRole('status')).toContainText('Signed in: retry@example.com');
  await expect(dialog).not.toBeVisible();
  expect(submissions).toBe(2);
});
