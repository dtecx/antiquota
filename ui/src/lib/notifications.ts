/** Sends opted-in browser notifications; native delivery is provided by the desktop adapter. */
import type { EventRecord } from '../gen/EventRecord';
import type { Preferences, NotificationKind } from './preferences';
interface BrowserNotifications {
  visible: () => boolean;
  focused: () => boolean;
  permission: () => NotificationPermission | null;
  request: () => Promise<NotificationPermission>;
  send: (title: string, body: string) => void;
}
export function createNotifications(browser: BrowserNotifications) {
  async function request() {
    try {
      if (browser.permission() === 'default') await browser.request();
    } catch {
      /* Permission prompts can be blocked. */
    }
  }
  function send(title: string, body: string, kind: NotificationKind, prefs: Preferences) {
    if (!prefs.notify[kind] || (!prefs.focused && browser.visible() && browser.focused())) return;
    try {
      if (browser.permission() === 'granted') browser.send(title, body);
    } catch {
      /* Notification delivery must not interrupt a turn. */
    }
  }
  function event(event: EventRecord, title: string, prefs: Preferences) {
    if (event.t === 'turn_end' && event.status.toLowerCase() !== 'quota')
      send(`${title}: turn ${event.turn} ${event.status}`, event.slot, 'turn', prefs);
    else if (event.t === 'approval' && event.state === 'pending')
      send('Approval needed', event.tool, 'approval', prefs);
    else if (event.t === 'job' && event.phase === 'ended')
      send('Job finished', title, 'job', prefs);
    else if (
      event.t === 'notice' &&
      event.kind === 'goal' &&
      event.data &&
      typeof event.data === 'object' &&
      !Array.isArray(event.data) &&
      event.data.met === true
    )
      send('Goal met', title, 'goal', prefs);
    else if (event.t === 'notice' && event.kind === 'wall')
      send('Out of quota', event.text, 'quota', prefs);
  }
  return { request, send, event };
}
export const notifications = createNotifications({
  visible: () => !document.hidden,
  focused: () => document.hasFocus(),
  permission: () => (typeof Notification === 'undefined' ? null : Notification.permission),
  request: () => Notification.requestPermission(),
  send: (title, body) => {
    new Notification(title, { body, tag: 'antiquota' });
  },
});
