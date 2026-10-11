/** Owns event projection, streamed step replacement, and transcript grouping. */
import type { EventRecord } from '../gen/EventRecord';
import type { StreamEvent } from '../gen/StreamEvent';
import type { ToolEvent } from './tools';
import { isStreamEvent } from './validate';

function finalizeTurn(events: EventRecord[], turn: number): EventRecord[] {
  return events.map((event) =>
    event.t === 'delta' && event.turn === turn ? { ...event, t: 'assistant' } : event,
  );
}

export function reduceTranscript(events: EventRecord[], event: EventRecord): EventRecord[] {
  if (['meta', 'accounts', 'deleted'].includes(event.t)) return events;
  if (event.t === 'turn_end') events = finalizeTurn(events, event.turn);
  if (event.t === 'approval') {
    // An answer replaces its pending card in place.
    const index = events.findIndex((item) => item.t === 'approval' && item.id === event.id);
    if (index >= 0) return events.map((item, position) => (position === index ? event : item));
    return [...events, event];
  }
  if (
    event.t === 'delta' &&
    events.some(
      (item) =>
        (item.t === 'turn_end' && item.turn === event.turn) ||
        (item.t === 'assistant' && item.turn === event.turn && item.step === event.step),
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
  // A fetched history and the live stream serialize the same record with different key orders.
  if (events.some((item) => item.ts === event.ts && canonical(item) === canonical(event)))
    return events;
  return [...events, event];
}

function canonical(value: unknown): string {
  return JSON.stringify(value, (_key, item: unknown) =>
    item && typeof item === 'object' && !Array.isArray(item)
      ? Object.fromEntries(Object.entries(item).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)))
      : item,
  );
}

export function projectHistory(events: EventRecord[]): EventRecord[] {
  // Durable history is authoritative. Equal timestamps and text can be separate messages.
  return events.reduce<EventRecord[]>((projection, event) => {
    if (['meta', 'accounts', 'deleted'].includes(event.t)) return projection;
    if (event.t === 'turn_end') return [...finalizeTurn(projection, event.turn), event];
    if (['tool', 'assistant', 'thinking', 'delta', 'approval'].includes(event.t))
      return reduceTranscript(projection, event);
    return [...projection, event];
  }, []);
}

export type TranscriptItem =
  { key: string; events: ToolEvent[] } | { key: string; event: EventRecord };

/** Groups only adjacent tools, preserving replies, thinking and boundaries in arrival order. */
export function transcriptItems(events: EventRecord[]): TranscriptItem[] {
  const items: TranscriptItem[] = [];
  events.forEach((event, index) => {
    if (['meta', 'accounts', 'deleted'].includes(event.t)) return;
    if (event.t === 'tool') {
      const previous = items.at(-1);
      if (previous && 'events' in previous) previous.events.push(event);
      else items.push({ key: `tools-${event.turn}-${event.step}`, events: [event] });
      return;
    }
    const key =
      event.t === 'delta' || event.t === 'assistant'
        ? `reply-${event.turn}-${event.step}`
        : event.t === 'thinking'
          ? `thinking-${event.turn}-${event.step}`
          : event.t === 'approval'
            ? `approval-${event.id}`
            : `${event.t}-${event.ts}-${index}`;
    items.push({ key, event });
  });
  return items;
}

/** Describes prompt origins without showing the goal's standing instructions in the bubble. */
export function userPresentation(user: Extract<EventRecord, { t: 'user' }>): {
  label: string;
  text: string;
  tooltip?: string;
} {
  const labels: Record<string, string> = {
    user: 'YOU',
    message: 'YOU',
    goal: 'GOAL',
    nudge: 'GOAL PUSH',
    job: 'JOB',
    loop: 'LOOP',
  };
  const label = Object.hasOwn(labels, user.kind) ? labels[user.kind]! : 'YOU';
  return user.kind === 'goal'
    ? {
        label,
        text: (user.text.split(/\r?\n[ \t]*\r?\n/, 1)[0] ?? '').replace(/^Objective:\s*/, ''),
        tooltip: user.text,
      }
    : { label, text: user.text };
}

export function nearBottom(distance: number): boolean {
  return distance <= 120;
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

/** Finds prompt/reply messages by literal source text, never tool or thinking detail. */
export function findTranscript(groups: TranscriptGroup[], query: string): string[] {
  const needle = query.trim().toLowerCase();
  if (!needle) return [];
  return groups.flatMap((group) => [
    ...(group.user && userPresentation(group.user).text.toLowerCase().includes(needle)
      ? [`${group.key}:user`]
      : []),
    ...transcriptItems(group.events).flatMap((item) =>
      !('events' in item) &&
      (item.event.t === 'assistant' || item.event.t === 'delta') &&
      item.event.text.toLowerCase().includes(needle)
        ? [`${group.key}:${item.key}`]
        : [],
    ),
  ]);
}

export function parseStreamEvent(text: string): StreamEvent | null {
  const value: unknown = JSON.parse(text);
  return isStreamEvent(value) ? value : null;
}
