/** Owns desktop capabilities and their browser fallbacks. */
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { getCurrentWindow, UserAttentionType } from '@tauri-apps/api/window';
import { openPath, openUrl, revealItemInDir } from '@tauri-apps/plugin-opener';
import { readText, writeText } from '@tauri-apps/plugin-clipboard-manager';
import { enable, disable, isEnabled } from '@tauri-apps/plugin-autostart';
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from '@tauri-apps/plugin-notification';

export const native = typeof window !== 'undefined' && isTauri();
let permission: NotificationPermission = 'default';
let zoomQueue = Promise.resolve();
export const readClipboard = () => (native ? readText() : navigator.clipboard.readText());
export const writeClipboard = (text: string) =>
  native ? writeText(text) : navigator.clipboard.writeText(text);
export async function externalLink(url: string): Promise<void> {
  if (!['http:', 'https:'].includes(new URL(url).protocol))
    throw new Error('Only HTTP links can open externally.');
  if (native) await openUrl(url);
  else window.open(url, '_blank', 'noopener,noreferrer');
}
export const openNativePath = (path: string) => openPath(path);
export const revealNativePath = (path: string) => revealItemInDir(path);
export const autostartEnabled = () => isEnabled();
export const setAutostart = (on: boolean) => (on ? enable() : disable());
export const restartEngine = () => invoke<void>('restart_engine');
export const retryEngine = () => invoke<void>('retry_engine');
export const quitApplication = () => invoke<void>('quit_app');
export const saveNativeFile = (name: string, contents: string) =>
  invoke<string | null>('save_file', { name, contents });
export const pickNativeFolder = (start: string) =>
  invoke<string | null>('pick_folder', { start: start || null });
export async function appInformation() {
  const value = await invoke<unknown>('app_info');
  if (
    !value ||
    typeof value !== 'object' ||
    !('os' in value) ||
    !('version' in value) ||
    typeof value.os !== 'string' ||
    typeof value.version !== 'string'
  )
    throw new Error('Desktop information is unavailable.');
  return { os: value.os, version: value.version };
}
export function applyNativeZoom(zoom: number): Promise<void> {
  if (!native) return Promise.resolve();
  const normalized = Number.isFinite(zoom) ? Math.min(1.6, Math.max(0.7, zoom)) : 1;
  document.documentElement.style.setProperty('--native-top', `${52 / normalized}px`);
  const next = zoomQueue.catch(() => {}).then(() => getCurrentWebview().setZoom(normalized));
  zoomQueue = next;
  return next;
}
export const nativeNotificationPermission = () => permission;
export async function requestNativeNotifications(): Promise<NotificationPermission> {
  if (await isPermissionGranted()) permission = 'granted';
  else permission = await requestPermission();
  return permission;
}
export async function sendNativeNotification(title: string, body: string): Promise<void> {
  if (await isPermissionGranted()) sendNotification({ title, body });
}
export const nativeAttention = () =>
  getCurrentWindow().requestUserAttention(UserAttentionType.Informational);

/** The listener cleanup also covers teardown before asynchronous registration completes. */
export function connectNative(
  onAction: (action: string) => void,
  fail: (error: unknown) => void,
): () => void {
  if (!native) return () => {};
  let active = true,
    unlisten: (() => void) | undefined;
  const report = (error: unknown) => {
    if (active) fail(error);
  };
  void listen<unknown>('native-action', (event) => {
    if (active && typeof event.payload === 'string') onAction(event.payload);
  })
    .then((stop) => {
      if (active) unlisten = stop;
      else stop();
    })
    .catch(report);
  void appInformation()
    .then((info) => {
      if (active) document.documentElement.dataset.nativeOs = info.os;
    })
    .catch(report);
  void isPermissionGranted()
    .then((granted) => {
      if (active && granted) permission = 'granted';
    })
    .catch(report);
  function link(event: MouseEvent) {
    if (
      !active ||
      event.defaultPrevented ||
      event.button !== 0 ||
      !(event.target instanceof Element)
    )
      return;
    const anchor = event.target.closest<HTMLAnchorElement>('a[href]');
    if (!anchor || anchor.hasAttribute('download')) return;
    let destination: URL;
    try {
      destination = new URL(anchor.href);
    } catch (error) {
      event.preventDefault();
      report(error);
      return;
    }
    if (!['http:', 'https:'].includes(destination.protocol)) return;
    if (destination.origin === location.origin && anchor.target !== '_blank') return;
    event.preventDefault();
    void externalLink(destination.href).catch(report);
  }
  document.addEventListener('click', link);
  return () => {
    active = false;
    unlisten?.();
    document.removeEventListener('click', link);
  };
}
