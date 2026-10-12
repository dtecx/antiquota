/** Proves device preferences survive old, malformed and blocked browser storage. */
import { get } from 'svelte/store';
import { describe, expect, it, vi } from 'vitest';
import { createPreferences } from './preferences';
describe('device preferences', () => {
  it('loads compatible keys and persists individual notification choices', () => {
    const values = new Map([
      [
        'antiquota.prefs',
        JSON.stringify({ view: 'steps', stamps: false, zoom: 4, notify: { job: false } }),
      ],
      ['antiquota.detail', 'detailed'],
    ]);
    const storage = {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => {
        values.set(key, value);
      },
    };
    const prefs = createPreferences(storage);
    expect(get(prefs.state)).toMatchObject({
      view: 'steps',
      stamps: false,
      zoom: 1.6,
      detail: 'verbose',
      notify: { job: false, turn: true },
    });
    prefs.notify('approval', false);
    prefs.change({ focused: true });
    expect(get(createPreferences(storage).state)).toMatchObject({
      focused: true,
      notify: { job: false, approval: false, turn: true },
    });
  });
  it('uses defaults for corrupt storage and still changes when writes are blocked', () => {
    const storage = {
      getItem: vi.fn(() => '{broken'),
      setItem: vi.fn(() => {
        throw new Error('blocked');
      }),
    };
    const prefs = createPreferences(storage);
    expect(get(prefs.state)).toMatchObject({
      zoom: 1,
      view: 'grouped',
      stamps: true,
      detail: 'compact',
    });
    expect(() => prefs.change({ stamps: false })).not.toThrow();
    expect(get(prefs.state).stamps).toBe(false);
    expect(
      get(
        createPreferences({
          getItem: () => {
            throw new Error('blocked');
          },
          setItem: () => {},
        }).state,
      ).zoom,
    ).toBe(1);
  });
});
it('restores per-device collapse and split choices while rejecting malformed values', () => {
  const valid = createPreferences({
    getItem: (key) =>
      key === 'antiquota.prefs'
        ? JSON.stringify({ collapsed: ['/Desktop/test/a', 4], split: 'session-b' })
        : null,
    setItem: () => {},
  });
  expect(get(valid.state)).toMatchObject({ collapsed: ['/Desktop/test/a'], split: 'session-b' });
  const invalid = createPreferences({
    getItem: () => JSON.stringify({ collapsed: 'wrong', split: 42 }),
    setItem: () => {},
  });
  expect(get(invalid.state)).toMatchObject({ collapsed: [], split: null });
});
it('normalizes invalid zoom changes and clamps extreme values at the shared boundary', () => {
  const prefs = createPreferences();
  for (const zoom of [NaN, Infinity, -Infinity]) {
    prefs.change({ zoom });
    expect(get(prefs.state).zoom).toBe(1);
  }
  prefs.change({ zoom: 0 });
  expect(get(prefs.state).zoom).toBe(0.7);
  prefs.change({ zoom: 5 });
  expect(get(prefs.state).zoom).toBe(1.6);
});

it('maps stored transcript modes, including the old detailed name, and defaults to compact', () => {
  const load = (value: string | null) =>
    get(
      createPreferences({
        getItem: (key) => (key === 'antiquota.detail' ? value : null),
        setItem: () => {},
      }).state,
    ).detail;
  expect([null, 'compact', 'detailed', 'verbose', 'full', 'odd'].map(load)).toEqual([
    'compact',
    'compact',
    'verbose',
    'verbose',
    'full',
    'compact',
  ]);
});
