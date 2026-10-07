/** Owns sign-in lifecycle, countdown, polling and cancellation without another event stream. */
import { get, writable } from 'svelte/store';
import type { LoginState } from '../gen/LoginState';
import type { Api } from './api';

export interface LoginView {
  open: boolean;
  phase: 'idle' | 'starting' | 'waiting' | 'submitting' | 'done' | 'verify' | 'failed';
  id: string;
  url: string;
  code: string;
  until: number;
  seconds: number;
  name: string;
  verify: string;
  error: string;
  message: string;
}
export interface ConsentTab {
  navigate: (url: string) => void;
  close: () => void;
}
export interface LoginBrowser {
  reserve: () => ConsentTab | null;
  clipboard: () => Promise<string>;
}
export const loginBrowser: LoginBrowser = {
  reserve: () => {
    const tab = window.open('', '_blank');
    if (!tab) return null;
    tab.opener = null;
    return {
      navigate: (url) => {
        tab.location.href = url;
      },
      close: () => tab.close(),
    };
  },
  clipboard: () => navigator.clipboard.readText(),
};
const codePattern = /^4\/[0-9A-Za-z_-]{20,}$/;
const empty: LoginView = {
  open: false,
  phase: 'idle',
  id: '',
  url: '',
  code: '',
  until: 0,
  seconds: 0,
  name: '',
  verify: '',
  error: '',
  message: '',
};
export function createLogin(
  api: Api,
  refresh: () => Promise<void>,
  browser: LoginBrowser = loginBrowser,
) {
  const state = writable<LoginView>({ ...empty });
  let generation = 0;
  let tick: ReturnType<typeof setInterval> | undefined;
  let poll: ReturnType<typeof setInterval> | undefined;
  let finish: ReturnType<typeof setTimeout> | undefined;
  let polling = false;
  function clear() {
    clearInterval(tick);
    clearInterval(poll);
    clearTimeout(finish);
    polling = false;
  }
  function error(message: string) {
    state.update((value) => ({ ...value, error: message }));
  }
  async function cancel(id: string) {
    if (id) await api.cancelLogin(id).catch(() => {});
  }
  function close() {
    const id = get(state).id;
    generation++;
    clear();
    state.set({ ...empty });
    void cancel(id);
  }
  function apply(result: LoginState, own: number) {
    if (own !== generation || result.state === 'waiting' || !get(state).id) return;
    clear();
    state.update((value) => ({
      ...value,
      id: '',
      phase: result.state,
      name: result.name ?? '',
      verify: result.verify_url ?? '',
      seconds: 0,
      error:
        result.state === 'failed'
          ? `${result.error || 'Sign-in failed'}. Get a new sign-in link and try again.`
          : '',
      message:
        result.state === 'done' ? `Signed in: ${result.name}. It joins the rotation now.` : '',
    }));
    if (result.state === 'done' || result.state === 'verify') void refresh();
    if (result.state === 'done') finish = setTimeout(close, 1200);
  }
  async function start() {
    if (get(state).phase === 'starting' || get(state).phase === 'submitting') return;
    let tab: ConsentTab | null = null;
    try {
      tab = browser.reserve();
    } catch {
      /* A manual consent link remains available. */
    }
    const previous = get(state).id;
    const own = ++generation;
    clear();
    state.set({ ...empty, open: true, phase: 'starting', message: 'Starting agy sign-in…' });
    await cancel(previous);
    if (own !== generation) {
      tab?.close();
      return;
    }
    try {
      const link = await api.startLogin();
      if (own !== generation) {
        tab?.close();
        await cancel(link.id);
        return;
      }
      let opened = false;
      try {
        if (tab) {
          tab.navigate(link.url);
          opened = true;
        }
      } catch {
        tab?.close();
      }
      const until = Date.now() + link.seconds * 1000;
      state.set({
        ...empty,
        open: true,
        phase: 'waiting',
        id: link.id,
        url: link.url,
        until,
        seconds: link.seconds,
        message: opened
          ? 'Sign in in the new tab, then paste the code here.'
          : 'Open the Google sign-in page, sign in, then paste the code here.',
      });
      tick = setInterval(
        () =>
          state.update((value) => ({
            ...value,
            seconds: Math.max(0, Math.round((value.until - Date.now()) / 1000)),
          })),
        500,
      );
      poll = setInterval(async () => {
        if (polling || own !== generation || !get(state).id) return;
        polling = true;
        try {
          apply(await api.loginState(link.id), own);
        } catch {
          /* The next poll can recover a temporary connection error. */
        } finally {
          if (own === generation) polling = false;
        }
      }, 2000);
    } catch (cause) {
      tab?.close();
      if (own === generation) {
        state.update((value) => ({ ...value, phase: 'idle', message: '' }));
        error(cause instanceof Error ? cause.message : 'Cannot start sign-in.');
      }
    }
  }
  async function submit() {
    const value = get(state);
    if (!value.id || !value.code.trim() || value.phase !== 'waiting') return;
    const own = generation;
    state.update((current) => ({
      ...current,
      phase: 'submitting',
      error: '',
      message: 'Checking the code…',
    }));
    try {
      const result = await api.submitLogin(value.id, { code: value.code.trim() });
      apply(result, own);
      if (result.state === 'waiting' && own === generation && get(state).phase === 'submitting')
        state.update((current) => ({ ...current, phase: 'waiting' }));
    } catch (cause) {
      if (own === generation && get(state).phase === 'submitting') {
        state.update((current) => ({ ...current, phase: 'waiting' }));
        error(cause instanceof Error ? cause.message : 'Cannot check this code.');
      }
    }
  }
  async function paste(quiet = false) {
    const value = get(state);
    if (!value.id || value.code || value.phase !== 'waiting') return;
    const own = generation;
    try {
      const code = (await browser.clipboard()).trim();
      if (own !== generation || get(state).code || get(state).phase !== 'waiting') return;
      if (codePattern.test(code)) {
        state.update((current) => ({ ...current, code, error: '' }));
        await submit();
      } else if (!quiet)
        error('The clipboard holds no sign-in code yet. Copy the code from Google’s page.');
    } catch {
      if (!quiet && own === generation)
        error('The browser did not allow reading the clipboard; paste the code with ⌘/Ctrl+V.');
    }
  }
  return {
    state,
    start,
    submit,
    paste,
    close,
    dispose: close,
    open: () => {
      close();
      state.set({ ...empty, open: true });
    },
    code: (code: string) => state.update((value) => ({ ...value, code, error: '' })),
    pasted: (code: string) => {
      if (get(state).phase !== 'waiting' || !get(state).id) return;
      if (codePattern.test(code.trim())) {
        state.update((value) => ({ ...value, code: code.trim(), error: '' }));
        void submit();
      } else error('Paste a sign-in code beginning with 4/.');
    },
  };
}
export type Login = ReturnType<typeof createLogin>;
