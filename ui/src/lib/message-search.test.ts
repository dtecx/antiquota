/** Proves debounce, stale responses and errors cannot replace the current search. */
import { get } from 'svelte/store';
import { afterEach, expect, it, vi } from 'vitest';
import { createMessageSearch } from './message-search';
import type { SearchHit } from '../gen/SearchHit';
afterEach(() => vi.useRealTimers());
it('debounces token changes and discards results after clearing or replacing the query', async () => {
  vi.useFakeTimers();
  const pending = new Map<string, (hits: SearchHit[]) => void>();
  const search = vi.fn(
    (q: string) => new Promise<SearchHit[]>((resolve) => pending.set(q, resolve)),
  );
  const controller = createMessageSearch({ search });
  controller.query('first');
  controller.query('  SECOND  ');
  await vi.advanceTimersByTimeAsync(219);
  expect(search).not.toHaveBeenCalled();
  await vi.advanceTimersByTimeAsync(1);
  expect(search).toHaveBeenCalledWith('second');
  controller.query('third');
  await vi.advanceTimersByTimeAsync(220);
  const hit: SearchHit = {
    id: 'three',
    title: 'Third',
    cwd: '/Desktop/test',
    ts: 1,
    who: 'user',
    snippet: 'Third match',
  };
  pending.get('third')?.([hit]);
  await Promise.resolve();
  pending.get('second')?.([{ ...hit, id: 'old' }]);
  await Promise.resolve();
  expect(get(controller.state).hits).toEqual([hit]);
  controller.query('fourth');
  await vi.advanceTimersByTimeAsync(220);
  controller.query('x');
  pending.get('fourth')?.([hit]);
  await Promise.resolve();
  expect(get(controller.state)).toEqual({ hits: [], loading: false, error: '' });
  controller.query('last');
  controller.dispose();
  await vi.advanceTimersByTimeAsync(220);
  expect(search).toHaveBeenCalledTimes(3);
});
it('reports a failed current search and clears that error on the next query', async () => {
  vi.useFakeTimers();
  const search = vi.fn().mockRejectedValue(new Error('offline'));
  const controller = createMessageSearch({ search });
  controller.query('match');
  await vi.advanceTimersByTimeAsync(220);
  expect(get(controller.state)).toEqual({ hits: [], loading: false, error: 'offline' });
  controller.query('');
  expect(get(controller.state).error).toBe('');
  controller.dispose();
});
