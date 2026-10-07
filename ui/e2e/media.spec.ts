/** Proves composer attachments (picker, paste, drop, limits, thumbnails) and live speech in Chromium. */
import { test, expect } from './fixtures';
import { calls, create, evidence, transcriptions } from './media-support';

// A 2×2 red PNG.
const PNG = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAFklEQVR4nGP8z8DAwMDAxMDAwMDAAAANHQEDasKb6QAAAABJRU5ErkJggg==',
  'base64',
);

test('attaches files by picker, paste and drop, then sends them', async ({ page, engine }) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await create(page, engine, 'Attachments fixture');
  const tray = page.getByLabel('Attached files');
  await page.locator('input[type=file]').setInputFiles([
    { name: 'notes.txt', mimeType: 'text/plain', buffer: Buffer.from('Fixture notes.\n') },
    { name: 'red dot.png', mimeType: 'image/png', buffer: PNG },
  ]);
  await expect(tray.getByText('notes.txt')).toBeVisible();
  await expect(tray.getByText('15 B')).toBeVisible();
  await expect(tray.locator('.chip', { hasText: 'red dot.png' }).locator('img')).toBeVisible();
  await page.getByRole('textbox', { name: 'Message' }).focus();
  await page.evaluate(
    (bytes) => {
      const data = new DataTransfer();
      data.items.add(new File([new Uint8Array(bytes)], 'image.png', { type: 'image/png' }));
      const event = new ClipboardEvent('paste', {
        clipboardData: data,
        bubbles: true,
        cancelable: true,
      });
      document.querySelector('#prompt')?.dispatchEvent(event);
    },
    [...PNG],
  );
  await expect(tray.getByText(/^screenshot-\d{6}\.png$/)).toBeVisible();
  await page.evaluate(() => {
    const data = new DataTransfer();
    data.items.add(new File(['{"dropped":true}'], 'dropped.json', { type: 'application/json' }));
    window.dispatchEvent(new DragEvent('dragenter', { dataTransfer: data, cancelable: true }));
  });
  await expect(page.getByText('Drop files to attach')).toBeVisible();
  await page.evaluate(() => {
    const data = new DataTransfer();
    data.items.add(new File(['x'], 'x.txt'));
    window.dispatchEvent(new DragEvent('dragenter', { dataTransfer: data, cancelable: true }));
    window.dispatchEvent(new DragEvent('dragleave', { dataTransfer: data }));
  });
  await expect(page.getByText('Drop files to attach')).toBeVisible();
  await evidence(engine, page, 'dropzone.png');
  await page.evaluate(() => {
    const data = new DataTransfer();
    data.items.add(new File(['{"dropped":true}'], 'dropped.json', { type: 'application/json' }));
    window.dispatchEvent(new DragEvent('drop', { dataTransfer: data, cancelable: true }));
  });
  await expect(page.getByText('Drop files to attach')).toBeHidden();
  await expect(tray.getByText('dropped.json')).toBeVisible();
  await expect(tray.getByText('uploading…')).toHaveCount(0);
  await page.getByRole('button', { name: /^Remove screenshot-/ }).click();
  await expect(tray.locator('.chip')).toHaveCount(3);
  await evidence(engine, page, 'tray.png');
  await page.getByRole('textbox', { name: 'Message' }).fill('Look at these.');
  await page.getByRole('button', { name: 'Send' }).click();
  await expect(tray).toBeHidden();
  const message = page.getByRole('region', { name: 'Your message' });
  await expect(message.getByText('Look at these.')).toBeVisible();
  const thumbnail = message.getByRole('img', { name: 'red dot.png' });
  await expect(thumbnail).toBeVisible();
  await expect(message.getByRole('link', { name: 'red dot.png' })).toHaveAttribute(
    'href',
    /\/attachments\/red%20dot\.png\?token=/,
  );
  await expect.poll(() => thumbnail.evaluate((img: HTMLImageElement) => img.naturalWidth)).toBe(2);
  await expect(message.getByText('notes.txt')).toBeVisible();
  await expect(message.getByText('dropped.json')).toBeVisible();
  await evidence(engine, page, 'sent-attachments.png');
  await expect
    .poll(async () => (await calls(engine)).find((argv) => argv.includes('stream-json')) ?? [])
    .toContain('--add-dir');
  const turn = (await calls(engine)).find((argv) => argv.includes('stream-json')) ?? [];
  const prompt = turn[turn.indexOf('-p') + 1] ?? '';
  expect(prompt).toContain('Look at these.\n\nAttached files (open each with your file viewer');
  for (const name of ['notes.txt', 'red dot.png', 'dropped.json']) expect(prompt).toContain(name);
  expect(prompt).not.toContain('screenshot-');
  expect(errors).toEqual([]);
});

