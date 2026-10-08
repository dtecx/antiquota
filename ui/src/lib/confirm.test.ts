/** Proves explicit positive consent, default labels and superseded questions. */
import { expect, it } from 'vitest';
import { get } from 'svelte/store';
import { ask, question } from './confirm';
it('offers Continue by default and resolves only its explicit answer', async () => {
  const pending = ask('Continue work?', 'Proceed with this action.');
  expect(get(question)).toMatchObject({ ok: 'Continue', danger: false });
  get(question)?.answer(true);
  expect(await pending).toBe(true);
  expect(get(question)).toBeNull();
  const cancel = ask('Cancel work?', 'Proceed?', { danger: true, ok: 'Discard' });
  expect(get(question)).toMatchObject({ ok: 'Discard', danger: true });
  get(question)?.answer(false);
  expect(await cancel).toBe(false);
});
it('cancels a superseded question rather than leaving its owner waiting', async () => {
  const old = ask('Old action?', 'Old');
  const current = ask('New action?', 'New');
  expect(await old).toBe(false);
  expect(get(question)?.title).toBe('New action?');
  get(question)?.answer(false);
  expect(await current).toBe(false);
});
