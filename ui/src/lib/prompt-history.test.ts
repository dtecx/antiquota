/** Proves durable prompt filtering, recall bounds, draft preservation, and session isolation. */
import { expect, it } from 'vitest';
import type { EventRecord } from '../gen/EventRecord';
import { createPromptHistory } from './prompt-history';

const user = (text: string, kind = 'user'): EventRecord => ({ t: 'user', ts: 1, text, kind });

it('restores only explicit durable user and message prompts in their recorded order', () => {
  const history = createPromptHistory();
  history.select('a', [
    user('first'),
    user('generated goal', 'goal'),
    { t: 'assistant', ts: 2, turn: 1, step: 1, text: 'assistant response' },
    user('  second\nline  ', 'message'),
    user('generated continuation', 'continue'),
    user('steering event', 'steer'),
  ]);
  expect(history.step(1, 'draft')).toBe('  second\nline  ');
  expect(history.step(1, 'ignored recall text')).toBe('first');
  expect(history.step(1, 'ignored recall text')).toBe('first');
});

it('clamps the oldest prompt and returns the original draft after the newest prompt', () => {
  const history = createPromptHistory();
  history.select('a', [user('first'), user('second')]);
  expect(history.active()).toBe(false);
  expect(history.step(-1, 'unsent draft')).toBe('unsent draft');
  expect(history.active()).toBe(false);
  expect(history.step(1, 'unsent draft')).toBe('second');
  expect(history.active()).toBe(true);
  expect(history.step(1, 'second')).toBe('first');
  expect(history.step(1, 'first')).toBe('first');
  expect(history.step(-1, 'first')).toBe('second');
  expect(history.step(-1, 'second')).toBe('unsent draft');
  expect(history.active()).toBe(false);
  expect(history.step(1, 'new draft')).toBe('second');
  expect(history.step(-1, 'second')).toBe('new draft');
});

it('remembers explicit submitted and local input and resets recall after editing', () => {
  const history = createPromptHistory();
  history.select('a', [user('durable')]);
  expect(history.step(1, 'original draft')).toBe('durable');
  history.remember('!git status');
  expect(history.active()).toBe(false);
  expect(history.step(1, 'new draft')).toBe('!git status');
  expect(history.step(1, '!git status')).toBe('durable');
  history.edited();
  expect(history.active()).toBe(false);
  expect(history.step(1, 'edited recalled text')).toBe('!git status');
  expect(history.step(-1, '!git status')).toBe('edited recalled text');
});

it('replaces history and recall state when selecting another or reopening the same session', () => {
  const history = createPromptHistory();
  history.select('a', [user('A prompt')]);
  history.remember('A local input');
  expect(history.step(1, 'A draft')).toBe('A local input');
  history.select('b', [user('B prompt')]);
  expect(history.active()).toBe(false);
  expect(history.step(1, 'B draft')).toBe('B prompt');
  expect(history.step(1, 'B prompt')).toBe('B prompt');
  expect(history.step(-1, 'B prompt')).toBe('B draft');
  history.select('b', [user('new durable B prompt')]);
  expect(history.step(1, 'restored B draft')).toBe('new durable B prompt');
  expect(history.step(-1, 'new durable B prompt')).toBe('restored B draft');
});

it('does not enter recall when history is empty or no session is selected', () => {
  const history = createPromptHistory();
  expect(history.step(1, 'draft')).toBeUndefined();
  expect(history.step(-1, 'draft')).toBeUndefined();
  history.select('a', [user('goal', 'goal')]);
  expect(history.step(1, 'draft')).toBeUndefined();
  history.select('', [user('unselected prompt')]);
  expect(history.step(1, 'draft')).toBeUndefined();
  expect(history.active()).toBe(false);
});

it('adopts a prompt remembered before its durable event without duplicating it', () => {
  const history = createPromptHistory();
  history.select('same-session', [user('older')]);
  history.remember('newer');
  history.sync([user('older'), user('newer')]);
  history.sync([user('older'), user('newer')]);
  expect(history.step(1, 'draft')).toBe('newer');
  expect(history.step(1, 'newer')).toBe('older');
  expect(history.step(-1, 'older')).toBe('newer');
  expect(history.step(-1, 'newer')).toBe('draft');
});

