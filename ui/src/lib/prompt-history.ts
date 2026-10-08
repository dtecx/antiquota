/** Owns prompt recall and preserves the draft that preceded history navigation. */
import type { EventRecord } from '../gen/EventRecord';

interface Prompt {
  text: string;
  durableIndex?: number;
  claimed: boolean;
}

const local = (text: string) => /^[!#]/.test(text.trimStart());
const userPrompts = (events: EventRecord[]) =>
  events.flatMap((event) =>
    event.t === 'user' && (event.kind === 'user' || event.kind === 'message') ? [event.text] : [],
  );

export function createPromptHistory() {
  let selected = '';
  let durable: string[] = [];
  let prompts: Prompt[] = [];
  let position = -1;
  let draft = '';
  function edited() {
    position = -1;
    draft = '';
  }
  function reset(texts: string[]) {
    durable = texts;
    prompts = texts.map((text, durableIndex) => ({ text, durableIndex, claimed: true }));
    edited();
  }
  return {
    select: (id: string, events: EventRecord[]) => {
      selected = id;
      reset(id ? userPrompts(events) : []);
    },
    sync: (events: EventRecord[]) => {
      if (!selected) return;
      const next = userPrompts(events);
      if (durable.some((text, index) => text !== next[index])) return reset(next);
      const recalled = position < 0 ? undefined : prompts[prompts.length - 1 - position];
      for (let index = durable.length; index < next.length; index++) {
        const text = next[index]!;
        const pending = prompts.find(
          (prompt) =>
            prompt.durableIndex === undefined && !local(prompt.text) && prompt.text === text,
        );
        if (pending) pending.durableIndex = index;
        else prompts.push({ text, durableIndex: index, claimed: false });
      }
      durable = next;
      if (recalled) position = prompts.length - 1 - prompts.indexOf(recalled);
    },
    remember: (text: string) => {
      const received =
        !local(text) &&
        prompts.find(
          (prompt) => prompt.durableIndex !== undefined && !prompt.claimed && prompt.text === text,
        );
      if (received) received.claimed = true;
      else prompts.push({ text, claimed: true });
      edited();
    },
    edited,
    step: (direction: 1 | -1, currentDraft: string): string | undefined => {
      if (!prompts.length) return undefined;
      if (position === -1) draft = currentDraft;
      position = Math.max(-1, Math.min(prompts.length - 1, position + direction));
      return position === -1 ? draft : prompts[prompts.length - 1 - position]?.text;
    },
    active: () => position !== -1,
  };
}
