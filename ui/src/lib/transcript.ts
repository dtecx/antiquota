/** Owns event projection, streamed step replacement, and transcript grouping. */
import type { EventRecord } from '../gen/EventRecord';
import type { StreamEvent } from '../gen/StreamEvent';
import { isStreamEvent } from './validate';

export function reduceTranscript(events: EventRecord[], event: EventRecord): EventRecord[] {
  if (['meta', 'accounts', 'deleted'].includes(event.t)) return events;
  if (event.t === 'approval') {
    // An answer replaces its pending card in place.
    const index = events.findIndex((item) => item.t === 'approval' && item.id === event.id);
    if (index >= 0) return events.map((item, position) => (position === index ? event : item));
    return [...events, event];
  }
  if (
    event.t === 'delta' &&
    events.some(
      (item) => item.t === 'assistant' && item.turn === event.turn && item.step === event.step,
    )
  )
    return events;
  if (
    event.t === 'delta' ||
    event.t === 'assistant' ||
    event.t === 'tool' ||
    event.t === 'thinking'
  ) {
    const index = events.findIndex(
      (item) =>
        'turn' in item &&
        'step' in item &&
        item.turn === event.turn &&
        item.step === event.step &&
        (item.t === event.t || (event.t === 'assistant' && item.t === 'delta')),
    );
    if (index >= 0) {
      const previous = events[index];
      if (
        event.t === 'tool' &&
        previous?.t === 'tool' &&
        ['done', 'error'].includes(previous.state.toLowerCase()) &&
        ['active', 'running'].includes(event.state.toLowerCase())
      )
        return events;
      const updated =
        event.t === 'delta' && previous?.t === 'delta'
          ? { ...event, text: previous.text + event.text }
          : event;
      return events.flatMap((item, position) => {
        if (position === index) return [updated];
        if (
          event.t === 'assistant' &&
          (item.t === 'delta' || item.t === 'assistant') &&
          item.turn === event.turn &&
          item.step === event.step
        )
          return [];
        return [item];
      });
    }
  }
  if (events.some((item) => item.ts === event.ts && JSON.stringify(item) === JSON.stringify(event)))
    return events;
  return [...events, event];
}

export function projectHistory(events: EventRecord[]): EventRecord[] {
  // Durable history is authoritative. Equal timestamps and text can be separate messages.
  return events.reduce<EventRecord[]>((projection, event) => {
    if (['meta', 'accounts', 'deleted'].includes(event.t)) return projection;
    if (['tool', 'assistant', 'thinking', 'delta', 'approval'].includes(event.t))
      return reduceTranscript(projection, event);
    return [...projection, event];
  }, []);
}

export interface TranscriptGroup {
  key: string;
  user: Extract<EventRecord, { t: 'user' }> | null;
  events: EventRecord[];
}

export function groupTranscript(events: EventRecord[]): TranscriptGroup[] {
  const groups: TranscriptGroup[] = [];
  for (const event of events) {
    if (['meta', 'accounts', 'deleted'].includes(event.t)) continue;
    if (event.t === 'user') {
      groups.push({ key: `user-${event.ts}-${groups.length}`, user: event, events: [] });
    } else {
      if (!groups.length) groups.push({ key: 'history', user: null, events: [] });
      groups.at(-1)?.events.push(event);
    }
  }
  return groups;
}

export function parseStreamEvent(text: string): StreamEvent | null {
  const value: unknown = JSON.parse(text);
  return isStreamEvent(value) ? value : null;
}
