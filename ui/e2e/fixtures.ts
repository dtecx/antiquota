/** Owns browser-test isolation and verified engine teardown with local evidence. */
import { test as base, expect } from '@playwright/test';
import { copyFile, mkdir, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { createSandbox } from '../scripts/sandbox.ts';
import { FixtureEngine } from '../scripts/engine.ts';

export const test = base.extend<{ engine: FixtureEngine }>({
  // Existing scenarios assert the verbose transcript; compact and full tests opt in explicitly.
  page: async ({ page }, use) => {
    await page.addInitScript(() => {
      try {
        if (!localStorage.getItem('antiquota.detail'))
          localStorage.setItem('antiquota.detail', 'verbose');
      } catch {
        /* Storage can be unavailable. */
      }
    });
    await use(page);
  },
  engine: async ({ browserName }, use, info) => {
    void browserName;
    const sandbox = await createSandbox('antiquota-ui-e2e');
    const engine = new FixtureEngine(sandbox);
    try {
      await engine.start();
      await use(engine);
    } finally {
      await engine.stop();
      const evidence = join(
        sandbox.repository,
        '.handoff',
        'evidence',
        info.file.endsWith('sessions.spec.ts') ? 'ui-slice1' : 'ui-slice2',
      );
      await mkdir(evidence, { recursive: true });
      const label = info.title.replace(/[^a-zA-Z0-9]+/g, '-').slice(0, 100);
      await copyFile(join(sandbox.root, 'server.log'), join(evidence, `${label}.log`)).catch(
        () => {},
      );
      await copyFile(
        join(sandbox.root, 'fake-calls.jsonl'),
        join(evidence, `${label}-fake-calls.jsonl`),
      ).catch(() => {});
      await rm(sandbox.root, { recursive: true, force: true });
    }
  },
});
export { expect };
