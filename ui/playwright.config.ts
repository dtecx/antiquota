/** Owns headless Chromium checks against an isolated Rust server and fake agent. */
import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: './e2e',
  fullyParallel: false,
  workers: 1,
  timeout: 35_000,
  expect: { timeout: 8_000 },
  outputDir: '../.handoff/evidence/ui-playwright',
  reporter: [['list'], ['json', { outputFile: '../.handoff/evidence/ui-e2e.json' }]],
  use: {
    browserName: 'chromium',
    headless: true,
    viewport: { width: 1280, height: 820 },
    colorScheme: 'dark',
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },
});
