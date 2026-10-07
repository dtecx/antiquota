/** Proves the goal dialog, goal bar controls and the plan bar in a real browser. */
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
  return state.sessions.find((item) => item.title === title)?.id ?? '';
}
async function screenshot(page: Page, engine: FixtureEngine, name: string) {
  const directory = join(engine.sandbox.repository, '.handoff', 'evidence', 'ui-slice4');
  await mkdir(directory, { recursive: true });
  await page.screenshot({ path: join(directory, `${name}.png`), fullPage: true });
}

test('starts a goal from the composer and follows it until met', async ({ page, engine }) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await engine.scenario({ done_after: 2, delay_ms: 300 });
  await create(page, engine, 'Goal fixture');
  await page.getByRole('textbox', { name: 'Message', exact: true }).fill('Complete fixture.');
  await page.getByRole('button', { name: '◎ Goal' }).click();
  const dialog = page.getByRole('dialog', { name: 'Goal' });
  await expect(dialog.getByRole('textbox', { name: 'Objective' })).toHaveValue('Complete fixture.');
  await dialog.getByRole('textbox', { name: 'Check command' }).fill('test -f notes.txt');
  await dialog.getByRole('spinbutton', { name: 'Rounds' }).fill('4');
  await screenshot(page, engine, 'goal-dialog');
  await dialog.getByRole('button', { name: 'Start goal' }).click();
  await expect(dialog).toBeHidden();
  await expect(page.getByRole('textbox', { name: 'Message', exact: true })).toHaveValue('');
  const bar = page.getByRole('region', { name: 'Goal' });
  await expect(bar).toContainText('Complete fixture.');
  await expect(bar).toContainText('check: test -f notes.txt');
  await expect(bar.getByRole('button', { name: 'Pause' })).toBeVisible();
  await expect(bar).toContainText('met', { timeout: 15_000 });
  await expect(page.getByText('Check `test -f notes.txt` exited 0.')).toBeVisible();
  await expect(page.getByText('Goal met.')).toBeVisible();
  await expect(bar.getByRole('button', { name: 'Resume' })).toHaveCount(0);
  await screenshot(page, engine, 'goal-met');
  await bar.getByRole('button', { name: 'Edit' }).click();
  await expect(dialog.getByRole('textbox', { name: 'Objective' })).toHaveValue('Complete fixture.');
  await expect(dialog.getByRole('textbox', { name: 'Check command' })).toHaveValue(
    'test -f notes.txt',
  );
  await dialog.getByRole('button', { name: 'Cancel' }).click();
  await bar.getByRole('button', { name: 'Remove goal' }).click();
  await expect(bar).toHaveCount(0);
  expect(errors).toEqual([]);
});

test('pauses and resumes a goal', async ({ page, engine }) => {
  await engine.scenario({ delay_ms: 3000 });
  const id = await create(page, engine, 'Pause fixture');
  await engine.call('POST', `/api/sessions/${id}/goal`, { objective: 'Finish.', verifier: false });
  const bar = page.getByRole('region', { name: 'Goal' });
  await expect(bar).toContainText('round 0/20');
  await bar.getByRole('button', { name: 'Pause' }).click();
  await expect(bar).toContainText('paused');
  await engine.scenario({});
  await bar.getByRole('button', { name: 'Resume' }).click();
  await expect(bar).toContainText('met', { timeout: 15_000 });
});

test('executes or revises a finished plan', async ({ page, engine }) => {
  const id = await create(page, engine, 'Plan fixture');
  await engine.call('PATCH', `/api/sessions/${id}`, { mode: 'plan' });
  await page.getByRole('textbox', { name: 'Message', exact: true }).fill('Plan a feature.');
  await page.getByRole('button', { name: 'Send', exact: true }).click();
  const plan = page.getByRole('region', { name: 'Plan ready' });
  await expect(plan).toBeVisible({ timeout: 15_000 });
  await screenshot(page, engine, 'plan-ready');
  await plan.getByRole('button', { name: 'Revise' }).click();
  await expect(page.getByPlaceholder('What should change in the plan?')).toBeFocused();
  await plan.getByRole('button', { name: 'Execute plan' }).click();
  await expect(plan).toHaveCount(0);
  await expect(page.getByLabel('Your message').last()).toContainText(
    'Go ahead: carry out the plan you just proposed',
  );
  const session = JSON.parse((await engine.call('GET', `/api/sessions/${id}`)).text) as {
    meta: { mode: string };
  };
  expect(session.meta.mode).toBe('accept-edits');
});
