/** Owns presentation of captured routing records, without account-selection policy. */
import type { RoutingEntry } from '../gen/RoutingEntry';
import type { Session } from '../gen/Session';
import { shortAccount, clockTime } from './account-format';
export function routingLine(entry: RoutingEntry, sessions: Session[]) {
  const slot = shortAccount(entry.slot);
  const title =
    sessions.find((session) => session.id === entry.session)?.title || entry.session || '';
  switch (entry.event) {
    case 'pick':
      return {
        label: `→ ${slot}`,
        text: `serves ${title}${entry.previous ? ` (was ${shortAccount(entry.previous)})` : ''}`,
      };
    case 'wall':
      return {
        label: `▮ ${slot}`,
        text: `${entry.why ?? ''}, walled until ${clockTime(entry.until ?? 0)}`,
      };
    case 'cooldown':
      return {
        label: `◌ ${slot}`,
        text: `${entry.why ?? ''}, rests ${Math.round((entry.secs ?? 0) / 60) || 1} min`,
      };
    case 'retry':
      return { label: `↻ ${slot}`, text: `temporary error, retry in ${entry.wait ?? 0}s` };
    case 'signed-out':
      return { label: `✕ ${slot}`, text: 'sign-in rejected: sign in again' };
    case 'paused':
      return { label: `⏸ ${slot}`, text: 'paused by you' };
    case 'resumed':
      return { label: `▶ ${slot}`, text: 'resumed by you' };
    default:
      return { label: entry.event, text: entry.slot ?? '' };
  }
}
