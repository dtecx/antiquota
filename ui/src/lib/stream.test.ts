/** Proves one stream reconnects, resyncs unknown frames, and closes on teardown. */
import { get } from 'svelte/store';
import { expect, it, vi } from 'vitest';
import { createStream } from './stream';
it('uses one connection and requests a fresh snapshot on reconnect or invalid frames', () => {
  const source = {
    onopen: null as EventSource['onopen'],
    onerror: null as EventSource['onerror'],
    onmessage: null as EventSource['onmessage'],
    close: vi.fn(),
  };
  const factory = vi.fn(() => source as unknown as EventSource);
  const event = vi.fn();
  const resync = vi.fn(async () => {});
  const stream = createStream('/api/events?token=synthetic', event, resync, factory);
  stream.open();
  expect(get(stream.connection)).toBe('connecting');
  source.onopen?.call(source as unknown as EventSource, new Event('open'));
  expect(get(stream.connection)).toBe('live');
  expect(resync).toHaveBeenCalledTimes(1);
  source.onmessage?.call(
    source as unknown as EventSource,
    new MessageEvent('message', { data: '{"t":"deleted","sid":"a","ts":1}' }),
  );
  expect(event).toHaveBeenCalledWith({ t: 'deleted', sid: 'a', ts: 1 });
  source.onerror?.call(source as unknown as EventSource, new Event('error'));
  expect(get(stream.connection)).toBe('reconnecting');
  source.onmessage?.call(
    source as unknown as EventSource,
    new MessageEvent('message', { data: 'not JSON' }),
  );
  expect(resync).toHaveBeenCalledTimes(2);
  stream.open();
  expect(source.close).toHaveBeenCalledTimes(1);
  stream.close();
  expect(source.close).toHaveBeenCalledTimes(2);
});
