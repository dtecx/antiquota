/** Owns composer suggestions, request ordering and insertion at the original caret. */
import { get, writable } from 'svelte/store';
import type { Api } from './api';
import { commands } from './commands';

export interface AutocompleteItem {
  label: string;
  detail: string;
}

export function createAutocomplete(api: Pick<Api, 'mentionFiles'>) {
  const state = writable<{ items: AutocompleteItem[]; index: number }>({ items: [], index: 0 });
  let generation = 0;
  let disposed = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let source: { text: string; start: number; caret: number; slash: boolean } | null = null;

  function clearTimer() {
    clearTimeout(timer);
    timer = undefined;
  }

  function close() {
    clearTimer();
    generation++;
    source = null;
    state.set({ items: [], index: 0 });
  }

  function show(items: AutocompleteItem[]) {
    const index = Math.min(get(state).index, Math.max(0, items.length - 1));
    state.set({ items, index });
    if (!items.length) source = null;
  }

  return {
    state,
    async query(sid: string, text: string, caret: number) {
      if (disposed) return;
      clearTimer();
      const own = ++generation;
      source = null;
      if (text.startsWith('/') && !/\s/.test(text)) {
        source = { text, start: 0, caret: text.length, slash: true };
        show(
          commands
            .filter(([label]) => label?.startsWith(text.toLowerCase()))
            .map(([label = '', detail = '']) => ({ label, detail })),
        );
        return;
      }
      state.set({ items: [], index: 0 });
      const before = text.slice(0, caret);
      const mention = before.match(/(^|\s)@([\w./~-]*)$/);
      if (!mention || !sid) return;
      const q = mention[2] ?? '';
      try {
        const files = await api.mentionFiles(sid, q);
        if (own !== generation || disposed) return;
        source = { text, start: before.length - q.length, caret, slash: false };
        show(files.map((label) => ({ label, detail: 'file' })));
      } catch {
        // Completion is optional; a failed lookup leaves the draft untouched.
      }
    },
    step(delta: number) {
      state.update(({ items, index }) => ({
        items,
        index: items.length ? (((index + delta) % items.length) + items.length) % items.length : 0,
      }));
    },
    apply(): { text: string; caret: number } | undefined {
      const { items, index } = get(state);
      const item = items[index];
      if (!item || !source) return;
      const result = source.slash
        ? { text: `${item.label} `, caret: item.label.length + 1 }
        : {
            text: `${source.text.slice(0, source.start)}${item.label} ${source.text.slice(source.caret)}`,
            caret: source.start + item.label.length + 1,
          };
      close();
      return result;
    },
    close,
    blur() {
      if (disposed) return;
      clearTimer();
      timer = setTimeout(close, 100);
    },
    dispose() {
      disposed = true;
      close();
    },
  };
}
