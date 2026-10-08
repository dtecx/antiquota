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
      ['antiquota.detail', 'compact'],
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
      detail: 'compact',
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
      detail: 'detailed',
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
