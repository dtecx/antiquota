/** Proves account history and display projections using compatible local fixture records. */
import { writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import type { RoutingEntry } from '../src/gen/RoutingEntry';
import { test, expect } from './fixtures';
import { addAccount, openAccounts, screenshot } from './account-support';
test('shows all four today counters and excludes earlier local-day usage', async ({
  page,
  engine,
}) => {
  const now = Date.now() / 1000;
  await writeFile(
    join(engine.sandbox.data, 'state', 'usage.jsonl'),
    [
      { t: now, slot: 'one@example.com', input: 100, output: 10, thinking: 7 },
      { t: now, slot: 'one@example.com', input: 200, output: 20, thinking: 5 },
      { t: now - 86400, slot: 'one@example.com', input: 9999, output: 9999, thinking: 9999 },
    ]
      .map((row) => JSON.stringify(row))
      .join('\n') + '\n',
  );
  await openAccounts(page, engine);
  await expect(
    page.getByRole('article', { name: 'one@example.com', exact: true }).locator('.today'),
  ).toHaveText('today input 300 · output 30 · thinking 12 · 2 turns');
  await screenshot(page, engine, 'accounts-today');
});
test('renders every routing kind newest first with the prior account and session', async ({
  page,
  engine,
}) => {
  const now = Date.now() / 1000;
  const records: RoutingEntry[] = [
    { ts: now - 7, event: 'wall', slot: 'one@example.com', why: 'quota spent', until: now + 1000 },
    {
      ts: now - 6,
      event: 'cooldown',
      slot: 'one@example.com',
      why: 'rate limited, quota left',
      secs: 60,
    },
    { ts: now - 5, event: 'retry', slot: 'one@example.com', wait: 5 },
    { ts: now - 4, event: 'signed-out', slot: 'one@example.com' },
    { ts: now - 3, event: 'paused', slot: 'one@example.com' },
    { ts: now - 2, event: 'resumed', slot: 'one@example.com' },
    {
      ts: now - 1,
      event: 'pick',
      slot: 'two@example.com',
      session: 'compatible-old-session',
      previous: 'one@example.com',
    },
  ];
  await writeFile(
    join(engine.sandbox.data, 'state', 'routing.jsonl'),
    records.map((row) => JSON.stringify(row)).join('\n') + '\n',
  );
  await openAccounts(page, engine);
  const rows = page.getByLabel('Routing log').locator('.row');
  await expect(rows).toHaveCount(7);
  await expect(rows.first()).toContainText('serves compatible-old-session (was one)');
  for (const [index, text] of [
    'resumed by you',
    'paused by you',
    'sign in again',
    'retry in 5s',
    'rests 1 min',
    'walled until',
  ].entries())
    await expect(rows.nth(index + 1)).toContainText(text);
  await page.setViewportSize({ width: 1280, height: 1100 });
  await rows.first().scrollIntoViewIfNeeded();
  await screenshot(page, engine, 'routing-all-kinds');
});
test('displays wall cooldown signed-out verification and paused attention states', async ({
  page,
  engine,
}) => {
  const until = Date.now() / 1000 + 3600;
  await addAccount(engine, 'wall@example.com', {
    walls: { gemini: until },
    why: { gemini: 'quota spent' },
  });
  await addAccount(engine, 'rest@example.com', {
    walls: { gemini: until },
    why: { gemini: 'rate limited, quota left' },
  });
  await addAccount(engine, 'out@example.com', {
    walls: { gemini: until },
    why: { gemini: 'signed out' },
  });
  await addAccount(engine, 'verify@example.com', {
    verify: 'https://accounts.google.com/signin/continue?fixture=display',
  });
  await addAccount(engine, 'paused@example.com', { paused: true });
  await openAccounts(page, engine);
  for (const [slot, text] of [
    ['wall', 'walled until'],
    ['rest', 'resting until'],
    ['out', 'signed out until'],
  ] as const)
    await expect(
      page.getByRole('article', { name: `${slot}@example.com`, exact: true }),
    ).toContainText(text);
  await expect(
    page
      .getByRole('article', { name: 'verify@example.com', exact: true })
      .getByRole('link', { name: 'Verify with Google' }),
  ).toHaveAttribute('href', 'https://accounts.google.com/signin/continue?fixture=display');
  await expect(page.locator('.pill')).toContainText('1 of 6 accounts free');
  await expect(page.locator('.pill')).toContainText('2 need attention');
  await expect(page.locator('.strip i.off')).toHaveCount(2);
  await expect(page.locator('.strip i.wall')).toHaveCount(3);
});
