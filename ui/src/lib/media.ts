/** Owns composer attachments (the upload tray, uploads, notices) and the pure parts of dictation. */
import { writable } from 'svelte/store';
import type { Api } from './api';

/** Largest file or recording the engine accepts, in bytes. */
export const UPLOAD_LIMIT = 25_000_000;

/** A file in the composer tray: uploading, or uploaded and ready to send. */
export interface PendingFile {
  key: number;
  name: string;
  size: number;
  image: boolean;
  uploading: boolean;
  /** Where the engine stored it; set once the upload finished. */
  path?: string;
  /** A local preview for images. */
  preview?: string;
}

/** Each session's tray, so switching sessions keeps what was attached to each. */
export const trays = writable<Record<string, PendingFile[]>>({});

let nextKey = 1;
export function pendingFile(file: File, name: string): PendingFile {
  const image = file.type.startsWith('image/');
  return {
    key: nextKey++,
    name,
    size: file.size,
    image,
    uploading: true,
    ...(image && typeof URL.createObjectURL === 'function'
      ? { preview: URL.createObjectURL(file) }
      : {}),
  };
}

function setTray(sid: string, change: (files: PendingFile[]) => PendingFile[]) {
  trays.update((all) => ({ ...all, [sid]: change(all[sid] ?? []) }));
}

/** Drops one file from a session's tray. */
export function removeFile(sid: string, file: PendingFile) {
  if (file.preview) URL.revokeObjectURL(file.preview);
  setTray(sid, (files) => files.filter((f) => f.key !== file.key));
}

/** Drops files that were sent from a session's tray. */
export function removeSent(sid: string, paths: string[]) {
  setTray(sid, (files) =>
    files.filter((file) => {
      const sent = paths.includes(file.path ?? '');
      if (sent && file.preview) URL.revokeObjectURL(file.preview);
      return !sent;
    }),
  );
}

/** Opening another session clears unsent attachments everywhere else. */
export function clearTrays(keep: string | string[]) {
  const visible = typeof keep === 'string' ? [keep] : keep;
  trays.update((all) => {
    for (const [sid, files] of Object.entries(all))
      if (!visible.includes(sid))
        for (const file of files) if (file.preview) URL.revokeObjectURL(file.preview);
    return Object.fromEntries(Object.entries(all).filter(([sid]) => visible.includes(sid)));
  });
}

/** A short message under the composer, such as "Still uploading…". */
export const mediaNotice = writable('');
let noticeTimer: ReturnType<typeof setTimeout> | undefined;
export function say(text: string) {
  mediaNotice.set(text);
  clearTimeout(noticeTimer);
  noticeTimer = setTimeout(() => mediaNotice.set(''), 4000);
}

/** Uploads files into a session's tray one by one; a failed upload leaves no chip. */
export async function uploadFiles(
  api: Pick<Api, 'attach'>,
  sid: string,
  files: File[],
  fail: (error: unknown) => void,
) {
  for (const file of files) {
    if (file.size > UPLOAD_LIMIT) {
      say(`${file.name} is over 25 MB`);
      continue;
    }
    const entry = pendingFile(file, file.name || 'pasted.png');
    setTray(sid, (current) => [...current, entry]);
    try {
      const stored = await api.attach(sid, file, entry.name);
      setTray(sid, (current) =>
        current.map((f) =>
          f.key === entry.key
            ? {
                ...f,
                name: stored.name,
                path: stored.path,
                image: stored.image,
                size: stored.size ?? f.size,
                uploading: false,
              }
            : f,
        ),
      );
    } catch (error) {
      removeFile(sid, entry);
      fail(error);
    }
  }
}

/** Files that finished uploading, in the order they were attached. */
export function readyPaths(tray: PendingFile[]): string[] {
  return tray.filter((file) => !file.uploading && file.path).map((file) => file.path ?? '');
}

/** Browsers name every pasted image image.png; give it a name that sorts by time instead. */
export function pastedName(name: string, now: Date): string {
  if (name && name !== 'image.png') return name;
  const stamp = now.toTimeString().slice(0, 8).replace(/:/g, '');
  return `screenshot-${stamp}.png`;
}

/** The short type label shown for a file without a preview. */
export function extensionLabel(name: string): string {
  const dot = name.lastIndexOf('.');
  return (dot > 0 ? name.slice(dot + 1) : 'file').slice(0, 4) || 'file';
}

/** Inserts text at the selection with a separating space, and the caret after it. */
export function insertAt(
  value: string,
  start: number,
  end: number,
  text: string,
): { value: string; caret: number } {
  const before = value.slice(0, start);
  const pad = before && !/\s$/.test(before) ? ' ' : '';
  const head = before + pad + text;
  return { value: head + value.slice(end), caret: head.length };
}

/** Why a recording is not worth sending, or null when it is. */
export function recordingProblem(seconds: number, peak: number): 'short' | 'silent' | null {
  if (seconds < 0.5) return 'short';
  return peak < 0.01 ? 'silent' : null;
}

/** The loudest sample in a block of audio. */
export function peakOf(samples: Float32Array): number {
  let peak = 0;
  for (const sample of samples) peak = Math.max(peak, Math.abs(sample));
  return peak;
}

/** Recorder containers in order of preference; the first one the browser records is used.
 * WebM Opus transcribes through agy; Chromium's Opus-in-MP4 did not, so MP4 is only the
 * fallback for WebKit, which records AAC MP4 and nothing else. */
export const RECORDING_TYPES = ['audio/webm;codecs=opus', 'audio/webm', 'audio/mp4', 'audio/ogg'];

/** The file extension the engine stores a recording under, from the recorder's media type. */
export function audioExtension(type: string): string {
  const base = type.split(';')[0]?.trim().toLowerCase() ?? '';
  return (
    {
      'audio/ogg': 'ogg',
      'audio/mp4': 'm4a',
      'audio/x-m4a': 'm4a',
      'audio/aac': 'aac',
      'audio/mpeg': 'mp3',
      'audio/wav': 'wav',
      'audio/x-wav': 'wav',
      'audio/flac': 'flac',
      'audio/webm': 'webm',
    }[base] ?? 'webm'
  );
}

/** "● 1:05 · Esc cancels" while recording. */
export function recordingLabel(seconds: number): string {
  const whole = Math.max(0, Math.floor(seconds));
  return `● ${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, '0')} · Esc cancels`;
}
