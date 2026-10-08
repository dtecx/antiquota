/** Proves mount-scoped timing and teardown without waiting on wall time. */
import { afterEach, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import { createToast, minuteClock, watchWorkbench } from './workbench-lifecycle';
import { createWorkbench } from './workbench';
import { createApi } from './api';
import { fixtureState } from './fixtures.test-support';
afterEach(() => vi.useRealTimers());
it('replaces the previous toast timer and clears only after 2.6 seconds from the last message', () => {
  vi.useFakeTimers();
  const toast = createToast();
  toast.show('first');
  vi.advanceTimersByTime(2000);
  toast.show('second');
  vi.advanceTimersByTime(600);
  expect(get(toast.state)).toBe('second');
  vi.advanceTimersByTime(1999);
  expect(get(toast.state)).toBe('second');
  vi.advanceTimersByTime(1);
  expect(get(toast.state)).toBe('');
  toast.show('closed');
  toast.dispose();
  expect(vi.getTimerCount()).toBe(0);
});
it('retries a missing initial model list at four seconds and cancels on unmount', async () => {
  vi.useFakeTimers();
  const api = { ...createApi('test'), state: vi.fn(async () => fixtureState([])) };
  const workbench = createWorkbench(api),
    toast = createToast();
  const dispose = watchWorkbench(workbench, toast);
  const empty = { ...fixtureState([]), models: [] };
  workbench.acceptSnapshot(empty);
  vi.advanceTimersByTime(3999);
  expect(api.state).not.toHaveBeenCalled();
  await vi.advanceTimersByTimeAsync(1);
  expect(api.state).toHaveBeenCalledOnce();
  expect(get(workbench.state).snapshot?.models).toHaveLength(1);
  dispose();
  toast.dispose();
  const other = createWorkbench(api),
    cancel = watchWorkbench(other, toast);
  other.acceptSnapshot(empty);
  cancel();
  await vi.advanceTimersByTimeAsync(4000);
  expect(api.state).toHaveBeenCalledOnce();
});
it('does not retry populated models and surfaces new feedback and errors without clearing durable errors', () => {
  vi.useFakeTimers();
  const workbench = createWorkbench(createApi('test')),
    toast = createToast();
  const dispose = watchWorkbench(workbench, toast);
  workbench.acceptSnapshot(fixtureState([]));
  expect(vi.getTimerCount()).toBe(0);
  workbench.change({ feedback: 'Copied' });
  expect(get(toast.state)).toBe('Copied');
  workbench.change({ error: 'offline' });
  expect(get(toast.state)).toBe('offline');
  vi.advanceTimersByTime(2600);
  expect(get(toast.state)).toBe('');
  expect(get(workbench.state).error).toBe('offline');
  dispose();
  toast.dispose();
});
it('refreshes relative-time input every minute and releases its only timer after last subscriber', () => {
  vi.useFakeTimers();
  vi.setSystemTime(120000);
  const seen: number[] = [],
    unsubscribe = minuteClock.subscribe((now) => seen.push(now));
  expect(seen.at(-1)).toBe(120);
  vi.advanceTimersByTime(59999);
  expect(seen).toHaveLength(1);
  vi.advanceTimersByTime(1);
  expect(seen.at(-1)).toBe(180);
  unsubscribe();
  expect(vi.getTimerCount()).toBe(0);
});
