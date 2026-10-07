/** Proves approval cards, the hook round trip and permission settings in a real browser. */
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
  const session = state.sessions.find((item) => item.title === title);
  if (!session) throw new Error('created session is missing');
  return session.id;
}
async function screenshot(page: Page, engine: FixtureEngine, name: string) {
  const directory = join(engine.sandbox.repository, '.handoff', 'evidence', 'ui-slice3');
  await mkdir(directory, { recursive: true });
  await page.screenshot({ path: join(directory, `${name}.png`), fullPage: true });
}

test('answers approval cards and keeps rules in settings', async ({ page, engine }) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  const id = await create(page, engine, 'Approval fixture');
  const ask = (command: string) =>
    engine.call('POST', '/api/approvals', {
      session: id,
      tool: 'run_command',
      args: { CommandLine: command },
    });
  await ask('touch approved-file');
  const card = page.getByRole('group', { name: 'Approval needed' });
  await expect(card).toContainText('agy wants to run a command');
  await expect(card).toContainText('touch approved-file');
  await expect(card).toContainText('waits 15 min');
  await expect(page.getByText('needs your approval')).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Approval fixture' }).getByText('approval', { exact: true }),
  ).toBeVisible();
  await screenshot(page, engine, 'approval-pending');
  await card.getByRole('button', { name: 'Always allow command(touch)' }).click();
  await expect(card).toHaveCount(0);
  await expect(page.getByText('✓ allowed')).toBeVisible();
  await expect(page.getByText('always: command(touch)')).toBeVisible();

  await ask('rm -rf build');
  await expect(card).toContainText('rm -rf build');
  await card.getByRole('button', { name: 'Decline' }).click();
  await expect(page.getByText('✕ declined')).toBeVisible();
  await screenshot(page, engine, 'approval-answered');

  await page.keyboard.press('ControlOrMeta+,');
  const settings = page.getByRole('region', { name: 'Permissions' });
  const allow = page.getByRole('list', { name: 'Allow without asking' });
  await expect(allow).toContainText('command(touch)');
  await page.getByRole('textbox', { name: 'New allow rule' }).fill('command(bash)');
  await settings.getByRole('button', { name: 'Add', exact: true }).first().click();
  await expect(page.getByRole('alert').filter({ hasText: 'arbitrary programs' })).toBeVisible();
  await page.getByRole('textbox', { name: 'New allow rule' }).fill('command(npm test)');
  await settings.getByRole('button', { name: 'Add', exact: true }).first().click();
  await expect(allow).toContainText('command(npm test)');
  await page.getByRole('textbox', { name: 'New deny rule' }).fill('command(rm)');
  await settings.getByRole('button', { name: 'Add', exact: true }).nth(1).click();
  await expect(page.getByRole('list', { name: 'Always refuse' })).toContainText('command(rm)');
  await page.getByRole('button', { name: 'Remove command(touch)' }).click();
  await expect(allow).not.toContainText('command(touch)');
  await page.getByRole('spinbutton', { name: 'Approval wait' }).fill('30');
  await page.getByRole('spinbutton', { name: 'Approval wait' }).press('Enter');
  await expect(page.getByText('Approval wait saved.')).toBeVisible();
  await screenshot(page, engine, 'permissions-settings');
  const rules = JSON.parse((await engine.call('GET', '/api/settings')).text) as {
    permissions: { allow: string[]; deny: string[] };
    defaults: { approval_timeout: string };
  };
  expect(rules.permissions.deny).toEqual(['command(rm)']);
  expect(rules.permissions.allow).toEqual(['command(npm test)']);
  expect(rules.defaults.approval_timeout).toBe('30');
  expect(errors).toEqual([]);
});

test('relays a browser answer to a waiting agy tool call', async ({ page, engine }) => {
  await engine.scenario({
    scenario: 'approval',
    hook_command: [engine.sandbox.executable('antiquota'), 'hook', 'pre-tool'],
    hook_payload: {
      toolCall: { name: 'run_command', args: { CommandLine: 'touch from-the-browser' } },
    },
  });
  await create(page, engine, 'Hook fixture');
  await page.getByRole('textbox', { name: 'Message', exact: true }).fill('Run the tool.');
  await page.getByRole('button', { name: 'Send', exact: true }).click();
  const card = page.getByRole('group', { name: 'Approval needed' });
  await expect(card).toContainText('touch from-the-browser');
  await card.getByRole('button', { name: 'Allow once' }).click();
  await expect(page.getByText('✓ allowed')).toBeVisible();
  await expect(page.locator('.turn-end')).toBeVisible();
  await page.getByText('Agent activity', { exact: true }).click();
  await expect(page.locator('.tool').first()).toContainText('done');
});
