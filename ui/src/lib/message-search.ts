/** Owns debounced message search and rejects stale responses, including cleared queries. */
import { writable } from 'svelte/store';
import type { Api } from './api';
import type { SearchHit } from '../gen/SearchHit';
export function createMessageSearch(api: Pick<Api, 'search'>) {
  const state = writable<{ hits: SearchHit[]; loading: boolean; error: string }>({
    hits: [],
    loading: false,
    error: '',
  });
  let generation = 0,
    timer: ReturnType<typeof setTimeout> | undefined;
  function query(text: string) {
    const own = ++generation,
      q = text.trim().toLowerCase();
    clearTimeout(timer);
    state.set({ hits: [], loading: q.length >= 2, error: '' });
    if (q.length < 2) return;
    timer = setTimeout(async () => {
      try {
        const hits = await api.search(q);
        if (own === generation) state.set({ hits, loading: false, error: '' });
      } catch (error) {
        if (own === generation)
          state.set({
            hits: [],
            loading: false,
            error: error instanceof Error ? error.message : 'Search unavailable.',
          });
      }
    }, 220);
  }
  return {
    state,
    query,
    dispose: () => {
      generation++;
      clearTimeout(timer);
    },
  };
}
