/** Proves folder recency, exact debounce, stale reads and teardown. */
import { afterEach, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import { createFolders, recentFolders } from './folders';
import { fixtureSession } from './fixtures.test-support';
import type { FolderResponse } from '../gen/FolderResponse';
afterEach(() => vi.useRealTimers());
it('offers four unique recent folders sorted by latest session update', () => {
  const rows = [1, 7, 2, 6, 3, 5, 4].map((updated) => ({
    ...fixtureSession(String(updated)),
    updated,
    cwd: `/test/${updated % 5}`,
  }));
  expect(recentFolders(rows)).toEqual(['/test/2', '/test/1', '/test/0', '/test/4']);
  expect(rows[0]?.updated).toBe(1);
});
it('debounces typed paths for 200ms and invalidates previous reads immediately', async () => {
  vi.useFakeTimers();
  const completions = new Map<string, (r: FolderResponse) => void>();
  const browse = vi.fn(
    (path: string) => new Promise<FolderResponse>((resolve) => completions.set(path, resolve)),
  );
  const folders = createFolders({ browse });
  folders.browse('');
  folders.browse('/test/a', true);
  completions.get('')?.({ path: '/', parent: null, dirs: ['stale'] });
  await Promise.resolve();
  expect(get(folders.state).listing).toBeNull();
  vi.advanceTimersByTime(199);
  expect(browse).toHaveBeenCalledTimes(1);
  folders.browse('/test/b', true);
  vi.advanceTimersByTime(199);
  expect(browse).toHaveBeenCalledTimes(1);
  vi.advanceTimersByTime(1);
  expect(browse).toHaveBeenLastCalledWith('/test/b');
  completions.get('/test/b')?.({ path: '/test/b', parent: '/test', dirs: ['child'] });
  await Promise.resolve();
  expect(get(folders.state)).toMatchObject({ recent: false, listing: { dirs: ['child'] } });
  folders.dispose();
});
it('shows current browsing errors and ignores results and timers after disposal', async () => {
  vi.useFakeTimers();
  const browse = vi.fn(async () => {
    throw new Error('Folder missing');
  });
  const folders = createFolders({ browse });
  folders.browse('/missing');
  await Promise.resolve();
  expect(get(folders.state).error).toBe('Folder missing');
  folders.browse('/later', true);
  folders.dispose();
  vi.advanceTimersByTime(200);
  expect(browse).toHaveBeenCalledOnce();
});