it('claims a durable event that arrives before API acceptance without adding a duplicate', () => {
  const history = createPromptHistory();
  history.select('same-session', [user('older')]);
  history.sync([user('older'), user('newer')]);
  history.remember('newer');
  expect(history.step(1, 'draft')).toBe('newer');
  expect(history.step(1, 'newer')).toBe('older');
  expect(history.step(-1, 'older')).toBe('newer');
  expect(history.step(-1, 'newer')).toBe('draft');
});

it('retains repeated identical durable prompts and separate legitimate repeated sends', () => {
  const history = createPromptHistory();
  history.select('a', [user('same'), user('same')]);
  history.remember('same');
  history.sync([user('same'), user('same'), user('same')]);
  history.sync([user('same'), user('same'), user('same'), user('same')]);
  history.remember('same');
  for (let index = 0; index < 4; index++) expect(history.step(1, 'draft')).toBe('same');
  for (let index = 0; index < 3; index++) expect(history.step(-1, 'same')).toBe('same');
  expect(history.step(-1, 'same')).toBe('draft');
});

it('keeps local shell commands and notes alongside a same-session durable refresh', () => {
  const history = createPromptHistory();
  history.select('a', [user('older')]);
  history.remember(' !git status');
  history.remember('# project note');
  history.sync([user('older'), user('new durable prompt')]);
  history.sync([user('older'), user('new durable prompt')]);
  expect(history.step(1, 'unsent')).toBe('new durable prompt');
  expect(history.step(1, 'new durable prompt')).toBe('# project note');
  expect(history.step(1, '# project note')).toBe(' !git status');
  expect(history.step(1, ' !git status')).toBe('older');
  for (let index = 0; index < 3; index++) history.step(-1, 'ignored');
  expect(history.step(-1, 'ignored')).toBe('unsent');
});

it('preserves the recalled record and original draft when durable prompts append', () => {
  const history = createPromptHistory();
  history.select('a', [user('oldest'), user('middle')]);
  expect(history.step(1, 'original unsent draft')).toBe('middle');
  history.sync([user('oldest'), user('middle'), user('newest')]);
  expect(history.active()).toBe(true);
  expect(history.step(1, 'middle')).toBe('oldest');
  expect(history.step(-1, 'oldest')).toBe('middle');
  expect(history.step(-1, 'middle')).toBe('newest');
  expect(history.step(-1, 'newest')).toBe('original unsent draft');
});

it('preserves recall while adopting a pending prompt and appending another', () => {
  const history = createPromptHistory();
  history.select('a', [user('older')]);
  history.remember('pending');
  expect(history.step(1, 'unsent')).toBe('pending');
  history.sync([user('older'), user('pending'), user('external')]);
  expect(history.step(-1, 'pending')).toBe('external');
  expect(history.step(-1, 'external')).toBe('unsent');
});

it('resets to a changed or shortened durable prefix without retaining obsolete local records', () => {
  const history = createPromptHistory();
  history.select('a', [user('oldest'), user('removed')]);
  history.remember('# old local note');
  history.step(1, 'unsent');
  history.sync([user('replacement')]);
  expect(history.active()).toBe(false);
  expect(history.step(1, 'new draft')).toBe('replacement');
  expect(history.step(-1, 'replacement')).toBe('new draft');
  history.sync([]);
  expect(history.step(1, 'new draft')).toBeUndefined();
  history.select('', []);
  history.sync([user('unselected')]);
  expect(history.step(1, 'new draft')).toBeUndefined();
});

it('ignores streaming fragments and generated prompts during synchronization', () => {
  const history = createPromptHistory();
  history.select('a', [user('durable')]);
  history.remember('# note');
  history.sync([
    user('durable'),
    user('goal instruction', 'goal'),
    user('steering', 'steer'),
    { t: 'delta', ts: 2, turn: 1, step: 1, text: 'streaming' },
  ]);
  expect(history.step(1, 'draft')).toBe('# note');
  expect(history.step(1, '# note')).toBe('durable');
});
