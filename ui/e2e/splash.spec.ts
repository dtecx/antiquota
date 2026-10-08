/** Proves the actual bundled splash page with deferred, mocked Tauri registration only. */
import { mkdir, readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { test, expect, type Page } from '@playwright/test';

type SplashMock = {
  calls: string[];
  event: string;
  failInvoke: string;
  release: () => void;
  reject: (message: string) => void;
  emit: (text: string, error: boolean) => void;
};
async function screenshot(page: Page, state: string) {
  const directory = fileURLToPath(new URL('../../.handoff/evidence/ui-slice10/', import.meta.url));
  await mkdir(directory, { recursive: true });
  await page.screenshot({ path: join(directory, `mocked-native-splash-${state}.png`) });
}
async function load(page: Page) {
  await page.evaluate(() => {
    let finish!: (stop: () => void) => void;
    let reject!: (error: Error) => void;
    let deliver: ((event: { payload: { text: string; error: boolean } }) => void) | undefined;
    const registration = new Promise<() => void>((resolve, fail) => {
      finish = resolve;
      reject = fail;
    });
    const mock: SplashMock = {
      calls: [],
      event: '',
      failInvoke: '',
      release: () => finish(() => {}),
      reject: (message) => reject(new Error(message)),
      emit: (text, error) => deliver?.({ payload: { text, error } }),
    };
    Object.assign(window, {
      splashMock: mock,
      __TAURI__: {
        event: {
          listen(event: string, callback: typeof deliver) {
            mock.event = event;
            deliver = callback;
            return registration;
          },
        },
        core: {
          async invoke(command: string) {
            mock.calls.push(command);
            if (mock.failInvoke) throw new Error(mock.failInvoke);
          },
        },
      },
    });
  });
  await page.setContent(
    await readFile(new URL('../../app/splash/index.html', import.meta.url), 'utf8'),
  );
}
const calls = (page: Page) =>
  page.evaluate(() => (window as unknown as { splashMock: SplashMock }).splashMock.calls);
async function release(page: Page) {
  await page.evaluate(() => (window as unknown as { splashMock: SplashMock }).splashMock.release());
  await expect.poll(() => calls(page)).toEqual(['retry_engine']);
}

test('bootstrap waits for the status listener before requesting engine startup', async ({
  page,
}) => {
  await load(page);
  await expect(page).toHaveTitle('antiquota');
  await expect(page.getByRole('status')).toHaveText('Starting the antiquota engine…');
  await expect(page.getByRole('button', { name: 'Try again' })).toBeHidden();
  await expect(page.getByRole('button', { name: 'Quit', exact: true })).toBeHidden();
  expect(
    await page.evaluate(() => (window as unknown as { splashMock: SplashMock }).splashMock.event),
  ).toBe('engine-status');
  expect(await calls(page)).toEqual([]);
  await expect(page.locator('.mark i')).toHaveCount(2);
  await expect(page.locator('.mark i').first()).toHaveCSS('animation-name', 'pass');
  await release(page);
  await screenshot(page, 'starting');
});

test('status errors expose retry and quit and retry restores the starting presentation', async ({
  page,
}) => {
  await load(page);
  await release(page);
  await page.evaluate(() =>
    (window as unknown as { splashMock: SplashMock }).splashMock.emit(
      'Synthetic startup failure <script>not markup</script>',
      true,
    ),
  );
  await expect(page.getByRole('status')).toHaveText(
    'Synthetic startup failure <script>not markup</script>',
  );
  await expect(page.locator('#main')).toHaveClass('error');
  await expect(page.locator('#text script')).toHaveCount(0);
  await expect(page.locator('.mark i').first()).toHaveCSS('animation-name', 'none');
  await expect(page.getByRole('button', { name: 'Try again' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Quit', exact: true })).toBeVisible();
  await screenshot(page, 'error');
  await page.getByRole('button', { name: 'Try again' }).click();
  await expect.poll(() => calls(page)).toEqual(['retry_engine', 'retry_engine']);
  await expect(page.getByRole('status')).toHaveText('Starting the antiquota engine…');
  await expect(page.locator('#main')).not.toHaveClass('error');
  await expect(page.getByRole('button', { name: 'Try again' })).toBeHidden();
  await page.evaluate(() =>
    (window as unknown as { splashMock: SplashMock }).splashMock.emit(
      'Synthetic second failure',
      true,
    ),
  );
  await page.getByRole('button', { name: 'Quit', exact: true }).click();
  await expect.poll(() => calls(page)).toEqual(['retry_engine', 'retry_engine', 'quit_app']);
  await page.evaluate(() =>
    (window as unknown as { splashMock: SplashMock }).splashMock.emit(
      'Synthetic ready status',
      false,
    ),
  );
  await expect(page.getByRole('status')).toHaveText('Synthetic ready status');
  await expect(page.locator('#main')).not.toHaveClass('error');
  await expect(page.getByRole('button', { name: 'Quit', exact: true })).toBeHidden();
});

test('reduced motion disables both splash animations', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await load(page);
  await expect(page.locator('.mark i').first()).toHaveCSS('animation-name', 'none');
  await expect(page.locator('.mark i').last()).toHaveCSS('animation-name', 'none');
  expect(await calls(page)).toEqual([]);
  await release(page);
});

test('failed listener registration exposes its error without starting an unobserved engine', async ({
  page,
}) => {
  await load(page);
  await page.evaluate(() =>
    (window as unknown as { splashMock: SplashMock }).splashMock.reject(
      'Synthetic listener failure',
    ),
  );
  await expect(page.getByRole('status')).toHaveText('Error: Synthetic listener failure');
  await expect(page.locator('#main')).toHaveClass('error');
  await expect(page.getByRole('button', { name: 'Try again' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Quit', exact: true })).toBeVisible();
  expect(await calls(page)).toEqual([]);
});

test('failed initial startup invocation appears as a recoverable splash error', async ({
  page,
}) => {
  await load(page);
  await page.evaluate(() => {
    const mock = (window as unknown as { splashMock: SplashMock }).splashMock;
    mock.failInvoke = 'Synthetic startup refusal';
    mock.release();
  });
  await expect(page.getByRole('status')).toHaveText('Error: Synthetic startup refusal');
  await expect(page.getByRole('button', { name: 'Try again' })).toBeVisible();
  expect(await calls(page)).toEqual(['retry_engine']);
});
