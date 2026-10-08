/** Proves composer completion ordering, insertion and delayed dismissal. */
import { get } from 'svelte/store';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createAutocomplete } from './autocomplete';
import { commands } from './commands';

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((accept, decline) => {
    resolve = accept;
    reject = decline;
  });
  return { promise, resolve, reject };
}

afterEach(() => vi.useRealTimers());

describe('autocomplete', () => {
  it('shares all 31 commands and filters slash prefixes without case sensitivity', async () => {
    const mentionFiles = vi.fn(async () => [] as string[]);
    const menu = createAutocomplete({ mentionFiles });
    await menu.query('session', '/', 1);
    expect(get(menu.state).items).toEqual(commands.map(([label, detail]) => ({ label, detail })));
    expect(get(menu.state).items).toHaveLength(31);
    await menu.query('session', '/M', 2);
    expect(get(menu.state).items.map((item) => item.label)).toEqual([
      '/memory',
      '/model',
      '/mode',
      '/mcp',
    ]);
    await menu.query('session', '/BG', 3);
    expect(menu.apply()).toEqual({ text: '/bg ', caret: 4 });
    expect(get(menu.state)).toEqual({ items: [], index: 0 });
    expect(mentionFiles).not.toHaveBeenCalled();
  });

  it('closes slash suggestions once arguments or other whitespace begin', async () => {
    const menu = createAutocomplete({ mentionFiles: vi.fn(async () => []) });
    for (const text of ['/model gemini', '/model\n', ' /model', '/unknown']) {
      await menu.query('session', '/', 1);
      await menu.query('session', text, text.length);
      expect(get(menu.state)).toEqual({ items: [], index: 0 });
      expect(menu.apply()).toBeUndefined();
    }
  });

  it('queries a mention before the caret and preserves its @, prefix and suffix when applied', async () => {
    const mentionFiles = vi.fn(async () => ['src/app.ts', 'src/lib.ts']);
    const menu = createAutocomplete({ mentionFiles });
    const text = 'Read @src/ then explain it';
    const caret = text.indexOf(' then');
    await menu.query('left-session', text, caret);
    expect(mentionFiles).toHaveBeenCalledWith('left-session', 'src/');
    expect(get(menu.state).items).toEqual([
      { label: 'src/app.ts', detail: 'file' },
      { label: 'src/lib.ts', detail: 'file' },
    ]);
    menu.step(1);
    expect(menu.apply()).toEqual({ text: 'Read @src/lib.ts  then explain it', caret: 17 });
    expect(menu.apply()).toBeUndefined();
    await menu.query('left-session', '@', 1);
    expect(mentionFiles).toHaveBeenLastCalledWith('left-session', '');
    expect(menu.apply()).toEqual({ text: '@src/app.ts ', caret: 12 });
  });

  it('accepts path characters and whitespace boundaries without completing email addresses', async () => {
    const mentionFiles = vi.fn(async () => [] as string[]);
    const menu = createAutocomplete({ mentionFiles });
    await menu.query('session', 'Read\n@~/../src-file.ts', 22);
    expect(mentionFiles).toHaveBeenCalledWith('session', '~/../src-file.ts');
    for (const text of ['a@src', 'Read @src?', 'Read @src rest']) {
      await menu.query('session', text, text.length);
      expect(get(menu.state).items).toEqual([]);
    }
    await menu.query('', '@src', 4);
    expect(mentionFiles).toHaveBeenCalledTimes(1);
  });

  it('ignores old query results and errors across text and session changes', async () => {
    const old = deferred<string[]>();
    const failed = deferred<string[]>();
    const newest = deferred<string[]>();
    const mentionFiles = vi
      .fn()
      .mockReturnValueOnce(old.promise)
      .mockReturnValueOnce(failed.promise)
      .mockReturnValueOnce(newest.promise);
    const menu = createAutocomplete({ mentionFiles });
    const first = menu.query('left', '@s', 2);
    const second = menu.query('left', '@src', 4);
    const third = menu.query('right', '@src', 4);
    newest.resolve(['right.ts']);
    await third;
    old.resolve(['old.ts']);
    failed.reject(new Error('old request failed'));
    await Promise.all([first, second]);
    expect(get(menu.state).items).toEqual([{ label: 'right.ts', detail: 'file' }]);
    expect(menu.apply()).toEqual({ text: '@right.ts ', caret: 10 });
  });

  it('invalidates pending lookups when closed or cleared, including a session with no selection', async () => {
    for (const dismiss of ['close', 'text', 'session']) {
      const pending = deferred<string[]>();
      const menu = createAutocomplete({ mentionFiles: () => pending.promise });
      const query = menu.query('session', '@src', 4);
      if (dismiss === 'close') menu.close();
      else
        await menu.query(
          dismiss === 'session' ? '' : 'session',
          dismiss === 'text' ? '' : '@src',
          4,
        );
      pending.resolve(['late.ts']);
      await query;
      expect(get(menu.state)).toEqual({ items: [], index: 0 });
      expect(menu.apply()).toBeUndefined();
    }
  });

  it('wraps selection in both directions and safely handles no matches or a failed lookup', async () => {
    const menu = createAutocomplete({
      mentionFiles: async () => {
        throw new Error('offline');
      },
    });
    menu.step(-1);
    expect(get(menu.state).index).toBe(0);
    await menu.query('session', '/M', 2);
    menu.step(-1);
    expect(get(menu.state).index).toBe(3);
    menu.step(1);
    expect(get(menu.state).index).toBe(0);
    menu.step(6);
    expect(get(menu.state).index).toBe(2);
    await menu.query('session', '/model', 6);
    expect(get(menu.state).index).toBe(0);
    await menu.query('session', '@src', 4);
    expect(get(menu.state)).toEqual({ items: [], index: 0 });
    expect(menu.apply()).toBeUndefined();
  });

  it('dismisses after 100 ms of blur and cancels an old dismissal on a new query or apply', async () => {
    vi.useFakeTimers();
    const menu = createAutocomplete({ mentionFiles: vi.fn(async () => []) });
    await menu.query('session', '/help', 5);
    menu.blur();
    vi.advanceTimersByTime(99);
    expect(get(menu.state).items).toHaveLength(1);
    vi.advanceTimersByTime(1);
    expect(get(menu.state).items).toEqual([]);
    await menu.query('session', '/help', 5);
    menu.blur();
    await menu.query('session', '/model', 6);
    vi.advanceTimersByTime(100);
    expect(get(menu.state).items).toEqual([{ label: '/model', detail: commands[16]![1] }]);
    menu.blur();
    expect(menu.apply()).toEqual({ text: '/model ', caret: 7 });
    expect(vi.getTimerCount()).toBe(0);
  });

  it('disposes timers and pending lookups and cannot reopen after disposal', async () => {
    vi.useFakeTimers();
    const pending = deferred<string[]>();
    const mentionFiles = vi.fn(() => pending.promise);
    const menu = createAutocomplete({ mentionFiles });
    const query = menu.query('session', '@src', 4);
    menu.blur();
    menu.dispose();
    expect(vi.getTimerCount()).toBe(0);
    pending.resolve(['late.ts']);
    await query;
    await menu.query('session', '@again', 6);
    menu.blur();
    expect(mentionFiles).toHaveBeenCalledOnce();
    expect(get(menu.state)).toEqual({ items: [], index: 0 });
    expect(vi.getTimerCount()).toBe(0);
  });
});
