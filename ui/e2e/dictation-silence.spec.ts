/** Proves the silence guard with a recorded silent WAV microphone: nothing is uploaded. */
import { test, expect } from './fixtures';
import {
  calls,
  create,
  evidence,
  recordedMicrophone,
  transcriptions,
  withoutSpeech,
} from './media-support';

test.use(recordedMicrophone('silence.wav'));

test('silent and too-short recordings are refused before upload', async ({ page, engine }) => {
  await withoutSpeech(page);
  await create(page, engine, 'Silence fixture');
  const mic = page.locator('button.mic');
  await mic.click();
  await expect(page.getByText(/Esc cancels/)).toBeVisible();
  await page.waitForTimeout(1500);
  await mic.click();
  await expect(
    page.getByRole('status').getByText('Nothing heard: is the right microphone selected?'),
  ).toBeVisible();
  await evidence(engine, page, 'silence.png');
  await mic.click();
  await expect(page.getByText(/Esc cancels/)).toBeVisible();
  await mic.click();
  await expect(
    page.getByRole('status').getByText('Too short: hold on a little longer'),
  ).toBeVisible();
  await expect(page.getByRole('textbox', { name: 'Message' })).toHaveValue('');
  expect(transcriptions(await calls(engine))).toHaveLength(0);
});
