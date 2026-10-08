/** Owns cancellable folder browsing and the recent project list. */
import { writable } from 'svelte/store';
import type { Api } from './api';
import type { FolderResponse } from '../gen/FolderResponse';
import type { Session } from '../gen/Session';

export function recentFolders(sessions: Session[]) {
  return [...new Set([...sessions].sort((a, b) => b.updated - a.updated).map((s) => s.cwd))]
    .filter(Boolean)
    .slice(0, 4);
}
export function createFolders(api: Pick<Api, 'browse'>) {
  const state = writable<{ listing: FolderResponse | null; recent: boolean; error: string }>({
    listing: null,
    recent: true,
    error: '',
  });
  let generation = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  async function load(path: string, own: number) {
    try {
      const listing = await api.browse(path);
      if (own === generation) state.set({ listing, recent: !path, error: '' });
    } catch (error) {
      if (own === generation)
        state.set({
          listing: null,
          recent: !path,
          error: error instanceof Error ? error.message : 'Cannot browse this folder.',
        });
    }
  }
  function browse(path: string, delayed = false) {
    clearTimeout(timer);
    const own = ++generation;
    state.set({ listing: null, recent: !path, error: '' });
    if (delayed) timer = setTimeout(() => void load(path, own), 200);
    else void load(path, own);
  }
  function dispose() {
    generation++;
    clearTimeout(timer);
  }
  return { state, browse, dispose };
}
