/** Proves recorded dictation: a recorded WAV microphone becomes inserted words through agy. */
import { test, expect } from './fixtures';
import {
  calls,
  create,
  evidence,
  recordedMicrophone,
  transcriptions,
  withoutSpeech,
} from './media-support';

test.use(recordedMicrophone('speech.wav'));

test('records speech, transcribes it and inserts the words at the caret', async ({
  page,
  engine,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await withoutSpeech(page);
  await create(page, engine, 'Dictation fixture');
  const box = page.getByRole('textbox', { name: 'Message' });
  await box.fill('Please  now');
  await box.evaluate((area: HTMLTextAreaElement) => area.setSelectionRange(7, 7));
  const mic = page.locator('button.mic');
  await mic.click();
  await expect(page.getByText(/^● 0:0\d · Esc cancels$/)).toBeVisible();
  await expect(mic).toHaveAttribute('aria-pressed', 'true');
  await expect(mic).toHaveAttribute('title', 'Recording: click to stop and transcribe');
  await page.waitForTimeout(1500);
  await evidence(engine, page, 'recording.png');
  await mic.click();
  await expect(box).toHaveValue('Please Dictated from the fixture. now', { timeout: 15_000 });
  await expect(box).toBeFocused();
  expect(await box.evaluate((area: HTMLTextAreaElement) => area.selectionStart)).toBe(33);
  await expect(mic).toHaveAttribute('aria-pressed', 'false');
  const asked = transcriptions(await calls(engine));
  expect(asked).toHaveLength(1);
  const argv = asked[0] ?? [];
  const prompt = argv.find((arg) => arg.startsWith('Listen to the audio file ')) ?? '';
  expect(prompt).toMatch(
    /\/state\/dictation\/[0-9a-f]{10}\.webm with your file viewer; do not run commands/,
  );
  expect(argv).not.toContain('--dangerously-skip-permissions');
  expect(argv).toContain('gemini-3.8-flash-high');
  expect(errors).toEqual([]);
});

test('Escape and the shortcut cancel or start a recording without transcribing', async ({
  page,
  engine,
}) => {
  await withoutSpeech(page);
  await create(page, engine, 'Dictation cancel fixture');
  const box = page.getByRole('textbox', { name: 'Message' });
  await box.fill('Keep this');
  await page.keyboard.press('ControlOrMeta+Shift+m');
  await expect(page.getByText(/Esc cancels/)).toBeVisible();
  await page.waitForTimeout(800);
  await page.keyboard.press('Escape');
  await expect(page.getByText(/Esc cancels/)).toBeHidden();
  await page.locator('button.mic').click();
  await expect(page.getByText(/Esc cancels/)).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.getByText(/Esc cancels/)).toBeHidden();
  await expect(box).toHaveValue('Keep this');
  expect(transcriptions(await calls(engine))).toHaveLength(0);
});
