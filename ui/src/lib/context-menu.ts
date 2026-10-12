/** Owns the one open action menu and returns focus to its trigger when dismissed. */
import { get, writable } from 'svelte/store';
export interface MenuItem {
  label: string;
  action: () => void | Promise<unknown>;
  danger?: boolean;
  disabled?: boolean;
  /** A choice in a set: shown with a check and announced as a radio item. */
  checked?: boolean;
  /** A small heading shown above this item, starting a new section. */
  heading?: string;
}
export const contextMenu = writable<{
  x: number;
  y: number;
  caption?: string | undefined;
  items: MenuItem[];
  trigger?: HTMLElement | undefined;
} | null>(null);
export function closeMenu(restore = true) {
  const trigger = get(contextMenu)?.trigger;
  contextMenu.set(null);
  if (restore) trigger?.focus();
}
export function openMenu(event: MouseEvent, items: MenuItem[], caption?: string) {
  event.preventDefault();
  const trigger = event.currentTarget instanceof HTMLElement ? event.currentTarget : undefined;
  const rect = trigger?.getBoundingClientRect();
  contextMenu.set({
    x: event.type === 'contextmenu' ? event.clientX : (rect?.left ?? event.clientX),
    y: event.type === 'contextmenu' ? event.clientY : (rect?.bottom ?? event.clientY),
    items,
    caption,
    trigger,
  });
}
