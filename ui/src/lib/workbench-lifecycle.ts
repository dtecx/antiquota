/** Owns mount-scoped model retry, transient toast feedback and the minute clock. */
import { readable, writable } from 'svelte/store';
import type { Workbench } from './workbench';

export const minuteClock = readable(Date.now() / 1000, (set) => {
  set(Date.now() / 1000);
  const timer = setInterval(() => set(Date.now() / 1000), 60000);
  return () => clearInterval(timer);
});
export function createToast() {
  const state = writable('');
  let timer: ReturnType<typeof setTimeout> | undefined;
  function show(text: string) {
    clearTimeout(timer);
    state.set(text);
    timer = setTimeout(() => state.set(''), 2600);
  }
  return { state, show, dispose: () => clearTimeout(timer) };
}
export function watchWorkbench(workbench: Workbench, toast: ReturnType<typeof createToast>) {
  let initialized = false,
    feedback = '',
    error = '';
  let retry: ReturnType<typeof setTimeout> | undefined;
  const unsubscribe = workbench.state.subscribe((view) => {
    if (!initialized && view.snapshot) {
      initialized = true;
      if (!view.snapshot.models.length) retry = setTimeout(() => void workbench.refresh(), 4000);
    }
    if (view.feedback && view.feedback !== feedback) toast.show(view.feedback);
    if (view.error && view.error !== error) toast.show(view.error);
    feedback = view.feedback;
    error = view.error;
  });
  return () => {
    clearTimeout(retry);
    unsubscribe();
  };
}
