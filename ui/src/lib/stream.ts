/** Owns the one SSE connection and authoritative resync requests on reconnect. */
import { writable } from 'svelte/store';
import type { StreamEvent } from '../gen/StreamEvent';
import { parseStreamEvent } from './transcript';

export type Connection = 'connecting' | 'live' | 'reconnecting';
export function createStream(
  url: string,
  onEvent: (event: StreamEvent) => void,
  resync: () => Promise<void>,
  factory: (url: string) => EventSource = (address) => new EventSource(address),
) {
  const connection = writable<Connection>('connecting');
  let source: EventSource | null = null;
  function open() {
    source?.close();
    connection.set('connecting');
    source = factory(url);
    source.onopen = () => {
      connection.set('live');
      void resync();
    };
    source.onerror = () => {
      connection.set('reconnecting');
    };
    source.onmessage = (message) => {
      try {
        const event = parseStreamEvent(String(message.data));
        if (event) onEvent(event);
        else void resync();
      } catch {
        void resync();
      }
    };
  }
  return {
    connection,
    open,
    close: () => {
      source?.close();
      source = null;
    },
  };
}
