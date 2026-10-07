/** Owns in-app confirmation questions; the dialog component answers them. */
import { writable } from 'svelte/store';

export interface Question {
  title: string;
  text: string;
  ok: string;
  danger: boolean;
  answer: (yes: boolean) => void;
}
export const question = writable<Question | null>(null);

/** Asks the user and resolves true only when they confirm. */
export function ask(
  title: string,
  text: string,
  { ok = 'OK', danger = false }: { ok?: string; danger?: boolean } = {},
): Promise<boolean> {
  return new Promise((resolve) => {
    question.set({
      title,
      text,
      ok,
      danger,
      answer: (yes) => {
        question.set(null);
        resolve(yes);
      },
    });
  });
}
