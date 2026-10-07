/** Proves account settings and synthetic sign-in against the Rust HTTP/SSE engine. */
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { test, expect } from './fixtures';
import {
  addAccount,
  fakeConsent,
  quotaResponse,
  screenshot,
  openAccounts,
  startLogin,
  createSession as create,
} from './account-support';
test('shows pool windows pauses resumes checks removes and refreshes accounts', async ({
  page,
  engine,
}) => {
  await addAccount(engine, 'two@example.com', {
    quota: {
      gemini: { '5h': [0.1, 4070926800], weekly: [0.3, 4071427200] },
      cg: { '5h': [0, null], weekly: [0.2, 4071427200] },
    },
  });
  await openAccounts(page, engine);
  await expect(page.getByText('No routing decisions yet.')).toBeVisible();
  const one = page.getByRole('article', { name: 'one@example.com', exact: true });
  const two = page.getByRole('article', { name: 'two@example.com', exact: true });
  await expect(two.getByRole('meter', { name: 'Gemini 5h remaining' })).toHaveAttribute(
    'aria-valuenow',
    '10',
  );
  await expect(
    two.getByRole('meter', { name: 'Claude / GPT 5h remaining' }).locator('..'),
  ).toHaveAttribute('title', 'Disabled until the other window resets');
  await one.getByRole('button', { name: 'Pause', exact: true }).click();
  await expect(one.getByText('paused', { exact: true })).toBeVisible();
  await expect(page.locator('.pill')).toContainText('1 of 2 accounts free');
  await one.getByRole('button', { name: 'Resume', exact: true }).click();
  await expect(one.getByRole('button', { name: 'Pause', exact: true })).toBeVisible();
  await two.getByRole('button', { name: 'Check', exact: true }).click();
  await expect(two.getByRole('meter', { name: 'Gemini 5h remaining' })).toHaveAttribute(
    'aria-valuenow',
    '90',
  );
  const callsBeforeProbe = (
    await readFile(join(engine.sandbox.root, 'fake-calls.jsonl'), 'utf8')
  ).split('\n').length;
  await page.getByRole('button', { name: 'Probe now' }).click();
  await expect
    .poll(async () => {
      const calls = (await readFile(join(engine.sandbox.root, 'fake-calls.jsonl'), 'utf8'))
        .split('\n')
        .slice(callsBeforeProbe - 1)
        .filter(Boolean)
        .map((line): unknown => JSON.parse(line));
      return ['one@example.com', 'two@example.com'].every((slot) =>
        calls.some(
          (call) =>
            call &&
            typeof call === 'object' &&
            'slot' in call &&
            call.slot === slot &&
            'argv' in call &&
            Array.isArray(call.argv) &&
            call.argv.includes('/quota'),
        ),
      );
    })
    .toBe(true);
  await two.getByRole('button', { name: 'Remove', exact: true }).click();
  const confirm = page.getByRole('dialog', { name: 'Remove two@example.com?' });
  await confirm.getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(two).toBeVisible();
  await two.getByRole('button', { name: 'Remove', exact: true }).click();
  await confirm.getByRole('button', { name: 'Remove', exact: true }).click();
  await expect(two).not.toBeVisible();
  await page.getByRole('button', { name: 'Probe now' }).click();
  await expect(page.getByText('Probing accounts…', { exact: true })).toBeVisible();
  await screenshot(page, engine, 'accounts-dark');
  await page.getByRole('button', { name: 'Close settings' }).click();
  await expect(page.getByRole('heading', { name: 'Accounts', exact: true })).not.toBeVisible();
});
test('persists rotation choices and range endpoints without losing focused drafts', async ({
  page,
  engine,
}) => {
  await openAccounts(page, engine);
  for (const rotation of ['sticky', 'round-robin', 'perishable']) {
    await page.getByRole('combobox', { name: 'Next account' }).selectOption(rotation);
    await page
      .getByRole('spinbutton', { name: 'Reserve', exact: true })
      .fill(rotation === 'sticky' ? '0' : '50');
    await page
      .getByRole('spinbutton', { name: 'Quota check', exact: true })
      .fill(rotation === 'sticky' ? '2' : '240');
    await page.getByRole('spinbutton', { name: 'Quota check', exact: true }).press('Tab');
    await expect(page.getByText('Rotation settings saved.')).toBeVisible();
  }
  await page.getByRole('spinbutton', { name: 'Reserve', exact: true }).fill('51');
  await page.getByRole('spinbutton', { name: 'Quota check', exact: true }).press('Tab');
  expect(
    await page
      .getByRole('spinbutton', { name: 'Reserve', exact: true })
      .evaluate((input: HTMLInputElement) => input.validity.rangeOverflow),
  ).toBe(true);
  await page.getByRole('spinbutton', { name: 'Reserve', exact: true }).fill('8');
  await page.getByRole('button', { name: 'Probe now' }).click();
  await expect(page.getByRole('spinbutton', { name: 'Reserve', exact: true })).toHaveValue('8');
  await page.getByRole('spinbutton', { name: 'Quota check', exact: true }).press('Tab');
  await page.reload();
  await page.getByRole('button', { name: 'Accounts and quota' }).click();
  await expect(page.getByRole('spinbutton', { name: 'Reserve', exact: true })).toHaveValue('8');
  const settings: unknown = JSON.parse(
    await readFile(join(engine.sandbox.data, 'settings.json'), 'utf8'),
  );
  expect(settings).toMatchObject({ rotation: 'perishable', reserve: '8', quota_poll: '240' });
});
test('opens exact verification link and checks continued verification then readiness', async ({
  page,
  engine,
}) => {
  const url = 'https://accounts.google.com/signin/continue?fixture=one';
  await fakeConsent(page);
  await engine.scenario({ metadata_by_slot: { 'one@example.com': { verify_url: url } } });
  await openAccounts(page, engine);
  const one = page.getByRole('article', { name: 'one@example.com', exact: true });
  await one.getByRole('button', { name: 'Check', exact: true }).click();
  await expect(one.getByRole('link', { name: 'Verify with Google' })).toHaveAttribute('href', url);
  const popup = page.waitForEvent('popup');
  await one.getByRole('link', { name: 'Verify with Google' }).click();
  const tab = await popup;
  await expect(tab.getByText('Synthetic consent fixture.')).toBeVisible();
  await tab.close();
  await one.getByRole('button', { name: 'Check again', exact: true }).click();
  await expect(page.getByText('Still not verified.', { exact: true })).toBeVisible();
  await engine.scenario({});
  await one.getByRole('button', { name: 'Check again', exact: true }).click();
  await expect(page.getByText('one@example.com is ready.', { exact: true })).toBeVisible();
  await expect(one.getByRole('link', { name: 'Verify with Google' })).not.toBeVisible();
});
test('signs in synthetically with Enter and removes the adopted account', async ({
  page,
  engine,
}) => {
  await fakeConsent(page);
  await engine.scenario({
    login: {
      email: 'new@example.com',
      code: 'good-code',
      url: 'https://accounts.google.com/o/oauth2/auth?client_id=stub&state=stub',
    },
  });
  await openAccounts(page, engine);
  const dialog = await startLogin(page);
  await expect(dialog.getByRole('link', { name: 'Reopen the page' })).toHaveAttribute(
    'href',
    'https://accounts.google.com/o/oauth2/auth?client_id=stub&state=stub',
  );
  const code = dialog.getByRole('textbox', { name: 'Sign-in code' });
  await code.fill('good-code');
  await code.press('Enter');
  await expect(dialog.getByRole('status')).toContainText('Signed in: new@example.com');
  await expect(dialog).not.toBeVisible();
  const account = page.getByRole('article', { name: 'new@example.com', exact: true });
  await expect(account).toBeVisible();
  await account.getByRole('button', { name: 'Sign in again' }).click();
  await expect(dialog).toBeVisible();
  await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
  await account.getByRole('button', { name: 'Remove', exact: true }).click();
  await page
    .getByRole('dialog', { name: 'Remove new@example.com?' })
    .getByRole('button', { name: 'Remove', exact: true })
    .click();
  await expect(account).not.toBeVisible();
});
test('cancels old links and handles rejected sign-in followed by automatic pasted-code success', async ({
  page,
  engine,
}) => {
  await fakeConsent(page);
  await engine.scenario({
    login: {
      email: 'paste@example.com',
      code: '4/abcdefghijklmnopqrstuv',
      url: 'https://accounts.google.com/o/oauth2/auth?client_id=stub&state=stub',
    },
  });
  await openAccounts(page, engine);
  const dialog = await startLogin(page);
  await dialog.getByRole('button', { name: 'New sign-in link' }).click();
  await expect(dialog.getByRole('textbox', { name: 'Sign-in code' })).toBeEnabled();
  await dialog.getByRole('textbox', { name: 'Sign-in code' }).fill('bad-code');
  await dialog.getByRole('button', { name: 'Add account', exact: true }).click();
  await expect(dialog.getByRole('alert')).toContainText('Get a new sign-in link');
  await expect(dialog.getByRole('textbox', { name: 'Sign-in code' })).toBeDisabled();
  await dialog.getByRole('button', { name: 'New sign-in link' }).click();
  await expect(dialog.getByRole('textbox', { name: 'Sign-in code' })).toBeEnabled();
  await dialog.getByRole('textbox', { name: 'Sign-in code' }).evaluate((input) => {
    const clipboardData = new DataTransfer();
    clipboardData.setData('text', '4/abcdefghijklmnopqrstuv');
    input.dispatchEvent(new ClipboardEvent('paste', { bubbles: true, clipboardData }));
  });
  await expect(dialog.getByRole('status')).toContainText('Signed in: paste@example.com');
  await expect(dialog).not.toBeVisible();
  await expect(page.getByRole('article', { name: 'paste@example.com', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Add account', exact: true }).click();
  await dialog.getByRole('button', { name: 'Open Google sign-in' }).click();
  await expect(dialog.getByRole('textbox', { name: 'Sign-in code' })).toBeEnabled();
  await page.keyboard.press('Escape');
  await expect(dialog).not.toBeVisible();
});
test('shows sign-in verification and preserves the parked account', async ({ page, engine }) => {
  await fakeConsent(page);
  const verify = 'https://accounts.google.com/signin/continue?fixture=new';
  await engine.scenario({
    login: {
      email: 'verify@example.com',
      code: 'good-code',
      url: 'https://accounts.google.com/o/oauth2/auth?client_id=stub&state=stub',
      verify_url: verify,
    },
  });
  await openAccounts(page, engine);
  const dialog = await startLogin(page);
  await dialog.getByRole('textbox', { name: 'Sign-in code' }).fill('good-code');
  await dialog.getByRole('button', { name: 'Add account', exact: true }).click();
  await expect(dialog.getByRole('link', { name: 'Verify with Google' })).toHaveAttribute(
    'href',
    verify,
  );
  await expect(dialog.getByRole('textbox', { name: 'Sign-in code' })).toBeDisabled();
  await expect(dialog.getByRole('button', { name: 'Add account', exact: true })).toBeDisabled();
  await screenshot(page, engine, 'login-verification');
  await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(
    page.getByRole('article', { name: 'verify@example.com', exact: true }),
  ).toBeVisible();
});
test('keeps account settings legible in light theme and narrow windows with keyboard close', async ({
  page,
  engine,
}) => {
  await addAccount(engine, 'two@example.com', { paused: true });
  await page.emulateMedia({ colorScheme: 'light' });
  await openAccounts(page, engine);
  await screenshot(page, engine, 'accounts-light');
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(page.getByRole('article', { name: 'two@example.com', exact: true })).toBeVisible();
  await screenshot(page, engine, 'accounts-narrow-light');
  expect(
    await page
      .getByRole('dialog', { name: 'Accounts', exact: true })
      .evaluate((dialog) => dialog.scrollWidth <= dialog.clientWidth),
  ).toBe(true);
  await page.keyboard.press('Escape');
  await expect(page.getByRole('heading', { name: 'Accounts', exact: true })).not.toBeVisible();
  await page.keyboard.press('Control+,');
  await expect(page.getByRole('heading', { name: 'Accounts', exact: true })).toBeVisible();
});
for (const scenario of ['quota', 'rate', 'auth'])
  test(`${scenario} handoff preserves one user message and shows the next serving account`, async ({
    page,
    engine,
  }) => {
    await addAccount(engine, 'two@example.com');
    await engine.scenario({
      scenarios_by_slot_and_turn: { 'one@example.com': [scenario], 'two@example.com': ['success'] },
      metadata_by_slot: {
        'one@example.com': {
          quota: quotaResponse(),
          ...(scenario === 'quota' ? { quota_after_turn: quotaResponse(0.01) } : {}),
        },
      },
      reply: 'Handoff recovered fixture reply.',
    });
    await create(page, engine, `${scenario} handoff`);
    const prompt = page.getByRole('textbox', { name: 'Message', exact: true });
    await prompt.fill('Continue once on another account.');
    await page.getByRole('button', { name: 'Send', exact: true }).click();
    await expect(page.getByLabel('Agent reply').last()).toContainText(
      'Handoff recovered fixture reply.',
    );
    await expect(page.getByLabel('Your message')).toHaveCount(1);
    await expect(page.getByRole('button', { name: 'Accounts and quota' })).toContainText(
      'two · Gemini',
    );
    await page.getByRole('button', { name: 'Accounts and quota' }).click();
    const routing = page.getByLabel('Routing log');
    await expect(routing.locator('.row').first()).toContainText('two');
    await expect(routing.locator('.row').first()).toContainText(
      `serves ${scenario} handoff (was one)`,
    );
    await expect(routing).toContainText(
      scenario === 'quota' ? 'walled until' : scenario === 'rate' ? 'rests' : 'sign in again',
    );
    await expect(
      page.getByRole('article', { name: 'two@example.com', exact: true }).locator('.today'),
    ).toContainText('today ↑ 100 ↓ 10 · 1 turn');
    await page.setViewportSize({ width: 1280, height: 1050 });
    await routing.locator('.row').first().scrollIntoViewIfNeeded();
    await screenshot(page, engine, `handoff-${scenario}`);
  });
test('honors an older URL-fragment selection when several sessions exist', async ({
  page,
  engine,
}) => {
  await create(page, engine, 'Older chosen session');
  const older = page.url();
  await create(page, engine, 'Newest unchosen session');
  await page.goto(older);
  await page.reload();
  await expect(
    page.getByRole('heading', { name: 'Older chosen session', exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole('heading', { name: 'Newest unchosen session', exact: true }),
  ).not.toBeVisible();
});

test('shows a leased account as running and disables check and removal until stop', async ({
  page,
  engine,
}) => {
  await engine.scenario({ scenario: 'delay', delay_ms: 10000 });
  await create(page, engine, 'Busy account');
  await page
    .getByRole('textbox', { name: 'Message', exact: true })
    .fill('Hold this account briefly.');
  await page.getByRole('button', { name: 'Send', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Stop', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Accounts and quota' }).click();
  const one = page.getByRole('article', { name: 'one@example.com', exact: true });
  await expect(one.locator('.probe')).toContainText('running');
  await expect(one.getByRole('button', { name: 'Check', exact: true })).toBeDisabled();
  await expect(one.getByRole('button', { name: 'Remove', exact: true })).toBeDisabled();
  await page.getByRole('button', { name: 'Close settings' }).click();
  await page.getByRole('button', { name: 'Stop', exact: true }).click();
  await expect(page.locator('.session-state .status')).toHaveText('idle');
});

test('reports account API errors without discarding data and accepts a later successful check', async ({
  page,
  engine,
}) => {
  await openAccounts(page, engine);
  const one = page.getByRole('article', { name: 'one@example.com', exact: true });
  let failed = false;
  await page.route('**/api/accounts/one%40example.com/check', async (route) => {
    if (!failed) {
      failed = true;
      await route.fulfill({
        status: 400,
        contentType: 'application/json',
        body: JSON.stringify({ error: 'Fixture account is temporarily busy.' }),
      });
    } else await route.continue();
  });
  await one.getByRole('button', { name: 'Check', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Fixture account is temporarily busy.');
  await expect(one).toBeVisible();
  await one.getByRole('button', { name: 'Check', exact: true }).click();
  await expect(page.getByText('one@example.com is ready.', { exact: true })).toBeVisible();
  await expect(page.getByRole('alert')).not.toBeVisible();
});

test('offers a manual consent link when popups are blocked and accepts polled completion', async ({
  page,
  engine,
}) => {
  await fakeConsent(page);
  await page.addInitScript(() => {
    window.open = () => null;
  });
  await engine.scenario({
    login: {
      email: 'automatic@example.com',
      url: 'https://accounts.google.com/o/oauth2/auth?client_id=stub&state=stub',
      auto_complete_ms: 1000,
    },
  });
  await openAccounts(page, engine);
  const dialog = await startLogin(page);
  await expect(dialog.getByRole('link', { name: 'Google sign-in page' })).toHaveAttribute(
    'href',
    'https://accounts.google.com/o/oauth2/auth?client_id=stub&state=stub',
  );
  await expect(dialog.getByRole('status')).toContainText('Signed in: automatic@example.com');
  await screenshot(page, engine, 'login-polled-completion');
  await expect(dialog).not.toBeVisible();
  await expect(
    page.getByRole('article', { name: 'automatic@example.com', exact: true }),
  ).toBeVisible();
});
