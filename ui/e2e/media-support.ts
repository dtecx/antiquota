/** Shared steps for the attachment and dictation browser checks. */
import type { Page } from '@playwright/test';
import { mkdir, readFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { expect } from './fixtures';
import type { FixtureEngine } from '../scripts/engine.ts';

/** Chromium options that replace the microphone with a recorded WAV file. */
export function recordedMicrophone(file: 'speech.wav' | 'silence.wav') {
  return {
    permissions: ['microphone'],
    launchOptions: {
      args: [
        '--use-fake-ui-for-media-stream',
        '--use-fake-device-for-media-stream',
        `--use-file-for-fake-audio-capture=${resolve(import.meta.dirname, 'audio', file)}`,
        '--autoplay-policy=no-user-gesture-required',
      ],
    },
  };
}

/** Removes speech recognition so dictation records and transcribes. */
export async function withoutSpeech(page: Page) {
  await page.addInitScript(() => {
    for (const name of ['SpeechRecognition', 'webkitSpeechRecognition'])
      Object.defineProperty(window, name, { value: undefined, configurable: true });
  });
}

export async function create(page: Page, engine: FixtureEngine, title: string) {
  await page.goto(engine.url);
  await expect(page.getByText('Engine connected')).toBeVisible();
  await page.getByRole('button', { name: 'New session' }).first().click();
  await page.getByRole('textbox', { name: 'Session name' }).fill(title);
  await page.getByRole('textbox', { name: 'Project folder' }).fill(engine.sandbox.project);
  await page.getByRole('button', { name: 'Create session' }).click();
  await expect(page.getByRole('heading', { name: title, exact: true })).toBeVisible();
}

/** Every fake-agy command line recorded so far. */
export async function calls(engine: FixtureEngine): Promise<string[][]> {
  const text = await readFile(join(engine.sandbox.root, 'fake-calls.jsonl'), 'utf8').catch(
    () => '',
  );
  return text
    .split('\n')
    .filter(Boolean)
    .map((line) => (JSON.parse(line) as { argv: string[] }).argv);
}

export function transcriptions(all: string[][]) {
  return all.filter((argv) => argv.some((arg) => arg.startsWith('Listen to the audio file ')));
}

export async function evidence(engine: FixtureEngine, page: Page, name: string) {
  const directory = join(engine.sandbox.repository, '.handoff', 'evidence', 'ui-slice8');
  await mkdir(directory, { recursive: true });
  await page.screenshot({ path: join(directory, name), fullPage: true });
}
