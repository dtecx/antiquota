/** Proves the Terminal tab: a shell in the session folder, input, output, resize, clear, restart and kill. */
import type { Page } from '@playwright/test';
import { mkdir, readdir } from 'node:fs/promises';
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
}

test('runs a shell in the session folder', async ({ page, engine }) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await create(page, engine, 'Terminal fixture');
  await page.keyboard.press('ControlOrMeta+b');
  await page.getByRole('tab', { name: 'Terminal' }).click();
  await expect(page.locator('.xterm-helper-textarea')).toBeFocused();
  const title = page.locator('.terminal .title');
  await expect(title).toContainText(engine.sandbox.project);
  const output = page.getByLabel('Terminal output');
  await page.locator('.xterm-helper-textarea').focus();
  await page.keyboard.type('printf "term-%s\\n" ok; pwd');
  await page.keyboard.press('Enter');
  await expect(output.locator('.xterm-rows')).toContainText('term-ok');
  await expect(output.locator('.xterm-rows')).toContainText(engine.sandbox.project);
  const directory = join(engine.sandbox.repository, '.handoff', 'evidence', 'ui-slice7');
  await mkdir(directory, { recursive: true });
  await page.screenshot({ path: join(directory, 'terminal.png'), fullPage: true });
  await page.getByRole('tab', { name: 'Files' }).click();
  await page.getByRole('tab', { name: 'Terminal' }).click();
  await expect(page.locator('.xterm-helper-textarea')).toBeFocused();
  await expect(output.locator('.xterm-rows')).toContainText('term-ok', { timeout: 2000 });
  await page.getByRole('button', { name: 'Clear' }).click();
  await expect(output.locator('.xterm-rows')).not.toContainText('term-ok');
  await page.getByRole('button', { name: 'Kill' }).click();
  await expect(title).toContainText('· exited');
  await page.getByRole('button', { name: 'Restart' }).click();
  await expect(title).not.toContainText('exited');
  await page.locator('.xterm-helper-textarea').focus();
  await page.keyboard.type('echo again-$((40+2))');
  await page.keyboard.press('Enter');
  await expect(output.locator('.xterm-rows')).toContainText('again-42');
  // The shell itself sees the new size after the window narrows.
  let measurement = 0;
  const size = async () => {
    const marker = ++measurement;
    await page.getByRole('button', { name: 'Clear' }).click();
    await page.locator('.xterm-helper-textarea').focus();
    await page.keyboard.type(`echo "si""ze-${marker}-$(stty size | tr ' ' x)-end"`);
    await page.keyboard.press('Enter');
    const rows = output.locator('.xterm-rows');
    const pattern = new RegExp(`size-${marker}-(\\d+)x(\\d+)-end`);
    const measured = { rows: 0, cols: 0 };
    await expect
      .poll(async () => {
        const match = pattern.exec((await rows.textContent()) ?? '');
        if (!match) return false;
        measured.rows = Number(match[1]);
        measured.cols = Number(match[2]);
        return true;
      })
      .toBe(true);
    return measured;
  };
  const wide = await size();
  const sent: { cols: number; rows: number }[] = [];
  page.on('request', (request) => {
    if (request.url().endsWith('/term/resize')) sent.push(request.postDataJSON());
  });
  await page.setViewportSize({ width: 900, height: 820 });
  // The layout may settle in more than one step; the shell must end at the size sent last.
  await expect
    .poll(
      async () => {
        const now = await size();
        const last = sent.at(-1);
        return last !== undefined && now.cols === last.cols && now.rows === last.rows ? now : null;
      },
      { timeout: 10_000 },
    )
    .not.toBeNull();
  const narrow = await size();
  expect(narrow.cols).toBeLessThan(wide.cols);
  expect(narrow.rows).toBe(wide.rows);
  expect(errors).toEqual([]);
});

test('real xterm retains exactly five thousand scrollback lines beyond its visible rows', async ({
  page,
  engine,
}) => {
  const asset = (await readdir(join(import.meta.dirname, '../dist/assets'))).find((name) =>
    /^xterm-.*\.js$/.test(name),
  );
  expect(asset).toBeTruthy();
  await page.goto(engine.url);
  const retained = await page.evaluate(async (url) => {
    const { Terminal } = await import(url);
    const host = document.createElement('div');
    document.body.append(host);
    const terminal = new Terminal({ cols: 80, rows: 24, scrollback: 5000 });
    terminal.open(host);
    await new Promise<void>((resolve) =>
      terminal.write(Array.from({ length: 6000 }, (_, i) => `line ${i + 1}\r\n`).join(''), resolve),
    );
    const buffer = terminal.buffer.active;
    const result = {
      base: buffer.baseY,
      length: buffer.length,
      rows: terminal.rows,
      first: buffer.getLine(0)?.translateToString(true),
      last: buffer.getLine(buffer.length - 2)?.translateToString(true),
    };
    terminal.dispose();
    host.remove();
    return result;
  }, `${engine.url}/assets/${asset}`);
  expect(retained).toEqual({
    base: 5000,
    length: 5024,
    rows: 24,
    first: 'line 978',
    last: 'line 6000',
  });
});
