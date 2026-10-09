/** Proves the development entry point: Vite serves the UI and proxies a working fake-agent engine. */
import { test, expect } from '@playwright/test';
import { spawn, spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { join } from 'node:path';

const UI = 'http://127.0.0.1:5173';

async function answers(url: string) {
  try {
    return (await fetch(url)).ok;
  } catch {
    return false;
  }
}

test('just dev runs Vite against a sandbox engine and stops both', async ({ page }) => {
  test.setTimeout(90_000);
  expect(await answers(UI), 'port 5173 must be free').toBe(false);
  const dev = spawn(process.execPath, ['scripts/dev.ts'], {
    cwd: join(import.meta.dirname, '..'),
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  let log = '';
  let data: string | undefined;
  dev.stdout.on('data', (chunk: Buffer) => (log += chunk.toString()));
  dev.stderr.on('data', (chunk: Buffer) => (log += chunk.toString()));
  try {
    await expect.poll(() => answers(UI), { timeout: 60_000 }).toBe(true);
    expect(log).toContain('Fake agent only.');
    data = /Development data: (.+)/.exec(log)?.[1]?.trim();
    expect(data?.split(/[\\/]/).join('/')).toContain('/Desktop/test/antiquota-dev');
    await page.goto(UI);
    await expect(page.getByText('Engine connected')).toBeVisible();
    // The UI's own token reaches the engine through the Vite proxy.
    const health = await page.evaluate(async () => (await fetch('/api/health')).json());
    expect(health).toMatchObject({ ok: true, app: 'antiquota' });
    const state = await page.evaluate(async () => {
      const token = document.querySelector<HTMLMetaElement>(
        'meta[name="antiquota-token"]',
      )?.content;
      const response = await fetch('/api/state', { headers: { 'X-Antiquota-Token': token ?? '' } });
      return { status: response.status, body: await response.json() };
    });
    expect(state.status).toBe(200);
    expect(state.body.defaults.model).toBe('gemini-3.8-flash-high');
  } finally {
    if (process.platform === 'win32') {
      // Windows has no SIGINT for a child: end the whole tree instead.
      spawnSync('taskkill', ['/pid', String(dev.pid), '/T', '/F']);
    } else {
      dev.kill('SIGINT');
    }
  }
  await new Promise((resolve) => (dev.exitCode === null ? dev.once('exit', resolve) : resolve(0)));
  await expect.poll(() => answers(UI), { timeout: 15_000 }).toBe(false);
  // A clean stop shuts the engine down, which removes its discovery file.
  if (process.platform !== 'win32')
    expect(existsSync(join(data ?? '', 'data', 'state', 'engine.json'))).toBe(false);
});
