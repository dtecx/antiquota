/** Proves the Changes and Files tabs, the file viewer and a worktree session in a real browser. */
import type { Page } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { test, expect } from './fixtures';
import type { FixtureEngine } from '../scripts/engine.ts';

function git(dir: string, ...args: string[]) {
  return execFileSync('git', args, { cwd: dir, encoding: 'utf8' });
}
async function repository(engine: FixtureEngine) {
  const project = engine.sandbox.project;
  git(project, 'init', '-q', '-b', 'main');
  git(project, 'config', 'user.email', 'test@example.com');
  git(project, 'config', 'user.name', 'Test');
  git(project, 'config', 'commit.gpgsign', 'false');
  await mkdir(join(project, 'src'), { recursive: true });
  await writeFile(join(project, 'readme.txt'), 'Fixture repository.\n');
  await writeFile(join(project, 'src', 'app.ts'), 'export const a = 1;\nexport const b = 2;\n');
  git(project, 'add', '.');
  git(project, 'commit', '-q', '-m', 'first');
}
async function create(page: Page, engine: FixtureEngine, title: string, worktree = false) {
  await page.goto(engine.url);
  await expect(page.getByText('Engine connected')).toBeVisible();
  await page.getByRole('button', { name: 'New session' }).first().click();
  await page.getByRole('textbox', { name: 'Session name' }).fill(title);
  await page.getByRole('textbox', { name: 'Project folder' }).fill(engine.sandbox.project);
  if (worktree) await page.getByRole('checkbox', { name: 'Own git worktree' }).check();
  await page.getByRole('button', { name: 'Create session' }).click();
  await expect(page.getByRole('heading', { name: title, exact: true })).toBeVisible();
}
async function screenshot(page: Page, engine: FixtureEngine, name: string) {
  const directory = join(engine.sandbox.repository, '.handoff', 'evidence', 'ui-slice6');
  await mkdir(directory, { recursive: true });
  await page.screenshot({ path: join(directory, `${name}.png`), fullPage: true });
}

test('reviews, discards and commits changes in the main checkout', async ({ page, engine }) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await repository(engine);
  const project = engine.sandbox.project;
  await writeFile(join(project, 'readme.txt'), 'Changed.\nTwice.\n');
  await writeFile(join(project, 'scratch.txt'), 'temporary\n');
  await create(page, engine, 'Review fixture');
  await page.keyboard.press('ControlOrMeta+b');
  const panel = page.getByRole('complementary', { name: 'Side panel' });
  await expect(panel).toContainText(`main · `);
  const files = panel.getByRole('list', { name: 'Uncommitted files' });
  await expect(files).toContainText('readme.txt');
  await expect(files).toContainText('+2');
  await files.getByText('readme.txt').click();
  await expect(panel.locator('pre.diff').first()).toContainText('+Changed.');
  await screenshot(page, engine, 'changes');
  await files.getByText('scratch.txt').click();
  await files
    .locator('li')
    .filter({ hasText: 'scratch.txt' })
    .getByRole('button', { name: 'Discard' })
    .click();
  const confirm = page.getByRole('dialog', { name: 'Discard scratch.txt?' });
  await expect(confirm).toContainText('The new file is deleted.');
  await confirm.getByRole('button', { name: 'Discard' }).click();
  await expect(files).not.toContainText('scratch.txt');
  expect(existsSync(join(project, 'scratch.txt'))).toBe(false);
  await expect(panel.getByRole('textbox', { name: 'Commit message' })).toHaveValue(
    'Review fixture',
  );
  await panel.getByRole('button', { name: 'Commit all' }).click();
  await expect(panel).toContainText('No uncommitted changes.');
  expect(git(project, 'log', '-1', '--format=%s').trim()).toBe('Review fixture');
  await expect(page.getByText(/^Committed [0-9a-f]+: Review fixture$/)).toBeVisible();
  expect(errors).toEqual([]);
});

test('browses files, views one and mentions it', async ({ page, engine }) => {
  await repository(engine);
  await create(page, engine, 'Files fixture');
  await page.getByRole('button', { name: 'Changes and files' }).click();
  await page.getByRole('tab', { name: 'Files' }).click();
  const contents = page.getByRole('list', { name: 'Folder contents' });
  await expect(contents).toContainText('src/');
  await contents.getByRole('button', { name: 'src/' }).click();
  await expect(page.getByRole('navigation', { name: 'Folder path' })).toContainText('project/src');
  await contents.locator('.row', { hasText: 'app.ts' }).click();
  const viewer = page.getByRole('dialog', { name: 'src/app.ts' });
  await expect(viewer.getByLabel('File contents')).toContainText('1export const a = 1;');
  await expect(viewer.getByLabel('File contents')).toContainText('2export const b = 2;');
  await screenshot(page, engine, 'viewer');
  await page.mouse.click(5, 5);
  await expect(viewer).toBeHidden();
  await contents.getByRole('button', { name: 'Mention app.ts' }).click();
  await expect(page.getByRole('textbox', { name: 'Message', exact: true })).toHaveValue(
    '@src/app.ts ',
  );
  await contents.getByRole('button', { name: '..' }).click();
  await expect(contents).toContainText('readme.txt');
});

test('works in a worktree, merges it and removes it', async ({ page, engine }) => {
  await repository(engine);
  await create(page, engine, 'Worktree fixture', true);
  const header = page.locator('header').first();
  await expect(header).toContainText(/antiquota\/worktree-fixture-[0-9a-f]{4}/);
  const state = JSON.parse((await engine.call('GET', '/api/state')).text) as {
    sessions: { id: string; cwd: string; worktree: { path: string } | null }[];
  };
  const session = state.sessions[0]!;
  expect(session.worktree?.path).toBe(session.cwd);
  await writeFile(join(session.cwd, 'feature.txt'), 'New feature.\n');
  await page.keyboard.press('ControlOrMeta+b');
  const panel = page.getByRole('complementary', { name: 'Side panel' });
  await expect(panel.getByRole('button', { name: 'Merge into main' })).toBeDisabled();
  await panel.getByRole('button', { name: 'Commit all' }).click();
  await expect(panel.getByRole('list', { name: 'Committed files' })).toContainText('feature.txt');
  await expect(panel).toContainText('1 ahead');
  await screenshot(page, engine, 'worktree');
  await panel.getByRole('button', { name: 'Merge into main' }).click();
  await expect(panel).toContainText('merged');
  expect(await readFile(join(engine.sandbox.project, 'feature.txt'), 'utf8')).toBe(
    'New feature.\n',
  );
  await panel.getByRole('button', { name: 'Remove worktree' }).click();
  await page
    .getByRole('dialog', { name: 'Remove the worktree?' })
    .getByRole('button', { name: 'Remove worktree' })
    .click();
  await expect(page.getByText('Removed the worktree; the session is archived.')).toBeVisible();
  expect(existsSync(session.cwd)).toBe(false);
});