test('rejects oversized files and sends an attachment without text', async ({ page, engine }) => {
  await create(page, engine, 'Attachment limits fixture');
  await page.locator('input[type=file]').setInputFiles({
    name: 'huge.bin',
    mimeType: 'application/octet-stream',
    buffer: Buffer.alloc(25_000_001),
  });
  await expect(page.getByRole('status').getByText('huge.bin is over 25 MB')).toBeVisible();
  await expect(page.getByLabel('Attached files')).toBeHidden();
  const send = page.getByRole('button', { name: 'Send' });
  await expect(send).toBeDisabled();
  await page
    .locator('input[type=file]')
    .setInputFiles({ name: 'only.txt', mimeType: 'text/plain', buffer: Buffer.from('only') });
  await expect(page.getByLabel('Attached files').getByText('4 B')).toBeVisible();
  await expect(send).toBeEnabled();
  await send.click();
  await expect(
    page.getByRole('region', { name: 'Your message' }).getByText('only.txt'),
  ).toBeVisible();
});

test('fills the draft from live speech recognition', async ({ page, engine }) => {
  await page.addInitScript(() => {
    class Speech {
      continuous = false;
      interimResults = false;
      lang = '';
      onresult: ((event: unknown) => void) | null = null;
      onerror: ((event: unknown) => void) | null = null;
      onend: (() => void) | null = null;
      start() {
        setTimeout(
          () =>
            this.onresult?.({
              results: [[{ transcript: 'hello ' }], [{ transcript: 'from speech' }]],
            }),
          50,
        );
      }
      stop() {
        this.onend?.();
      }
      abort() {}
    }
    const scope = window as { webkitSpeechRecognition?: unknown; SpeechRecognition?: unknown };
    scope.SpeechRecognition = scope.webkitSpeechRecognition = Speech;
  });
  await create(page, engine, 'Speech fixture');
  const box = page.getByRole('textbox', { name: 'Message' });
  await box.fill('Say');
  await page.locator('button.mic').click();
  await expect(page.getByText('● listening')).toBeVisible();
  await expect(box).toHaveValue('Say hello from speech');
  await page.locator('button.mic').click();
  await expect(page.getByText('● listening')).toBeHidden();
  expect(transcriptions(await calls(engine))).toHaveLength(0);
});

test('failed and slow uploads, session switches and drops without a session', async ({
  page,
  engine,
}) => {
  await page.goto(engine.url);
  await expect(page.getByText('Engine connected')).toBeVisible();
  await page.evaluate(() => {
    const data = new DataTransfer();
    data.items.add(new File(['x'], 'early.txt'));
    window.dispatchEvent(new DragEvent('drop', { dataTransfer: data, cancelable: true }));
  });
  await expect(page.getByRole('alert')).toContainText('Open or create a session first.');
  await create(page, engine, 'Upload failures fixture');
  let release: () => void = () => {};
  const held = new Promise<void>((resolve) => (release = resolve));
  await page.route('**/attach', async (route) => {
    const name = decodeURIComponent(route.request().headers()['x-filename'] ?? '');
    if (name === 'broken.txt')
      return route.fulfill({ status: 400, json: { error: 'The upload was refused.' } });
    if (name === 'slow.txt') await held;
    return route.continue();
  });
  const picker = page.locator('input[type=file]');
  await picker.setInputFiles({
    name: 'broken.txt',
    mimeType: 'text/plain',
    buffer: Buffer.from('x'),
  });
  await expect(page.getByRole('alert')).toContainText('The upload was refused.');
  await expect(page.getByLabel('Attached files')).toBeHidden();
  await picker.setInputFiles({
    name: 'slow.txt',
    mimeType: 'text/plain',
    buffer: Buffer.from('s'),
  });
  await expect(page.getByLabel('Attached files').getByText('uploading…')).toBeVisible();
  await page.getByRole('textbox', { name: 'Message' }).fill('Wait for it.');
  await page.getByRole('button', { name: 'Send' }).click();
  await expect(page.getByRole('status').getByText('Still uploading…')).toBeVisible();
  release();
  await expect(page.getByLabel('Attached files').getByText('1 B')).toBeVisible();
  await page.getByRole('button', { name: 'New session' }).first().click();
  await page.getByRole('textbox', { name: 'Session name' }).fill('Second session');
  await page.getByRole('textbox', { name: 'Project folder' }).fill(engine.sandbox.project);
  await page.getByRole('button', { name: 'Create session' }).click();
  await expect(page.getByRole('heading', { name: 'Second session', exact: true })).toBeVisible();
  await page.getByRole('button', { name: /Upload failures fixture/ }).click();
  await expect(page.getByRole('heading', { name: 'Upload failures fixture' })).toBeVisible();
  await expect(page.getByLabel('Attached files')).toBeHidden();
  expect((await calls(engine)).filter((argv) => argv.includes('stream-json'))).toHaveLength(0);
});
