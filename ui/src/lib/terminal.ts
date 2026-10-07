/** Owns one xterm instance per session, its shell connection and lazy loading of xterm. */
import type { Terminal as Xterm } from '@xterm/xterm';
import type { FitAddon } from '@xterm/addon-fit';
import type { Api } from './api';

interface Modules {
  Terminal: typeof Xterm;
  FitAddon: typeof FitAddon;
}
let loading: Promise<Modules> | null = null;

/** Loads xterm and its stylesheet once; a failure can be retried. */
export function loadXterm(): Promise<Modules> {
  loading ??= Promise.all([
    import('@xterm/xterm'),
    import('@xterm/addon-fit'),
    import('@xterm/xterm/css/xterm.css'),
  ])
    .then(([xterm, fit]) => ({ Terminal: xterm.Terminal, FitAddon: fit.FitAddon }))
    .catch((error: unknown) => {
      loading = null;
      throw error;
    });
  return loading;
}

/** Decodes a base64 frame into bytes for xterm. */
export function decodeFrame(data: string): Uint8Array {
  const text = atob(data);
  const bytes = new Uint8Array(text.length);
  for (let i = 0; i < text.length; i += 1) bytes[i] = text.charCodeAt(i);
  return bytes;
}

export interface Frame {
  d?: string;
  shell?: string;
  cwd?: string;
  alive?: boolean;
  exit?: boolean;
}

export interface Shell {
  sid: string;
  term: Xterm;
  fit: FitAddon;
  element: HTMLDivElement;
  title: string;
  alive: boolean;
  source: EventSource | null;
  resizeTimer: ReturnType<typeof setTimeout> | undefined;
  listeners: Set<() => void>;
}

const shells = new Map<string, Shell>();

function changed(shell: Shell) {
  for (const listener of shell.listeners) listener();
}

function red(shell: Shell, text: string) {
  shell.term.write(`\r\n\x1b[31m${text}\x1b[0m\r\n`);
}

function cssVar(name: string, fallback: string) {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
}

/** The session's shell view, created once and kept across tab and session switches. */
export async function shellFor(
  sid: string,
  api: Api,
  streamUrl: (sid: string) => string,
): Promise<Shell> {
  const existing = shells.get(sid);
  if (existing) return existing;
  const { Terminal, FitAddon } = await loadXterm();
  const element = document.createElement('div');
  element.className = 'xterm-host';
  const term = new Terminal({
    scrollback: 5000,
    cursorBlink: true,
    fontFamily: cssVar('--mono', 'monospace'),
    fontSize: 12,
    theme: {
      background: cssVar('--well', '#0a0b0c'),
      foreground: cssVar('--ink', '#e8e6e1'),
      cursor: cssVar('--baton', '#e8a33d'),
    },
  });
  const fit = new FitAddon();
  term.loadAddon(fit);
  term.open(element);
  const shell: Shell = {
    sid,
    term,
    fit,
    element,
    title: 'Terminal',
    alive: false,
    source: null,
    resizeTimer: undefined,
    listeners: new Set(),
  };
  term.onData((data) => {
    api.termInput(sid, data).catch((error: unknown) => {
      red(shell, error instanceof Error ? error.message : 'Input failed.');
    });
  });
  shells.set(sid, shell);
  await start(shell, api, streamUrl, false);
  return shell;
}

function connect(shell: Shell, url: string) {
  shell.source?.close();
  const source = new EventSource(url);
  shell.source = source;
  let first = true;
  source.onmessage = (message: MessageEvent<string>) => {
    const frame = JSON.parse(message.data) as Frame;
    if (frame.exit) {
      shell.alive = false;
      shell.title = shell.title.replace(/( · exited)?$/, ' · exited');
      source.close();
      if (shell.source === source) shell.source = null;
      changed(shell);
      return;
    }
    if (first) {
      // The first frame replays the recent screen.
      shell.term.reset();
      first = false;
    }
    if (frame.d) shell.term.write(decodeFrame(frame.d));
  };
}

async function start(shell: Shell, api: Api, streamUrl: (sid: string) => string, restart: boolean) {
  try {
    shell.fit.fit();
  } catch {
    /* Not attached yet; the engine's default size applies. */
  }
  try {
    const info = await api.termStart(shell.sid, {
      cols: shell.term.cols,
      rows: shell.term.rows,
      restart,
    });
    shell.alive = info.alive;
    shell.title = `${info.shell} · ${info.cwd}`;
    connect(shell, streamUrl(shell.sid));
  } catch (error) {
    red(shell, error instanceof Error ? error.message : 'The shell did not start.');
  }
  changed(shell);
}

export function restartShell(shell: Shell, api: Api, streamUrl: (sid: string) => string) {
  return start(shell, api, streamUrl, true);
}

export async function killShell(shell: Shell, api: Api) {
  try {
    await api.termKill(shell.sid);
  } catch (error) {
    red(shell, error instanceof Error ? error.message : 'The shell could not be ended.');
  }
}

/** Fits xterm to its pane and tells the engine the new size after a short pause. */
export function fitShell(shell: Shell, api: Api) {
  try {
    shell.fit.fit();
  } catch {
    return;
  }
  clearTimeout(shell.resizeTimer);
  shell.resizeTimer = setTimeout(() => {
    void api.termResize(shell.sid, shell.term.cols, shell.term.rows).catch(() => {});
  }, 80);
}
